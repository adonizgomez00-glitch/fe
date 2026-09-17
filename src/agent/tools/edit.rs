use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct EditTool;

impl EditTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edita un archivo reemplazando texto específico (oldString) con texto nuevo (newString). Útil para modificaciones precisas sin reescribir todo el archivo."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "edit",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Ruta absoluta del archivo a editar"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "Texto exacto a reemplazar"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "Texto de reemplazo"
                    }
                },
                "required": ["file_path", "old_string", "new_string"]
            }
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        true
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let file_path = params
            .get("file_path")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'file_path'"))?;

        let old_string = params
            .get("old_string")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'old_string'"))?;

        let new_string = params
            .get("new_string")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'new_string'"))?;

        let content = fs::read_to_string(file_path).await
            .map_err(|e| anyhow::anyhow!("Error leyendo '{}': {}", file_path, e))?;

        if !content.contains(old_string) {
            anyhow::bail!("old_string no encontrado en '{}'", file_path);
        }

        if content.matches(old_string).count() > 1 && !params.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false) {
            let occurrences: Vec<_> = content.match_indices(old_string).collect();
            anyhow::bail!(
                "old_string encontrado {} veces. Usa 'replace_all': true para reemplazar todas las ocurrencias.",
                occurrences.len()
            );
        }

        let new_content = if params.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false) {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        fs::write(file_path, &new_content).await
            .map_err(|e| anyhow::anyhow!("Error escribiendo '{}': {}", file_path, e))?;

        let changes = content.matches(old_string).count();
        Ok(ToolResult {
            output: format!("Archivo editado: {} ({} reemplazo{})", file_path, changes, if changes == 1 { "" } else { "s" }),
            truncated: false,
            exit_code: Some(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tokio::fs;

    fn make_tool() -> EditTool {
        EditTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_edit_single_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hola mundo").await.unwrap();

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "old_string": "mundo",
            "new_string": "fe"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("Archivo editado"));

        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hola fe");
    }

    #[tokio::test]
    async fn test_edit_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hola mundo").await.unwrap();

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "old_string": "xyz",
            "new_string": "fe"
        });
        assert!(tool.execute(params).await.is_err());
    }

    #[tokio::test]
    async fn test_edit_multiple_occurrences_fails_without_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "foo foo foo").await.unwrap();

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "old_string": "foo",
            "new_string": "bar"
        });
        assert!(tool.execute(params).await.is_err());
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "foo foo foo").await.unwrap();

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "old_string": "foo",
            "new_string": "bar",
            "replace_all": true
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("3 reemplazos"));

        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "bar bar bar");
    }
}
