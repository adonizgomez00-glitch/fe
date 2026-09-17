#![allow(dead_code)]
use std::path::PathBuf;
use std::time::Duration;
use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::timeout;

use super::protocol::{
    JsonRpcRequest, JsonRpcResponse, DaemonEvent, PromptParams, PromptFlags,
    HealthResult,
};

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn default_path() -> PathBuf {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("fe");
        data_dir.join("daemon.sock")
    }

    pub fn pid_path() -> PathBuf {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("fe");
        data_dir.join("daemon.pid")
    }

    pub async fn is_running(&self) -> bool {
        if !self.socket_path.exists() {
            return false;
        }
        self.health().await.is_ok()
    }

    pub async fn connect(&self) -> Result<UnixStream> {
        let stream = timeout(Duration::from_secs(5), UnixStream::connect(&self.socket_path)).await??;
        Ok(stream)
    }

    async fn send_single_request(&self, req: &JsonRpcRequest) -> Result<String> {
        let mut stream = self.connect().await?;
        let line = serde_json::to_string(req)?;
        stream.write_all(line.as_bytes()).await?;
        stream.write_all(b"\n").await?;

        let mut reader = BufReader::new(&mut stream);
        let mut line_buf = String::new();
        let bytes_read = timeout(Duration::from_secs(30), reader.read_line(&mut line_buf)).await??;

        if bytes_read == 0 {
            anyhow::bail!("Server closed connection without response");
        }

        Ok(line_buf.trim().to_string())
    }

    async fn send_streaming_request(&self, req: &JsonRpcRequest) -> Result<Vec<String>> {
        let mut stream = self.connect().await?;
        let line = serde_json::to_string(req)?;
        stream.write_all(line.as_bytes()).await?;
        stream.write_all(b"\n").await?;

        let mut reader = BufReader::new(&mut stream);
        let mut line_buf = String::new();
        let mut responses = Vec::new();

        loop {
            line_buf.clear();
            let bytes_read = timeout(Duration::from_secs(120), reader.read_line(&mut line_buf)).await??;
            if bytes_read == 0 {
                break;
            }
            let trimmed = line_buf.trim().to_string();
            if !trimmed.is_empty() {
                let is_terminal = trimmed.contains("\"done\"") || trimmed.contains("\"error\"");
                responses.push(trimmed);
                if is_terminal {
                    break;
                }
            }
        }

        Ok(responses)
    }

    pub async fn execute_prompt(&self, text: &str, flags: PromptFlags) -> Result<()> {
        let req = JsonRpcRequest::prompt(1, PromptParams {
            text: text.to_string(),
            flags,
        });
        let responses = self.send_streaming_request(&req).await?;

        for response in responses {
            let parsed: JsonRpcResponse = serde_json::from_str(&response)?;
            if let Some(result) = parsed.result {
                if let Ok(event) = serde_json::from_value::<DaemonEvent>(result) {
                    match event {
                        DaemonEvent::Token { content } => {
                            print!("{}", content);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        }
                        DaemonEvent::ToolCall { tool, tool_params } => {
                            let description = tool_params
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&tool);
                            println!("\n🔧 {} — {}", tool, description);
                        }
                        DaemonEvent::ToolResult { tool: _, output } => {
                            if !output.trim().is_empty() {
                                println!("{}", output);
                            }
                        }
                        DaemonEvent::Done { .. } => {}
                        DaemonEvent::EventError { code, message } => {
                            eprintln!("\n⚠️ Error {}: {}", code, message);
                        }
                    }
                }
            } else if let Some(error) = parsed.error {
                eprintln!("\n⚠️ Error {}: {}", error.code, error.message);
            }
        }

        Ok(())
    }

    pub async fn health(&self) -> Result<HealthResult> {
        let req = JsonRpcRequest::health(1);
        let response = self.send_single_request(&req).await?;
        let parsed: JsonRpcResponse = serde_json::from_str(&response)?;
        if let Some(result) = parsed.result {
            Ok(serde_json::from_value::<HealthResult>(result)?)
        } else if let Some(error) = parsed.error {
            anyhow::bail!("Health check error {}: {}", error.code, error.message)
        } else {
            anyhow::bail!("Invalid health response")
        }
    }

    pub fn check_pid() -> Option<u32> {
        let pid_path = Self::pid_path();
        if !pid_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(pid_path).ok()?;
        let pid: u32 = content.trim().parse().ok()?;
        Some(pid)
    }

    pub fn save_pid(pid: u32) -> Result<()> {
        let pid_path = Self::pid_path();
        if let Some(parent) = pid_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&pid_path, pid.to_string())?;
        Ok(())
    }

    pub fn remove_pid() {
        let pid_path = Self::pid_path();
        if pid_path.exists() {
            let _ = std::fs::remove_file(pid_path);
        }
    }

    pub fn is_process_alive(pid: u32) -> bool {
        std::fs::metadata(format!("/proc/{}", pid)).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DaemonServer;
    use crate::config::Config;

    fn get_test_socket() -> (PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("test.sock");
        (socket, dir)
    }

    async fn start_and_wait(socket: &PathBuf) {
        let srv = DaemonServer::new(socket.clone(), Config::default());
        tokio::spawn(async move {
            let _ = srv.start().await;
        });
        for _ in 0..50 {
            if socket.exists() {
                // Give the server a moment to start accepting
                tokio::time::sleep(Duration::from_millis(100)).await;
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("Server did not start socket within 5s");
    }

    #[tokio::test]
    async fn test_client_health() {
        let (socket, _dir) = get_test_socket();
        start_and_wait(&socket).await;

        let client = DaemonClient::new(socket.clone());
        // Retry health check with 1s timeout each
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Ok(health) = client.health().await {
                assert_eq!(health.status, "ok");
                assert_eq!(health.version, "0.1.0");
                DaemonServer::stop(&socket).await.unwrap();
                return;
            }
        }
        DaemonServer::stop(&socket).await.unwrap();
        panic!("Client could not reach server after 6s");
    }

    #[tokio::test]
    async fn test_client_is_running() {
        let (socket, _dir) = get_test_socket();
        start_and_wait(&socket).await;

        let client = DaemonClient::new(socket.clone());
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if client.is_running().await {
                assert!(DaemonClient::new(socket.clone()).is_running().await);
                DaemonServer::stop(&socket).await.unwrap();
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert!(!client.is_running().await);
                return;
            }
        }
        DaemonServer::stop(&socket).await.unwrap();
        panic!("Server did not become reachable within 6s");
    }

    #[tokio::test]
    async fn test_client_not_running() {
        let (socket, _dir) = get_test_socket();
        let client = DaemonClient::new(socket);
        assert!(!client.is_running().await);
    }

    #[tokio::test]
    async fn test_client_invalid_method() {
        let (socket, _dir) = get_test_socket();
        start_and_wait(&socket).await;

        let client = DaemonClient::new(socket.clone());
        let req = JsonRpcRequest::new(1, "invalid.method", None);
        let response = client.send_single_request(&req).await.unwrap();

        assert!(response.contains("\"error\""));

        DaemonServer::stop(&socket).await.unwrap();
    }
}
