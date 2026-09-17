use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct ReadTool;

impl ReadTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Lee el contenido de un archivo. Soporta offset (línea inicial) y limit (máximo de líneas)."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "read",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Ruta absoluta del archivo a leer"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Línea inicial (1-indexed, default: 1)",
                        "default": 1
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Máximo de líneas a leer (default: 2000)",
                        "default": 2000
                    }
                },
                "required": ["file_path"]
            }
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        false
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params
            .get("file_path")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'file_path'"))?;

        let offset = params.get("offset").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
        let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(2000) as usize;

        let content = fs::read_to_string(file_path).await
            .map_err(|e| anyhow::anyhow!("Error leyendo '{}': {}", file_path, e))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start = offset.saturating_sub(1);
        let end = (start + limit).min(total_lines);

        let truncated = end < total_lines;
        let selected: Vec<&str> = lines[start..end].to_vec();

        let mut output = String::new();
        for (i, line) in selected.iter().enumerate() {
            output.push_str(&format!("{}: {}\n", start + i + 1, line));
        }

        if truncated {
            output.push_str(&format!(
                "... ({} líneas de {})",
                end - start,
                total_lines
            ));
        }

        Ok(ToolResult {
            output,
            truncated,
            exit_code: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_tool() -> ReadTool {
        ReadTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let tool = make_tool();
        let params = serde_json::json!({"file_path": "/tmp/nonexistent_12345.txt"});
        let result = tool.execute(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let content: Vec<String> = (1..=10).map(|i| format!("linea {}", i)).collect();
        fs::write(&path, content.join("\n")).await.unwrap();

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "offset": 2,
            "limit": 3
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("2: linea 2"));
        assert!(result.output.contains("4: linea 4"));
        assert!(!result.output.contains("1: linea 1"));
        assert!(!result.output.contains("5: linea 5"));
    }
}
