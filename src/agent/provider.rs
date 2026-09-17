use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::{Result, Context};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_stream::StreamExt;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    pub function: ToolFunction,
}

impl ToolCall {
    pub fn normalize(&mut self) {
        if let Value::String(s) = &self.function.arguments {
            if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                self.function.arguments = parsed;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone)]
pub struct ChatResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    options: ChatOptions,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ChatOptions {
    #[serde(rename = "num_ctx")]
    num_ctx: u32,
    temperature: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct ChatResponse {
    message: ChatResponseMessage,
    done: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct ChatResponseMessage {
    role: Option<String>,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn supports_tools(&self) -> bool;
    fn supports_reasoning(&self) -> bool;
    fn max_context(&self) -> u32;
    async fn chat(&self, model: &str, messages: &[Message], tools: &[Value], spinner_done: Option<&AtomicBool>) -> Result<ChatResult>;
    async fn list_models(&self) -> Result<Vec<String>>;
}

pub struct OllamaProvider {
    client: Client,
    host: String,
    max_retries: u32,
    max_context_tokens: u32,
    keep_alive_secs: u64,
}

impl OllamaProvider {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.ollama.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            host: config.ollama.host.clone(),
            max_retries: config.ollama.max_retries,
            max_context_tokens: config.agent.max_context_tokens,
            keep_alive_secs: config.ollama.keep_alive_secs,
        }
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn supports_reasoning(&self) -> bool {
        false
    }

    fn max_context(&self) -> u32 {
        self.max_context_tokens
    }

    async fn chat(&self, model: &str, messages: &[Message], tools: &[Value], spinner_done: Option<&AtomicBool>) -> Result<ChatResult> {
        let request = ChatRequest {
            model: model.to_string(),
            messages: messages.to_vec(),
            stream: true,
            options: ChatOptions {
                num_ctx: self.max_context_tokens,
                temperature: 0.3,
            },
            tools: tools.to_vec(),
            keep_alive: Some(serde_json::json!(self.keep_alive_secs)),
        };

        let url = format!("{}/api/chat", self.host);
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_secs(2u64.pow(attempt));
                tokio::time::sleep(delay).await;
                tracing::warn!("Reintento {} de {}", attempt, self.max_retries);
            }

            match self.send_chat(&url, &request, spinner_done).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    tracing::error!("Error en llamada a Ollama (intento {}): {}", attempt, last_error.as_ref().unwrap());
                }
            }
        }

        Err(last_error.unwrap())
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.host);
        let response = self.client.get(&url).send().await?;
        let body: serde_json::Value = response.json().await?;
        let models = body["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }
}

impl OllamaProvider {
    async fn send_chat(&self, url: &str, request: &ChatRequest, spinner_done: Option<&AtomicBool>) -> Result<ChatResult> {
        let response = self
            .client
            .post(url)
            .json(request)
            .send()
            .await
            .context("Error de conexión con Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama respondió con HTTP {}: {}", status, body);
        }

        if let Some(done) = spinner_done {
            done.store(true, Ordering::Relaxed);
            print!("\r");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }

        let mut full_content = String::new();
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if let Ok(chat_resp) = serde_json::from_str::<ChatResponse>(&line) {
                    if !chat_resp.message.tool_calls.is_empty() {
                        tool_calls = chat_resp.message.tool_calls;
                        for tc in &mut tool_calls {
                            tc.normalize();
                        }
                    }
                    if let Some(content) = chat_resp.message.content {
                        if !content.is_empty() {
                            print!("{}", content);
                            use std::io::Write;
                            std::io::stdout().flush()?;
                            full_content.push_str(&content);
                        }
                    }
                }
            }
        }

        let remaining = buffer.trim();
        if !remaining.is_empty() {
            if let Ok(chat_resp) = serde_json::from_str::<ChatResponse>(remaining) {
                if !chat_resp.message.tool_calls.is_empty() {
                    tool_calls = chat_resp.message.tool_calls;
                    for tc in &mut tool_calls {
                        tc.normalize();
                    }
                }
                if let Some(content) = chat_resp.message.content {
                    if !content.is_empty() {
                        print!("{}", content);
                        use std::io::Write;
                        std::io::stdout().flush()?;
                        full_content.push_str(&content);
                    }
                }
            }
        }

        println!();

        if !tool_calls.is_empty() && full_content.is_empty() {
            full_content = String::new();
        }

        Ok(ChatResult {
            content: full_content,
            tool_calls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_provider_name() {
        let config = make_config("http://localhost:11434");
        let provider = OllamaProvider::new(&config);
        assert_eq!(provider.name(), "ollama");
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
        let provider = OllamaProvider::new(&config);
        let result = provider.chat("test", &[], &[], None).await.unwrap();
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
        let provider = OllamaProvider::new(&config);
        let result = provider.chat("test", &[], &[], None).await.unwrap();
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
        let provider = OllamaProvider::new(&config);
        let result = provider.chat("test", &[], &[], None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_models() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "models": [
                {"name": "qwen2.5:3b"},
                {"name": "gemma3:4b"}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap()
            ))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let provider = OllamaProvider::new(&config);
        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 2);
        assert!(models.contains(&"qwen2.5:3b".to_string()));
    }

    #[tokio::test]
    async fn test_list_models_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = make_config(&mock_server.uri());
        let provider = OllamaProvider::new(&config);
        let result = provider.list_models().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_chat_with_empty_tools() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "No tools needed."
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
        let provider = OllamaProvider::new(&config);
        let result = provider.chat("test", &[], &[], None).await.unwrap();
        assert_eq!(result.content, "No tools needed.");
    }

    #[tokio::test]
    async fn test_chat_retry_on_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let mut config = make_config(&mock_server.uri());
        config.ollama.max_retries = 1;
        let provider = OllamaProvider::new(&config);
        let result = provider.chat("test", &[], &[], None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_provider_supports_tools() {
        let config = make_config("http://localhost:11434");
        let provider = OllamaProvider::new(&config);
        assert!(provider.supports_tools());
    }

    #[tokio::test]
    async fn test_provider_max_context() {
        let config = make_config("http://localhost:11434");
        let provider = OllamaProvider::new(&config);
        assert_eq!(provider.max_context(), 8192);
    }
}
