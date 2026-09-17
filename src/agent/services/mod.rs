use crate::agent::provider::Provider;
use crate::agent::tools::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

pub struct CoreServices {
    pub tools: Arc<dyn ToolsService>,
    pub providers: Arc<dyn ProvidersService>,
    pub cache: Arc<dyn CacheService>,
    pub security: Arc<dyn SecurityService>,
}

impl CoreServices {
    pub fn new(
        tools: Arc<dyn ToolsService>,
        providers: Arc<dyn ProvidersService>,
        cache: Arc<dyn CacheService>,
        security: Arc<dyn SecurityService>,
    ) -> Self {
        Self {
            tools,
            providers,
            cache,
            security,
        }
    }
}

#[async_trait]
pub trait ToolsService: Send + Sync {
    async fn get_tool(&self, name: &str) -> Option<Box<dyn Tool>>;
    async fn list_tools(&self) -> Vec<String>;
    async fn register_tool(&self, tool: Box<dyn Tool>) -> Result<(), anyhow::Error>;
    async fn execute_tool(&self, name: &str, params: serde_json::Value) -> Result<ToolResult, anyhow::Error>;
    async fn all_schemas(&self) -> Vec<serde_json::Value>;
}

#[async_trait]
pub trait ProvidersService: Send + Sync {
    async fn get_provider(&self, name: &str) -> Option<Arc<dyn Provider>>;
    async fn list_providers(&self) -> Vec<String>;
    async fn register_provider(&self, provider: Arc<dyn Provider>) -> Result<(), anyhow::Error>;
    async fn default_provider(&self) -> Arc<dyn Provider>;
}

#[async_trait]
pub trait CacheService: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: String, ttl_secs: u64) -> Result<(), anyhow::Error>;
    async fn delete(&self, key: &str) -> Result<(), anyhow::Error>;
    async fn clear(&self) -> Result<(), anyhow::Error>;
}

#[async_trait]
pub trait SecurityService: Send + Sync {
    async fn confirm_destructive(&self, command: &str, description: &str) -> Result<bool, anyhow::Error>;
    async fn is_destructive(&self, command: &str) -> bool;
    async fn sanitize_command(&self, command: &str) -> String;
}

pub mod default_impl {
    use super::*;
    use crate::agent::tools::{ToolRegistry, ToolResult};
    use crate::agent::provider::OllamaProvider;
    use crate::config::Config;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::Mutex as TokioMutex;

    pub struct DefaultToolsService {
        registry: Arc<TokioMutex<ToolRegistry>>,
    }

    impl DefaultToolsService {
        pub fn new(registry: ToolRegistry) -> Self {
            Self {
                registry: Arc::new(TokioMutex::new(registry)),
            }
        }
    }

    #[async_trait]
    impl ToolsService for DefaultToolsService {
        async fn get_tool(&self, name: &str) -> Option<Box<dyn Tool>> {
            let registry = self.registry.lock().await;
            registry.get(name).map(|t| Box::new(ToolRef { name: t.name().to_string(), description: t.description().to_string() }) as Box<dyn Tool>)
        }

        async fn list_tools(&self) -> Vec<String> {
            let registry = self.registry.lock().await;
            registry.list()
        }

        async fn register_tool(&self, tool: Box<dyn Tool>) -> Result<(), anyhow::Error> {
            let mut registry = self.registry.lock().await;
            registry.register(tool);
            Ok(())
        }

        async fn execute_tool(&self, name: &str, params: serde_json::Value) -> Result<ToolResult, anyhow::Error> {
            let tool_opt = {
                let guard = self.registry.lock().await;
                guard.get(name).map(|t| format!("{}", t.name()))
            };
            if let Some(name) = tool_opt {
                let guard = self.registry.lock().await;
                guard.execute(&name, params).await
            } else {
                Err(anyhow::anyhow!("Tool not found: {}", name))
            }
        }

        async fn all_schemas(&self) -> Vec<serde_json::Value> {
            let registry = self.registry.lock().await;
            registry.all_schemas()
        }
    }

    #[derive(Clone)]
    struct ToolRef {
        name: String,
        description: String,
    }

    #[async_trait]
    impl Tool for ToolRef {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            &self.description
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn is_destructive(&self, _params: &serde_json::Value) -> bool {
            false
        }
        async fn execute(&self, _params: serde_json::Value) -> anyhow::Result<ToolResult> {
            Err(anyhow::anyhow!("Not a real tool"))
        }
    }

