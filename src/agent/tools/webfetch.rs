use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct WebfetchTool;

impl WebfetchTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebfetchTool {
    fn name(&self) -> &str {
        "webfetch"
    }

    fn description(&self) -> &str {
        "Obtiene el contenido de una URL y lo retorna como texto plano o markdown. Útil para consultar documentación o APIs."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "webfetch",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL completa a obtener"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["markdown", "text", "html"],
                        "description": "Formato de salida (default: markdown)"
                    }
                },
                "required": ["url"]
            }
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        false
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let url = params
            .get("url")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'url'"))?;

        let response = reqwest::get(url).await
            .map_err(|e| anyhow::anyhow!("Error fetching URL '{}': {}", url, e))?;

        let status = response.status();
        let text = response.text().await
            .map_err(|e| anyhow::anyhow!("Error leyendo respuesta: {}", e))?;

        let output = format!("HTTP {}\n\n{}", status, text);

        Ok(ToolResult {
            output,
            truncated: false,
            exit_code: Some(if status.is_success() { 0 } else { 1 }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    fn make_tool() -> WebfetchTool {
        WebfetchTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_webfetch_missing_url() {
        let tool = make_tool();
        let params = serde_json::json!({});
        assert!(tool.execute(params).await.is_err());
    }

    #[tokio::test]
    async fn test_webfetch_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_string("contenido de prueba"))
            .mount(&mock_server)
            .await;

        let tool = make_tool();
        let params = serde_json::json!({
            "url": format!("{}/test", mock_server.uri())
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("contenido de prueba"));
        assert!(result.output.contains("HTTP 200"));
    }
}
