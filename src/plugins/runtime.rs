use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use tokio::sync::Mutex;

use super::manifest::PluginManifest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolResponse {
    pub output: String,
    #[serde(default)]
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcErrorValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcErrorValue {
    code: i64,
    message: String,
}

pub struct PluginRuntime {
    manifest: PluginManifest,
    plugin_dir: Option<std::path::PathBuf>,
    process: Option<Child>,
    stdin: Option<Mutex<ChildStdin>>,
    stdout: Option<Mutex<BufReader<ChildStdout>>>,
    request_id: AtomicU64,
    spawned: bool,
}

impl Drop for PluginRuntime {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.start_kill();
        }
    }
}

impl PluginRuntime {
    pub fn new(manifest: PluginManifest, plugin_dir: Option<std::path::PathBuf>) -> Self {
        Self {
            manifest,
            plugin_dir,
            process: None,
            stdin: None,
            stdout: None,
            request_id: AtomicU64::new(1),
            spawned: false,
        }
    }

    pub fn plugin_name(&self) -> &str {
        &self.manifest.name
    }

    pub fn is_spawned(&self) -> bool {
        self.spawned
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub async fn spawn(&mut self) -> Result<()> {
        if self.spawned {
            return Ok(());
        }

        let mut cmd = Command::new(&self.manifest.command);
        cmd.args(&self.manifest.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (key, val) in &self.manifest.env {
            cmd.env(key, val);
        }

        if let Some(ref dir) = self.plugin_dir {
            cmd.current_dir(dir);
            cmd.env("FE_PLUGIN_DIR", dir.to_string_lossy().as_ref());
        }

        let mut process = cmd.spawn()
            .map_err(|e| anyhow!("Error spawning plugin '{}': {}", self.manifest.name, e))?;

        let stdin = process.stdin.take()
            .ok_or_else(|| anyhow!("Failed to open stdin for plugin '{}'", self.manifest.name))?;
        let stdout = process.stdout.take()
            .ok_or_else(|| anyhow!("Failed to open stdout for plugin '{}'", self.manifest.name))?;

        self.process = Some(process);
        self.stdin = Some(Mutex::new(stdin));
        self.stdout = Some(Mutex::new(BufReader::new(stdout)));
        self.spawned = true;

        tracing::info!("Plugin '{}' spawned ({} {})", self.manifest.name, self.manifest.command, self.manifest.args.join(" "));

        Ok(())
    }

    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<PluginToolResponse> {
        let params = serde_json::json!({
            "tool": tool_name,
            "arguments": args
        });

        let response: Value = self.send_request("tool.call", Some(params)).await?;

        let tool_response: PluginToolResponse = serde_json::from_value(response)
            .map_err(|e| anyhow!("Failed to parse plugin tool response: {}", e))?;

        Ok(tool_response)
    }

    pub async fn call_command(&self, command_name: &str, args: Value) -> Result<PluginToolResponse> {
        let params = serde_json::json!({
            "command": command_name,
            "arguments": args
        });

        let response: Value = self.send_request("command.call", Some(params)).await?;

        let cmd_response: PluginToolResponse = serde_json::from_value(response)
            .map_err(|e| anyhow!("Failed to parse plugin command response: {}", e))?;

        Ok(cmd_response)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.send_request("shutdown", None).await;

        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
            let _ = process.wait().await;
        }

        self.stdin = None;
        self.stdout = None;
        self.spawned = false;

        Ok(())
    }

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params,
        };

        let request_json = serde_json::to_string(&request)?;

        {
            let stdin = self.stdin.as_ref()
                .ok_or_else(|| anyhow!("Plugin '{}' not spawned", self.manifest.name))?;
            let mut stdin = stdin.lock().await;

            stdin.write_all(request_json.as_bytes()).await
                .map_err(|e| anyhow!("Error writing to plugin '{}': {}", self.manifest.name, e))?;
            stdin.write_all(b"\n").await
                .map_err(|e| anyhow!("Error writing newline to plugin '{}': {}", self.manifest.name, e))?;
            stdin.flush().await
                .map_err(|e| anyhow!("Error flushing stdin for plugin '{}': {}", self.manifest.name, e))?;
        }

        let line = {
            let stdout = self.stdout.as_ref()
                .ok_or_else(|| anyhow!("Plugin '{}' not spawned", self.manifest.name))?;
            let mut stdout = stdout.lock().await;
            let mut line = String::new();

            let read_result = timeout(Duration::from_secs(30), stdout.read_line(&mut line)).await
                .map_err(|_| anyhow!("Timeout waiting for response from plugin '{}'", self.manifest.name))?
                .map_err(|e| anyhow!("Error reading from plugin '{}': {}", self.manifest.name, e))?;

            if read_result == 0 {
                return Err(anyhow!("Plugin '{}' closed connection", self.manifest.name));
            }

            line
        };

        let response: JsonRpcResponse = serde_json::from_str(&line)
            .map_err(|e| anyhow!("Failed to parse JSON-RPC response from plugin '{}': {} (raw: {})", self.manifest.name, e, line.trim()))?;

