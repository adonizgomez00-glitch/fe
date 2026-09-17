#![allow(dead_code)]
use std::path::PathBuf;
use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::agent::session::SessionManager;
use crate::agent::core::Agent;
use crate::cli::GlobalFlags;
use crate::config::Config;

use super::protocol::{
    self, DaemonMethod, DaemonEvent, JsonRpcRequest, JsonRpcResponse,
    PromptParams, error_codes, HealthResult, SessionShowParams,
};

pub struct DaemonHandler {
    config: Config,
    sessions_dir: PathBuf,
}

impl DaemonHandler {
    pub fn new(config: Config) -> Self {
        let sessions_dir = config.paths.sessions_dir.clone();
        Self { config, sessions_dir }
    }

    pub async fn handle(
        &self,
        req: JsonRpcRequest,
        tx: mpsc::UnboundedSender<String>,
    ) {
        let id = req.id;
        let method = match req.parse_method() {
            Ok(m) => m,
            Err(_) => {
                let resp = JsonRpcResponse::error(id, error_codes::METHOD_NOT_FOUND, "Method not found", None);
                let _ = tx.send(protocol::serialize_response(&resp));
                return;
            }
        };

        let result = match method {
            DaemonMethod::Prompt => self.handle_prompt(id, req.params, tx.clone()).await,
            DaemonMethod::SessionList => self.handle_session_list(id, &tx).await,
            DaemonMethod::SessionShow => self.handle_session_show(id, req.params, &tx).await,
            DaemonMethod::ConfigGet => self.handle_config_get(id, &tx).await,
            DaemonMethod::Health => self.handle_health(id, &tx).await,
            _ => {
                let resp = JsonRpcResponse::error(id, error_codes::METHOD_NOT_IMPLEMENTED, "Method not implemented in daemon mode", None);
                let _ = tx.send(protocol::serialize_response(&resp));
                return;
            }
        };

        if let Err(e) = result {
            let resp = JsonRpcResponse::error(id, error_codes::INTERNAL_ERROR, &e.to_string(), None);
            let _ = tx.send(protocol::serialize_response(&resp));
        }
    }

