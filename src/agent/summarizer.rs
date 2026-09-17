use anyhow::Result;
use reqwest::Client;
use serde::Serialize;
use crate::config::Config;

#[derive(Debug, Serialize)]
struct SummarizeRequest {
    model: String,
    messages: Vec<SummarizeMessage>,
    stream: bool,
    options: SummarizeOptions,
}

#[derive(Debug, Serialize)]
struct SummarizeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct SummarizeOptions {
    num_ctx: u32,
    temperature: f32,
}

#[derive(Debug, serde::Deserialize)]
struct SummarizeResponse {
    message: SummarizeResponseMessage,
    done: bool,
}

#[derive(Debug, serde::Deserialize)]
struct SummarizeResponseMessage {
    content: Option<String>,
}

pub struct Summarizer {
    client: Client,
    host: String,
    model: String,
    num_ctx: u32,
}

impl Summarizer {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            host: config.ollama.host.clone(),
            model: config.models.summarizer.clone(),
            num_ctx: config.agent.max_context_tokens.max(4096),
        }
    }

    pub async fn summarize(
        &self,
        turns_text: &str,
        existing_summary: Option<&str>,
        skills_active: &[String],
    ) -> Result<String> {
        let mut system = String::from(
            "Eres un asistente especializado en resumir sesiones de terminal. \
             Genera un resumen CONCISO (máximo 3 párrafos) que capture:\n\
             - Qué se hizo (comandos ejecutados, archivos creados/modificados)\n\
             - Resultados principales\n\
             - Estado actual del proyecto/sistema\n\n\
             Reglas:\
             - Preserva etiquetas de skill como [skill:nombre-skill] si aparecen\n\
             - Sé específico: incluye nombres de archivos, directorios, comandos clave\n\
             - NO incluyas saludos ni meta-comentarios\n\
             - Responde ÚNICAMENTE con el resumen, sin intro\n\
             - Idioma: español\n"
        );

        if !skills_active.is_empty() {
            system.push_str("\nSkills activas en esta sesión: ");
            system.push_str(&skills_active.join(", "));
            system.push('\n');
        }

        if let Some(summary) = existing_summary {
            system.push_str("\nResumen anterior:\n");
            system.push_str(summary);
            system.push('\n');
        }

        let user_prompt = format!(
            "Resume los siguientes turnos de sesión:\n\n{}",
            turns_text
        );

        let request = SummarizeRequest {
            model: self.model.clone(),
            messages: vec![
                SummarizeMessage {
                    role: "system".into(),
                    content: system,
                },
                SummarizeMessage {
                    role: "user".into(),
                    content: user_prompt,
                },
            ],
            stream: false,
            options: SummarizeOptions {
                num_ctx: self.num_ctx,
                temperature: 0.3,
            },
        };

        let url = format!("{}/api/chat", self.host);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama respondió con HTTP {}: {}", status, body);
        }

        let body: SummarizeResponse = response.json().await?;
        let content = body.message.content.unwrap_or_default().trim().to_string();

        if content.is_empty() {
            anyhow::bail!("Respuesta vacía del modelo de resumen");
        }

        Ok(content)
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[test]
    fn test_summarizer_new() {
        let config = Config::default();
        let summarizer = Summarizer::new(&config);
        assert_eq!(summarizer.model, "gemma3:4b");
        assert_eq!(summarizer.host, "http://localhost:11434");
    }

    #[tokio::test]
    async fn test_summarizer_http_error() {
        let mut config = Config::default();
        config.ollama.host = "http://127.0.0.1:1".into();
        config.ollama.timeout_secs = 2;
        let summarizer = Summarizer::new(&config);
        let result = summarizer.summarize("contenido de prueba", None, &[]).await;
        assert!(result.is_err());
    }

    fn summarizer_config(mock_uri: &str) -> Config {
        let mut config = Config::default();
        config.ollama.host = mock_uri.to_string();
        config.ollama.max_retries = 0;
        config.ollama.timeout_secs = 5;
        config
    }

    #[tokio::test]
    async fn test_summarize_success() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "El usuario ejecutó apt update y apt upgrade. Se actualizaron 15 paquetes del sistema."
            },
            "done": true
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap()
            ))
            .mount(&mock_server)
            .await;

        let config = summarizer_config(&mock_server.uri());
        let summarizer = Summarizer::new(&config);
        let result = summarizer
            .summarize("Turno 1: apt update", None, &[])
            .await
            .unwrap();

        assert!(result.contains("apt update"));
        assert!(result.contains("apt upgrade"));
    }

    #[tokio::test]
    async fn test_summarize_with_skills() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "[skill:C-sysadmin:package-management] apt update ejecutado."
            },
            "done": true
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap()
            ))
            .mount(&mock_server)
            .await;

        let config = summarizer_config(&mock_server.uri());
        let summarizer = Summarizer::new(&config);
        let skills = vec!["C-sysadmin".to_string()];
        let result = summarizer
            .summarize("Turno 1: apt update", None, &skills)
            .await
            .unwrap();

        assert!(result.contains("C-sysadmin"));
    }

    #[tokio::test]
    async fn test_summarize_with_existing_summary() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "Resumen anterior: apt update. Nuevo: apt upgrade -y."
            },
            "done": true
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap()
            ))
            .mount(&mock_server)
            .await;

        let config = summarizer_config(&mock_server.uri());
        let summarizer = Summarizer::new(&config);
        let result = summarizer
            .summarize("Turno 2: apt upgrade -y", Some("Resumen anterior: apt update"), &[])
            .await
            .unwrap();

        assert!(result.contains("apt upgrade -y"));
    }

    #[tokio::test]
    async fn test_summarize_empty_response() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": ""
            },
            "done": true
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap()
            ))
            .mount(&mock_server)
            .await;

        let config = summarizer_config(&mock_server.uri());
        let summarizer = Summarizer::new(&config);
        let result = summarizer.summarize("contenido", None, &[]).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_summarize_http_500() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = summarizer_config(&mock_server.uri());
        let summarizer = Summarizer::new(&config);
        let result = summarizer.summarize("contenido", None, &[]).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_summarize_http_404() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let config = summarizer_config(&mock_server.uri());
        let summarizer = Summarizer::new(&config);
        let result = summarizer.summarize("contenido", None, &[]).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_summarize_detects_empty_response_fields() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "message": {
                "role": "assistant",
                "content": null
            },
            "done": true
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                serde_json::to_string(&mock_response).unwrap()
            ))
            .mount(&mock_server)
            .await;

        let config = summarizer_config(&mock_server.uri());
        let summarizer = Summarizer::new(&config);
        let result = summarizer.summarize("contenido", None, &[]).await;

        assert!(result.is_err());
    }
}
