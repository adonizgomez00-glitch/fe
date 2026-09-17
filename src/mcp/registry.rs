use std::collections::HashMap;

use anyhow::{Result, anyhow};
use serde_json::Value;

use super::client::McpClient;
use super::types::{McpServerConfig, McpTool, McpResource, McpPrompt, McpPromptMessage};

pub struct McpRegistry {
    clients: HashMap<String, McpClient>,
}

impl Drop for McpRegistry {
    fn drop(&mut self) {
        for (name, mut client) in self.clients.drain() {
            tracing::info!("Shutting down MCP server '{}'", name);
            let _ = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new();
                if let Ok(rt) = rt {
                    let _ = rt.block_on(client.shutdown());
                }
            });
        }
    }
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn server_names(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    pub fn is_connected(&self, name: &str) -> bool {
        self.clients.get(name).map(|c| c.is_initialized()).unwrap_or(false)
    }

    pub fn client(&mut self, name: &str) -> Option<&mut McpClient> {
        self.clients.get_mut(name)
    }

    pub async fn load_from_config(servers: &[McpServerConfig]) -> Result<Self> {
        let mut registry = Self::new();

        for server_config in servers {
            if !server_config.enabled {
                tracing::info!("MCP server '{}' is disabled, skipping", server_config.name);
                continue;
            }

            let mut client = McpClient::new(server_config.clone());
            match client.try_spawn_and_init().await {
                Ok(()) => {
                    tracing::info!("MCP server '{}' connected successfully", server_config.name);
                    registry.clients.insert(server_config.name.clone(), client);
                }
                Err(e) => {
                    tracing::warn!("Failed to connect MCP server '{}': {}", server_config.name, e);
                }
            }
        }

        Ok(registry)
    }

    pub async fn discover_all_tools(&mut self) -> Result<Vec<(String, McpTool)>> {
        let mut all_tools = Vec::new();

        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in &names {
            let client = self.clients.get_mut(name)
                .ok_or_else(|| anyhow!("Server '{}' not found", name))?;

            if !client.is_initialized() {
                continue;
            }

            let tools = client.list_tools().await?;
            for tool in tools {
                all_tools.push((name.clone(), tool));
            }
        }

        Ok(all_tools)
    }

    pub async fn discover_all_resources(&mut self) -> Result<Vec<(String, McpResource)>> {
        let mut all_resources = Vec::new();

        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in &names {
            let client = self.clients.get_mut(name)
                .ok_or_else(|| anyhow!("Server '{}' not found", name))?;

            if !client.is_initialized() {
                continue;
            }

            match client.list_resources().await {
                Ok(resources) => {
                    for resource in resources {
                        all_resources.push((name.clone(), resource));
                    }
                }
                Err(e) => {
                    tracing::warn!("MCP server '{}' does not support resources: {}", name, e);
                }
            }
        }

        Ok(all_resources)
    }

    pub async fn read_resource(&mut self, server_name: &str, uri: &str) -> Result<String> {
        let client = self.clients.get_mut(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        client.read_resource(uri).await
    }

    pub async fn discover_all_prompts(&mut self) -> Result<Vec<(String, McpPrompt)>> {
        let mut all_prompts = Vec::new();

        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in &names {
            let client = self.clients.get_mut(name)
                .ok_or_else(|| anyhow!("Server '{}' not found", name))?;

            if !client.is_initialized() {
                continue;
            }

            match client.list_prompts().await {
                Ok(prompts) => {
                    for prompt in prompts {
                        all_prompts.push((name.clone(), prompt));
                    }
                }
                Err(e) => {
                    tracing::warn!("MCP server '{}' does not support prompts: {}", name, e);
                }
            }
        }

        Ok(all_prompts)
    }

    pub async fn get_prompt(&mut self, server_name: &str, prompt_name: &str, args: Option<Value>) -> Result<Vec<McpPromptMessage>> {
        let client = self.clients.get_mut(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        client.get_prompt(prompt_name, args).await
    }

    pub async fn call_tool(&mut self, server_name: &str, tool_name: &str, args: Value) -> Result<Value> {
        let client = self.clients.get_mut(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        client.call_tool(tool_name, args).await
    }

    pub fn format_tool_name(server: &str, tool: &str) -> String {
        format!("mcp:{}:{}", server, tool)
    }

    pub fn parse_tool_name(full_name: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = full_name.splitn(3, ':').collect();
        if parts.len() == 3 && parts[0] == "mcp" {
            Some((parts[1].to_string(), parts[2].to_string()))
        } else {
            None
        }
    }

    pub async fn shutdown_all(&mut self) {
        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in names {
            if let Some(client) = self.clients.get_mut(&name) {
                let _ = client.shutdown().await;
            }
        }
        self.clients.clear();
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_name_valid() {
        let (server, tool) = McpRegistry::parse_tool_name("mcp:filesystem:read_file").unwrap();
        assert_eq!(server, "filesystem");
        assert_eq!(tool, "read_file");
    }

    #[test]
    fn test_parse_tool_name_no_prefix() {
        assert!(McpRegistry::parse_tool_name("bash").is_none());
    }

    #[test]
    fn test_parse_tool_name_partial() {
        assert!(McpRegistry::parse_tool_name("mcp:filesystem").is_none());
    }

    #[test]
    fn test_format_tool_name() {
        let name = McpRegistry::format_tool_name("filesystem", "read_file");
        assert_eq!(name, "mcp:filesystem:read_file");
    }

    #[test]
    fn test_new_registry_empty() {
        let registry = McpRegistry::new();
        assert_eq!(registry.client_count(), 0);
    }

    #[test]
    fn test_server_names_empty() {
        let registry = McpRegistry::new();
        assert!(registry.server_names().is_empty());
    }
}
