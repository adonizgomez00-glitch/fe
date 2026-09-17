use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct MkdirTool;

impl MkdirTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for MkdirTool {
    fn name(&self) -> &str {
        "mkdir"
    }

    fn description(&self) -> &str {
        "Crea uno o más directorios. Crea directorios padre si no existen (equivalente a mkdir -p)."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "mkdir",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Ruta del directorio a crear"
                    },
                    "description": {
                        "type": "string",
                        "description": "Explicación de qué directorio se crea"
                    }
                },
                "required": ["path", "description"]
            }
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        false
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let path = params
            .get("path")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'path'"))?;

        fs::create_dir_all(path).await
            .map_err(|e| anyhow::anyhow!("Error creando directorio '{}': {}", path, e))?;

        Ok(ToolResult {
            output: format!("Directorio creado: {}", path),
            truncated: false,
            exit_code: Some(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_tool() -> MkdirTool {
        MkdirTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_mkdir_creates_directory() {
        let tmp = std::env::temp_dir().join(format!("fe-test-mkdir-{}", std::process::id()));
        let path_str = tmp.to_string_lossy().to_string();

        let tool = make_tool();
        let params = serde_json::json!({
            "path": path_str,
            "description": "test"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("Directorio creado"));
        assert!(tmp.exists());
        assert!(tmp.is_dir());

        std::fs::remove_dir(&tmp).ok();
    }

    #[tokio::test]
    async fn test_mkdir_nested() {
        let tmp = std::env::temp_dir().join(format!("fe-test-mkdir-nested-{}", std::process::id()));
        let nested = tmp.join("a").join("b").join("c");
        let path_str = nested.to_string_lossy().to_string();

        let tool = make_tool();
        let params = serde_json::json!({
            "path": path_str,
            "description": "test nested"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("Directorio creado"));
        assert!(nested.exists());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[tokio::test]
    async fn test_mkdir_missing_path() {
        let tool = make_tool();
        let params = serde_json::json!({});
        assert!(tool.execute(params).await.is_err());
    }
}