    pub struct DefaultProvidersService {
        providers: Arc<TokioMutex<HashMap<String, Arc<dyn Provider>>>>,
        default_name: String,
    }

    impl DefaultProvidersService {
        pub fn new(config: &Config) -> Self {
            let mut providers: HashMap<String, Arc<dyn Provider>> = HashMap::new();
            let ollama = Arc::new(OllamaProvider::new(config));
            providers.insert("ollama".to_string(), ollama);
            Self {
                providers: Arc::new(TokioMutex::new(providers)),
                default_name: "ollama".to_string(),
            }
        }
    }

    #[async_trait]
    impl ProvidersService for DefaultProvidersService {
        async fn get_provider(&self, name: &str) -> Option<Arc<dyn Provider>> {
            let providers = self.providers.lock().await;
            providers.get(name).cloned()
        }

        async fn list_providers(&self) -> Vec<String> {
            let providers = self.providers.lock().await;
            providers.keys().cloned().collect()
        }

        async fn register_provider(&self, provider: Arc<dyn Provider>) -> Result<(), anyhow::Error> {
            let mut providers = self.providers.lock().await;
            providers.insert(provider.name().to_string(), provider);
            Ok(())
        }

        async fn default_provider(&self) -> Arc<dyn Provider> {
            let providers = self.providers.lock().await;
            providers.get(&self.default_name).cloned().expect("default provider not found")
        }
    }

    pub struct DefaultCacheService {
        cache: Arc<TokioMutex<HashMap<String, CacheEntry>>>,
    }

    struct CacheEntry {
        value: String,
        expires_at: Option<Instant>,
    }

    impl DefaultCacheService {
        pub fn new() -> Self {
            Self {
                cache: Arc::new(TokioMutex::new(HashMap::new())),
            }
        }

        fn is_expired(entry: &CacheEntry) -> bool {
            entry.expires_at.map_or(false, |exp| Instant::now() > exp)
        }
    }

    #[async_trait]
    impl CacheService for DefaultCacheService {
        async fn get(&self, key: &str) -> Option<String> {
            let mut cache = self.cache.lock().await;
            if let Some(entry) = cache.get(key) {
                if Self::is_expired(entry) {
                    cache.remove(key);
                    None
                } else {
                    Some(entry.value.clone())
                }
            } else {
                None
            }
        }

        async fn set(&self, key: &str, value: String, ttl_secs: u64) -> Result<(), anyhow::Error> {
            let mut cache = self.cache.lock().await;
            let expires_at = if ttl_secs > 0 {
                Some(Instant::now() + Duration::from_secs(ttl_secs))
            } else {
                None
            };
            cache.insert(key.to_string(), CacheEntry { value, expires_at });
            Ok(())
        }

        async fn delete(&self, key: &str) -> Result<(), anyhow::Error> {
            let mut cache = self.cache.lock().await;
            cache.remove(key);
            Ok(())
        }

        async fn clear(&self) -> Result<(), anyhow::Error> {
            let mut cache = self.cache.lock().await;
            cache.clear();
            Ok(())
        }
    }

    pub struct DefaultSecurityService;

    impl DefaultSecurityService {
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl SecurityService for DefaultSecurityService {
        async fn confirm_destructive(&self, command: &str, description: &str) -> Result<bool, anyhow::Error> {
            use crate::utils::confirm::confirm_destructive;
            let prompt = format!("{}: {}", command, description);
            Ok(confirm_destructive(&prompt))
        }

        async fn is_destructive(&self, command: &str) -> bool {
            let destructive_patterns = [
                "rm -rf", "rm -r", "rm -f", "dd ", "mkfs", "fdisk", "parted",
                "wipefs", "shred", "chmod -R", "chown -R", "userdel", "groupdel",
                "deluser", "apt purge", "snap remove", "flatpak uninstall",
                "docker system prune", "docker volume prune", "podman system prune",
                "git reset --hard", "git clean -fd", "truncate", "reboot",
                "poweroff", "shutdown", "systemctl disable", "systemctl mask",
            ];
            let lower = command.to_lowercase();
            destructive_patterns.iter().any(|p| lower.contains(p))
        }

        async fn sanitize_command(&self, command: &str) -> String {
            command.trim().to_string()
        }
    }
}