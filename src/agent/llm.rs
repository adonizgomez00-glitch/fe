use std::sync::{Arc, atomic::AtomicBool};
use anyhow::Result;
use serde_json::Value;

#[allow(unused_imports)]
pub use crate::agent::provider::{Message, ToolCall, ToolFunction, ChatResult, Provider};

pub struct OllamaClient {
    provider: Arc<dyn Provider>,
}

impl OllamaClient {
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self { provider }
    }

    #[allow(dead_code)]
    pub fn provider(&self) -> &dyn Provider {
        self.provider.as_ref()
    }

    pub async fn chat_with_tools(
        &self,
        model: &str,
        messages: &[Message],
        tool_schemas: &[Value],
        spinner_done: Option<&AtomicBool>,
    ) -> Result<ChatResult> {
        self.provider.chat(model, messages, tool_schemas, spinner_done).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::provider::OllamaProvider;
    use crate::config::Config;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    fn make_config(host: &str) -> Config {
        let mut config = Config::default();
        config.ollama.host = host.to_string();
        config.ollama.max_retries = 0;
        config.ollama.timeout_secs = 5;
        config
    }

    #[tokio::test]
    async fn test_chat_with_tools_text_response() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "Respuesta normal."
            },
            "done": true
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap() + "\n"
            ))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let provider = Arc::new(OllamaProvider::new(&config));
        let client = OllamaClient::new(provider);
        let result = client.chat_with_tools("test", &[], &[], None).await.unwrap();
        assert!(result.content.contains("Respuesta normal"));
        assert!(result.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn test_chat_with_tool_calls() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "bash",
                        "arguments": {"command": "echo hello", "description": "test"}
                    }
                }]
            },
            "done": true
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap() + "\n"
            ))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let provider = Arc::new(OllamaProvider::new(&config));
        let client = OllamaClient::new(provider);
        let result = client.chat_with_tools("test", &[], &[], None).await.unwrap();
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].function.name, "bash");
    }

    #[tokio::test]
    async fn test_chat_error_handling() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let provider = Arc::new(OllamaProvider::new(&config));
        let client = OllamaClient::new(provider);
        let result = client.chat_with_tools("test", &[], &[], None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_chat_with_partial_stream() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "Respuesta parcial."
            },
            "done": true
        });

Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap() + "\n"
            ))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let provider = Arc::new(OllamaProvider::new(&config));
        let client = OllamaClient::new(provider);
        let result = client.chat_with_tools("test", &[], &[], None).await.unwrap();
        assert!(result.content.contains("Respuesta parcial"));
    }

    #[tokio::test]
    async fn test_chat_connection_refused() {
        let config = make_config("http://127.0.0.1:1");
        let provider = Arc::new(OllamaProvider::new(&config));
        let client = OllamaClient::new(provider);
        let result = client.chat_with_tools("test", &[], &[], None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_chat_multiple_lines() {
        let mock_server = MockServer::start().await;

        let responses = vec![
            serde_json::json!({"message": {"role": "assistant", "content": "Línea 1"}, "done": false}),
            serde_json::json!({"message": {"role": "assistant", "content": "Línea 2"}, "done": false}),
            serde_json::json!({"message": {"role": "assistant", "content": ""}, "done": true}),
        ];

        let body = responses.iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n") + "\n";

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let provider = Arc::new(OllamaProvider::new(&config));
        let client = OllamaClient::new(provider);
        let result = client.chat_with_tools("test", &[], &[], None).await.unwrap();
        assert!(result.content.contains("Línea 1"));
        assert!(result.content.contains("Línea 2"));
    }
}