        if let Some(error) = response.error {
            return Err(anyhow!("Plugin '{}' error: [{}] {}", self.manifest.name, error.code, error.message));
        }

        response.result
            .ok_or_else(|| anyhow!("Plugin '{}' returned empty result", self.manifest.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn create_test_plugin_script(dir: &Path, name: &str) -> std::path::PathBuf {
        let plugin_dir = dir.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();

        let script = r#"#!/usr/bin/env python3
import sys, json
for line in sys.stdin:
    req = json.loads(line)
    if req["method"] == "tool.call":
        tool = req["params"]["tool"]
        args = req["params"].get("arguments", {})
        if tool == "echo":
            result = {"output": f"Plugin echo: {args.get('msg', '')}", "exit_code": 0}
        else:
            result = {"output": f"Unknown tool: {tool}", "exit_code": 1}
    elif req["method"] == "shutdown":
        break
    else:
        result = {"output": "unknown method", "exit_code": 1}
    resp = {"jsonrpc": "2.0", "id": req["id"], "result": result}
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()
"#;

        let script_path = plugin_dir.join("plugin.py");
        fs::write(&script_path, script).unwrap();

        let manifest = format!(
            r#"
name = "{name}"
version = "1.0.0"
description = "Plugin {name}"
command = "python3"
args = ["plugin.py"]

[[tools]]
name = "echo"
description = "Echo test"
params = {{ type = "object", properties = {{ msg = {{ type = "string" }} }}, required = ["msg"] }}
"#
        );
        fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();

        plugin_dir
    }

    #[tokio::test]
    async fn test_plugin_runtime_spawn_and_call() {
        let dir = std::env::temp_dir().join(format!("fe_plugin_runtime_test_{}", uuid::Uuid::new_v4()));
        let plugin_dir = create_test_plugin_script(&dir, "test-plugin");

        let manifest = crate::plugins::manifest::PluginManifest::from_path(&plugin_dir.join("plugin.toml")).unwrap();
        let mut runtime = PluginRuntime::new(manifest, Some(plugin_dir.clone()));
        runtime.spawn().await.unwrap();

        let result = runtime.call_tool("echo", serde_json::json!({"msg": "hello"})).await.unwrap();
        assert_eq!(result.output, "Plugin echo: hello");
        assert_eq!(result.exit_code, 0);

        runtime.shutdown().await.unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_plugin_runtime_spawn_twice_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("fe_plugin_runtime_test_{}", uuid::Uuid::new_v4()));
        let plugin_dir = create_test_plugin_script(&dir, "test-plugin");

        let manifest = crate::plugins::manifest::PluginManifest::from_path(&plugin_dir.join("plugin.toml")).unwrap();
        let mut runtime = PluginRuntime::new(manifest, Some(plugin_dir.clone()));
        runtime.spawn().await.unwrap();
        runtime.spawn().await.unwrap();

        assert!(runtime.is_spawned());
        runtime.shutdown().await.unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_plugin_runtime_unknown_tool() {
        let dir = std::env::temp_dir().join(format!("fe_plugin_runtime_test_{}", uuid::Uuid::new_v4()));
        let plugin_dir = create_test_plugin_script(&dir, "test-plugin");

        let manifest = crate::plugins::manifest::PluginManifest::from_path(&plugin_dir.join("plugin.toml")).unwrap();
        let mut runtime = PluginRuntime::new(manifest, Some(plugin_dir.clone()));
        runtime.spawn().await.unwrap();

        let result = runtime.call_tool("nonexistent", serde_json::json!({})).await.unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.output.contains("Unknown tool"));

        runtime.shutdown().await.unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_plugin_runtime_shutdown_cleanup() {
        let dir = std::env::temp_dir().join(format!("fe_plugin_runtime_test_{}", uuid::Uuid::new_v4()));
        let plugin_dir = create_test_plugin_script(&dir, "test-plugin");

        let manifest = crate::plugins::manifest::PluginManifest::from_path(&plugin_dir.join("plugin.toml")).unwrap();
        let mut runtime = PluginRuntime::new(manifest, Some(plugin_dir.clone()));
        runtime.spawn().await.unwrap();
        runtime.shutdown().await.unwrap();

        assert!(!runtime.is_spawned());
        fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn test_plugin_runtime_call_after_shutdown_fails() {
        let dir = std::env::temp_dir().join(format!("fe_plugin_runtime_test_{}", uuid::Uuid::new_v4()));
        let plugin_dir = create_test_plugin_script(&dir, "test-plugin");

        let manifest = crate::plugins::manifest::PluginManifest::from_path(&plugin_dir.join("plugin.toml")).unwrap();
        let mut runtime = PluginRuntime::new(manifest, Some(plugin_dir.clone()));
        runtime.spawn().await.unwrap();
        runtime.shutdown().await.unwrap();

        let result = runtime.call_tool("echo", serde_json::json!({"msg": "hi"})).await;
        assert!(result.is_err());
        fs::remove_dir_all(&dir).ok();
    }
}
