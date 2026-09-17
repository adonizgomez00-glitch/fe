use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use tokio::sync::Mutex;

use super::types::*;

pub struct McpClient {
    config: McpServerConfig,
    process: Option<Child>,
    stdin: Option<Mutex<ChildStdin>>,
    stdout: Option<Mutex<BufReader<ChildStdout>>>,
    request_id: AtomicU64,
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
    prompts: Vec<McpPrompt>,
    initialized: bool,
    server_info: Option<ServerInfo>,
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.start_kill();
        }
    }
}

impl McpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            process: None,
            stdin: None,
            stdout: None,
            request_id: AtomicU64::new(1),
            tools: Vec::new(),
            resources: Vec::new(),
            prompts: Vec::new(),
            initialized: false,
            server_info: None,
        }
    }

    pub fn server_name(&self) -> &str {
        &self.config.name
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.server_info.as_ref()
    }

    pub async fn spawn(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (key, val) in &self.config.env {
            cmd.env(key, val);
        }

        let mut process = cmd.spawn()
            .map_err(|e| anyhow!("Error spawning MCP server '{}': {}", self.config.name, e))?;

        let stdin = process.stdin.take()
            .ok_or_else(|| anyhow!("Failed to open stdin for MCP server '{}'", self.config.name))?;
        let stdout = process.stdout.take()
            .ok_or_else(|| anyhow!("Failed to open stdout for MCP server '{}'", self.config.name))?;

        self.process = Some(process);
        self.stdin = Some(Mutex::new(stdin));
        self.stdout = Some(Mutex::new(BufReader::new(stdout)));

        tracing::info!("MCP server '{}' spawned ({} {})", self.config.name, self.config.command, self.config.args.join(" "));

        Ok(())
    }

    pub async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "fe",
                "version": "0.1.0"
            }
        });

        let response: Value = self.send_request("initialize", Some(params)).await?;

        let result: InitializeResult = serde_json::from_value(response)
            .map_err(|e| anyhow!("Failed to parse initialize result: {}", e))?;

        self.initialized = true;
        self.server_info = Some(result.server_info.clone());

        tracing::info!("MCP server '{}' initialized (v{}, protocol: {})",
            self.config.name, result.server_info.version, result.protocol_version);

        Ok(result)
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let response: Value = self.send_request("tools/list", None).await?;

        let tools_array = response.get("tools")
            .ok_or_else(|| anyhow!("Missing 'tools' in tools/list response"))?;

        let tools: Vec<McpTool> = serde_json::from_value(tools_array.clone())
            .map_err(|e| anyhow!("Failed to parse tools list: {}", e))?;

        self.tools = tools.clone();
        tracing::info!("MCP server '{}': {} tools available", self.config.name, tools.len());

        Ok(tools)
    }

    pub async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value> {
        let params = serde_json::json!({
            "name": name,
            "arguments": args
        });

        let response: Value = self.send_request("tools/call", Some(params)).await?;

        Ok(response)
    }

    pub async fn list_resources(&mut self) -> Result<Vec<McpResource>> {
        let response: Value = self.send_request("resources/list", None).await?;

        let resources_array = response.get("resources")
            .ok_or_else(|| anyhow!("Missing 'resources' in resources/list response"))?;

        let resources: Vec<McpResource> = serde_json::from_value(resources_array.clone())
            .map_err(|e| anyhow!("Failed to parse resources list: {}", e))?;

        self.resources = resources.clone();

        Ok(resources)
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<String> {
        let params = serde_json::json!({ "uri": uri });
        let response: Value = self.send_request("resources/read", Some(params)).await?;

        let contents = response.get("contents")
            .ok_or_else(|| anyhow!("Missing 'contents' in resources/read response"))?;

        Ok(serde_json::to_string_pretty(contents)?)
    }

    pub async fn list_prompts(&mut self) -> Result<Vec<McpPrompt>> {
        let response: Value = self.send_request("prompts/list", None).await?;

        let prompts_array = response.get("prompts")
            .ok_or_else(|| anyhow!("Missing 'prompts' in prompts/list response"))?;

        let prompts: Vec<McpPrompt> = serde_json::from_value(prompts_array.clone())
            .map_err(|e| anyhow!("Failed to parse prompts list: {}", e))?;

        self.prompts = prompts.clone();
        tracing::info!("MCP server '{}': {} prompts available", self.config.name, prompts.len());

        Ok(prompts)
    }

    pub async fn get_prompt(&mut self, name: &str, args: Option<Value>) -> Result<Vec<McpPromptMessage>> {
        let mut params = serde_json::json!({ "name": name });
        if let Some(a) = args {
            params["arguments"] = a;
        }

        let response: Value = self.send_request("prompts/get", Some(params)).await?;

        let messages = response.get("messages")
            .ok_or_else(|| anyhow!("Missing 'messages' in prompts/get response"))?;

        let msgs: Vec<McpPromptMessage> = serde_json::from_value(messages.clone())
            .map_err(|e| anyhow!("Failed to parse prompt messages: {}", e))?;

        Ok(msgs)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if self.initialized {
            let _ = self.send_request("shutdown", None).await;
        }

        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
            let _ = process.wait().await;
        }

        self.stdin = None;
        self.stdout = None;
        self.initialized = false;

        Ok(())
    }

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);

        let request_json = request.serialize();

        {
            let stdin = self.stdin.as_ref()
                .ok_or_else(|| anyhow!("MCP server '{}' not spawned", self.config.name))?;
            let mut stdin = stdin.lock().await;

            stdin.write_all(request_json.as_bytes()).await
                .map_err(|e| anyhow!("Error writing to MCP server '{}': {}", self.config.name, e))?;
            stdin.write_all(b"\n").await
                .map_err(|e| anyhow!("Error writing newline to MCP server '{}': {}", self.config.name, e))?;
            stdin.flush().await
                .map_err(|e| anyhow!("Error flushing stdin to MCP server '{}': {}", self.config.name, e))?;
        }

        let line = {
            let stdout = self.stdout.as_ref()
                .ok_or_else(|| anyhow!("MCP server '{}' not spawned", self.config.name))?;
            let mut stdout = stdout.lock().await;
            let mut line = String::new();

            let read_result = timeout(Duration::from_secs(30), stdout.read_line(&mut line)).await
                .map_err(|_| anyhow!("Timeout waiting for response from MCP server '{}'", self.config.name))?
                .map_err(|e| anyhow!("Error reading from MCP server '{}': {}", self.config.name, e))?;

            if read_result == 0 {
                return Err(anyhow!("MCP server '{}' closed connection", self.config.name));
            }

            line
        };

        let response: JsonRpcResponse = serde_json::from_str(&line)
            .map_err(|e| anyhow!("Failed to parse JSON-RPC response from MCP server '{}': {} (raw: {})", self.config.name, e, line.trim()))?;

        if let Some(error) = response.error {
            return Err(anyhow!("MCP server '{}' error: [{}] {}", self.config.name, error.code, error.message));
        }

        response.result
            .ok_or_else(|| anyhow!("MCP server '{}' returned empty result", self.config.name))
    }

    pub async fn try_spawn_and_init(&mut self) -> Result<()> {
        self.spawn().await?;
        self.initialize().await?;
        self.list_tools().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_mcp_client_new() {
        let config = McpServerConfig {
            name: "test".into(),
            command: "python3".into(),
            args: vec!["-c", "print('ok')"].into_iter().map(String::from).collect(),
            env: HashMap::new(),
            enabled: true,
        };
        let client = McpClient::new(config);
        assert!(!client.is_initialized());
        assert_eq!(client.server_name(), "test");
    }
}
