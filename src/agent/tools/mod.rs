use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod bash;
pub mod read;
pub mod write;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod task;
pub mod webfetch;
pub mod websearch;
pub mod mkdir;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ToolResult {
    pub output: String,
    pub truncated: bool,
    pub exit_code: Option<i32>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    fn is_destructive(&self, params: &Value) -> bool;
    async fn execute(&self, params: Value) -> Result<ToolResult>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn all_schemas(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": t.schema()
                })
            })
            .collect()
    }

    pub fn list(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub async fn execute(&self, name: &str, params: Value) -> Result<ToolResult> {
        if let Some(tool) = self.get(name) {
            tool.execute(params).await
        } else {
            Err(anyhow::anyhow!("Tool not found: {}", name))
        }
    }

    pub fn register_defaults(&mut self, config: &crate::config::Config) {
        self.register(Box::new(bash::BashTool::new(config)));
        self.register(Box::new(read::ReadTool::new(config)));
        self.register(Box::new(write::WriteTool::new(config)));
        self.register(Box::new(edit::EditTool::new(config)));
        self.register(Box::new(glob::GlobTool::new(config)));
        self.register(Box::new(grep::GrepTool::new(config)));
        self.register(Box::new(task::TaskTool::new(config)));
        self.register(Box::new(webfetch::WebfetchTool::new(config)));
        self.register(Box::new(websearch::WebsearchTool::new(config)));
        self.register(Box::new(mkdir::MkdirTool::new(config)));
    }

    pub fn is_destructive(&self, name: &str, params: &Value) -> bool {
        self.get(name)
            .map(|t| t.is_destructive(params))
            .unwrap_or(false)
    }

    pub fn register_mcp_tools(&mut self, registry: &Arc<Mutex<crate::mcp::registry::McpRegistry>>, server_name: &str, tools: &[crate::mcp::types::McpTool]) {
        for tool in tools {
            let full_name = crate::mcp::registry::McpRegistry::format_tool_name(server_name, &tool.name);
            let wrapper = McpToolWrapper::new(registry.clone(), server_name.to_string(), tool.clone(), full_name);
            self.register(Box::new(wrapper));
            tracing::info!("Registered MCP tool: mcp:{}:{}", server_name, tool.name);
        }
    }

    pub fn register_plugin_tools(&mut self, runtime: &Arc<Mutex<crate::plugins::runtime::PluginRuntime>>, plugin_name: &str, tools: &[crate::plugins::manifest::PluginTool]) {
        for tool in tools {
            let full_name = format!("plugin:{}:{}", plugin_name, tool.name);
            let wrapper = PluginToolWrapper::new(runtime.clone(), plugin_name.to_string(), tool.clone(), full_name.clone());
            self.register(Box::new(wrapper));
            tracing::info!("Registered plugin tool: {}", full_name);
        }
    }
}

pub struct McpToolWrapper {
    registry: Arc<Mutex<crate::mcp::registry::McpRegistry>>,
    server_name: String,
    tool: crate::mcp::types::McpTool,
    full_name: String,
}

impl McpToolWrapper {
    pub fn new(registry: Arc<Mutex<crate::mcp::registry::McpRegistry>>, server_name: String, tool: crate::mcp::types::McpTool, full_name: String) -> Self {
        Self { registry, server_name, tool, full_name }
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.full_name
    }

    fn description(&self) -> &str {
        self.tool.description.as_deref().unwrap_or("MCP tool")
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": self.full_name,
            "description": self.description(),
            "parameters": self.tool.input_schema.clone().unwrap_or_else(|| serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }))
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        false
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let mut registry = self.registry.lock().await;
        let result = registry.call_tool(&self.server_name, &self.tool.name, params).await?;

        let output = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "{}".to_string());

        Ok(ToolResult {
            output,
            truncated: false,
            exit_code: Some(0),
        })
    }
}

pub struct PluginToolWrapper {
    runtime: Arc<Mutex<crate::plugins::runtime::PluginRuntime>>,
    plugin_name: String,
    tool: crate::plugins::manifest::PluginTool,
    full_name: String,
}

impl PluginToolWrapper {
    pub fn new(runtime: Arc<Mutex<crate::plugins::runtime::PluginRuntime>>, plugin_name: String, tool: crate::plugins::manifest::PluginTool, full_name: String) -> Self {
        Self { runtime, plugin_name, tool, full_name }
    }
}

#[async_trait]
impl Tool for PluginToolWrapper {
    fn name(&self) -> &str {
        &self.full_name
    }

    fn description(&self) -> &str {
        &self.tool.description
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": self.full_name,
            "description": self.description(),
            "parameters": self.tool.params.clone()
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        false
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let runtime = self.runtime.lock().await;
        let result = runtime.call_tool(&self.tool.name, params).await?;

        Ok(ToolResult {
            output: result.output,
            truncated: false,
            exit_code: Some(result.exit_code),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        let config = Config::default();
        registry.register_defaults(&config);

        assert!(registry.get("bash").is_some());
        assert!(registry.get("read").is_some());
        assert!(registry.get("write").is_some());
        assert!(registry.get("edit").is_some());
        assert!(registry.get("glob").is_some());
        assert!(registry.get("grep").is_some());
        assert!(registry.get("webfetch").is_some());
        assert!(registry.get("mkdir").is_some());
    }

    #[test]
    fn test_registry_unknown_tool() {
        let registry = ToolRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_all_schemas_returns_valid_json() {
        let mut registry = ToolRegistry::new();
        let config = Config::default();
        registry.register_defaults(&config);

        let schemas = registry.all_schemas();
        assert!(!schemas.is_empty());

        for schema in &schemas {
            assert!(schema.get("type").is_some());
            assert!(schema.get("function").is_some());
        }
    }
}
