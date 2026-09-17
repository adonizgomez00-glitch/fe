#![allow(dead_code)]
use std::path::PathBuf;
use anyhow::Result;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::Config;
use super::handler::DaemonHandler;
use super::protocol;

pub struct DaemonServer {
    socket_path: PathBuf,
    handler: DaemonHandler,
}

impl DaemonServer {
    pub fn new(socket_path: PathBuf, config: Config) -> Self {
        let handler = DaemonHandler::new(config);
        Self { socket_path, handler }
    }

    pub async fn start(&self) -> Result<()> {
        let socket_dir = self.socket_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid socket path"))?;
        tokio::fs::create_dir_all(socket_dir).await?;

        if self.socket_path.exists() {
            tokio::fs::remove_file(&self.socket_path).await?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        tracing::info!("Daemon listening on {:?}", self.socket_path);

        loop {
            let (stream, _addr) = listener.accept().await?;
            let socket_path = self.socket_path.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, socket_path).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }

    pub async fn stop(socket_path: &PathBuf) -> Result<()> {
        if socket_path.exists() {
            tokio::fs::remove_file(socket_path).await?;
            tracing::info!("Daemon socket removed: {:?}", socket_path);
        }
        Ok(())
    }
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    _socket_path: PathBuf,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        let bytes_read = buf_reader.read_line(&mut line_buf).await?;
        if bytes_read == 0 {
            break;
        }

        let line = line_buf.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let req = match protocol::parse_request(&line) {
            Ok(r) => r,
            Err(err_resp) => {
                let json = protocol::serialize_response(&err_resp);
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                continue;
            }
        };

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        let handler_ref = DaemonHandler::new(
            Config::load().unwrap_or_default()
        );
        tokio::spawn(async move {
            handler_ref.handle(req, tx).await;
        });

        while let Some(response) = rx.recv().await {
            writer.write_all(response.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn test_server_binds_and_stops() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("daemon.sock");
        let config = Config::default();
        let server = DaemonServer::new(socket_path.clone(), config);

        tokio::spawn(async move {
            let _ = server.start().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(socket_path.exists(), "Socket file should exist after bind");

        DaemonServer::stop(&socket_path).await.unwrap();
        assert!(!socket_path.exists(), "Socket file should be removed after stop");
    }

    #[tokio::test]
    async fn test_server_receives_and_responds() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("daemon2.sock");
        let config = Config::default();
        let server = DaemonServer::new(socket_path.clone(), config);

        tokio::spawn(async move {
            let _ = server.start().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        let health_req = r#"{"jsonrpc":"2.0","id":1,"method":"health"}"#;
        stream.write_all(health_req.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        let mut buf = vec![0u8; 4096];
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            stream.read(&mut buf),
        ).await
        .expect("Timeout reading response")
        .expect("Read failed");
        let response = String::from_utf8_lossy(&buf[..n]);

        assert!(!response.is_empty(), "Should receive a response");
        assert!(response.contains("\"id\":1"));

        DaemonServer::stop(&socket_path).await.unwrap();
    }

    #[tokio::test]
    async fn test_server_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("daemon3.sock");
        let config = Config::default();
        let server = DaemonServer::new(socket_path.clone(), config);

        tokio::spawn(async move {
            let _ = server.start().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        stream.write_all(b"not json at all\n").await.unwrap();

        let mut buf = vec![0u8; 4096];
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            stream.read(&mut buf),
        ).await
        .expect("Timeout reading response")
        .expect("Read failed");
        let response = String::from_utf8_lossy(&buf[..n]);

        assert!(!response.is_empty(), "Should receive error response for invalid JSON");
        assert!(response.contains("parse error") || response.contains("-32700"));

        DaemonServer::stop(&socket_path).await.unwrap();
    }
}
