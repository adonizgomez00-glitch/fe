use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct WebsearchTool;

impl WebsearchTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebsearchTool {
    fn name(&self) -> &str {
        "websearch"
    }

    fn description(&self) -> &str {
        "Realiza una búsqueda web y retorna resultados relevantes. Útil para consultar información actualizada."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "websearch",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Consulta de búsqueda"
                    },
                    "num_results": {
                        "type": "integer",
                        "description": "Número de resultados (default: 5)"
                    }
                },
                "required": ["query"]
            }
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        false
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let query = params
            .get("query")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'query'"))?;

        tracing::info!("Búsqueda web: {}", query);

        Ok(ToolResult {
            output: format!("Resultados de búsqueda para: '{}'\n(implementación pendiente)", query),
            truncated: false,
            exit_code: Some(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_tool() -> WebsearchTool {
        WebsearchTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_websearch_missing_query() {
        let tool = make_tool();
        let params = serde_json::json!({});
        assert!(tool.execute(params).await.is_err());
    }

    #[tokio::test]
    async fn test_websearch_returns_query() {
        let tool = make_tool();
        let params = serde_json::json!({
            "query": "rust programming"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("rust programming"));
    }
}