    async fn handle_prompt(
        &self,
        id: u64,
        params: Option<Value>,
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        let params: PromptParams = params
            .and_then(|p| serde_json::from_value::<PromptParams>(p).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid prompt params"))?;

        let flags = GlobalFlags {
            batch: params.flags.batch,
            reasoning: params.flags.reasoning,
            fast: params.flags.fast,
            dry_run: params.flags.dry_run,
            destructive: params.flags.destructive,
            checkpoint: params.flags.checkpoint,
            daemon: None,
        };

        let mut agent = Agent::new(self.config.clone(), flags).await;

        let _output = match agent.run(&params.text).await {
            Ok(()) => "OK".to_string(),
            Err(e) => {
                let event = DaemonEvent::EventError {
                    code: error_codes::INTERNAL_ERROR,
                    message: e.to_string(),
                };
                let _ = tx.send(event.to_json_line(id));
                return Ok(());
            }
        };

        let session_id = agent.get_session_id();
        let done = DaemonEvent::Done { session_id };
        let _ = tx.send(done.to_json_line(id));

        Ok(())
    }

    async fn handle_session_list(&self, id: u64, tx: &mpsc::UnboundedSender<String>) -> Result<()> {
        let manager = SessionManager::new(&self.sessions_dir);
        let sessions = manager.list()?;

        let result: Vec<Value> = sessions.iter().map(|s| {
            serde_json::json!({
                "id": s.id,
                "start_time": s.start_time,
                "model": s.model,
                "skills": s.skills,
                "turn_count": s.turn_count,
                "preview": s.preview,
            })
        }).collect();

        let resp = JsonRpcResponse::success(id, serde_json::json!({ "sessions": result }));
        let _ = tx.send(protocol::serialize_response(&resp));
        Ok(())
    }

    async fn handle_session_show(&self, id: u64, params: Option<Value>, tx: &mpsc::UnboundedSender<String>) -> Result<()> {
        let params: SessionShowParams = params
            .and_then(|p| serde_json::from_value::<SessionShowParams>(p).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid session.show params"))?;

        let manager = SessionManager::new(&self.sessions_dir);
        match manager.show(&params.id)? {
            Some(content) => {
                let resp = JsonRpcResponse::success(id, serde_json::json!({ "content": content }));
                let _ = tx.send(protocol::serialize_response(&resp));
            }
            None => {
                let resp = JsonRpcResponse::error(id, error_codes::SESSION_NOT_FOUND, "Session not found", None);
                let _ = tx.send(protocol::serialize_response(&resp));
            }
        }
        Ok(())
    }

    async fn handle_config_get(&self, id: u64, tx: &mpsc::UnboundedSender<String>) -> Result<()> {
        let config_json = serde_json::to_value(&self.config)?;
        let resp = JsonRpcResponse::success(id, serde_json::json!({ "config": config_json }));
        let _ = tx.send(protocol::serialize_response(&resp));
        Ok(())
    }

    async fn handle_health(&self, id: u64, tx: &mpsc::UnboundedSender<String>) -> Result<()> {
        let ollama_connected = self.check_ollama().await;
        let manager = SessionManager::new(&self.sessions_dir);
        let sessions_count = manager.list().ok().map(|l| l.len());

        let result = HealthResult {
            status: "ok".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ollama_connected,
            sessions_count,
            skills_count: None,
        };

        let resp = JsonRpcResponse::success(id, serde_json::to_value(result)?);
        let _ = tx.send(protocol::serialize_response(&resp));
        Ok(())
    }

    async fn check_ollama(&self) -> bool {
        let host = &self.config.ollama.host;
        reqwest::get(format!("{}/api/tags", host))
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn make_handler() -> DaemonHandler {
        DaemonHandler::new(Config::default())
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let handler = make_handler();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let req = JsonRpcRequest::new(1, "unknown.method", None);

        handler.handle(req, tx).await;

        if let Some(response) = rx.recv().await {
            assert!(response.contains("\"code\":-32601") || response.contains("\"code\":-32002"));
        }
    }

    #[tokio::test]
    async fn test_handle_health() {
        let handler = make_handler();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let req = JsonRpcRequest::health(1);

        handler.handle(req, tx).await;

        if let Some(response) = rx.recv().await {
            assert!(response.contains("\"id\":1"));
            assert!(response.contains("\"status\":\"ok\""));
            assert!(response.contains("\"version\""));
        }
    }

    #[tokio::test]
    async fn test_handle_session_list_empty() {
        let handler = DaemonHandler {
            sessions_dir: std::env::temp_dir().join(format!("fe_test_{}", uuid::Uuid::new_v4())),
            ..make_handler()
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let req = JsonRpcRequest::session_list(1);

        handler.handle(req, tx).await;

        if let Some(response) = rx.recv().await {
            assert!(response.contains("\"sessions\""));
            assert!(response.contains("\"id\":1"));
        }
    }

    #[tokio::test]
    async fn test_handle_session_show_missing_params() {
        let handler = make_handler();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let req = JsonRpcRequest::new(1, "session.show", None);

        handler.handle(req, tx).await;

        if let Some(response) = rx.recv().await {
            assert!(response.contains("\"error\""));
        }
    }

    #[tokio::test]
    async fn test_handle_session_show_not_found() {
        let handler = make_handler();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let params = serde_json::json!({"id": "nonexistent-session-id"});
        let req = JsonRpcRequest::new(1, "session.show", Some(params));

        handler.handle(req, tx).await;

        if let Some(response) = rx.recv().await {
            assert!(response.contains("Session not found"));
        }
    }

    #[tokio::test]
    async fn test_handle_config_get() {
        let handler = make_handler();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let req = JsonRpcRequest::config_get(1);

        handler.handle(req, tx).await;

        if let Some(response) = rx.recv().await {
            assert!(response.contains("\"config\""));
            assert!(response.contains("\"ollama\""));
            assert!(response.contains("\"id\":1"));
        }
    }

    #[tokio::test]
    async fn test_handle_implemented_methods_dont_return_error() {
        let handler = make_handler();
        let methods = vec![
            JsonRpcRequest::health(1),
            JsonRpcRequest::config_get(2),
        ];

        for req in methods {
            let (tx, mut rx) = mpsc::unbounded_channel();
            handler.handle(req, tx).await;
            if let Some(response) = rx.recv().await {
                let parsed: JsonRpcResponse = serde_json::from_str(&response).unwrap();
                assert!(parsed.error.is_none() || !response.contains("-32603"),
                    "Method should not return internal error: {}", response);
            }
        }
    }

    #[tokio::test]
    async fn test_handle_invalid_prompt_params() {
        let handler = make_handler();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let req = JsonRpcRequest::new(1, "prompt", Some(serde_json::json!({"invalid": "data"})));

        handler.handle(req, tx).await;

        if let Some(response) = rx.recv().await {
            assert!(response.contains("\"error\""));
        }
    }
}
