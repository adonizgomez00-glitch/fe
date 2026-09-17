use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct WriteTool;

impl WriteTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Escribe contenido en un archivo. Si el archivo no existe, lo crea. Si existe, lo sobrescribe."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "write",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Ruta del archivo a escribir (absoluta o relativa al directorio actual)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Contenido a escribir en el archivo"
                    }
                },
                "required": ["file_path", "content"]
            }
        })
    }

    fn is_destructive(&self, params: &Value) -> bool {
        params.get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false)
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let raw_path = params
            .get("file_path")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'file_path'. La tool 'write' requiere los campos: file_path (ruta del archivo) y content (contenido a escribir)."))?;

        let content = params
            .get("content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'content'. La tool 'write' requiere los campos: file_path (ruta del archivo) y content (contenido a escribir)."))?;

        let file_path = std::path::Path::new(raw_path);
        let abs_path = if file_path.is_relative() {
            let cwd = std::env::current_dir()
                .map_err(|e| anyhow::anyhow!("Error obteniendo directorio actual: {}", e))?;
            cwd.join(file_path)
        } else {
            file_path.to_path_buf()
        };

        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| anyhow::anyhow!("Error creando directorio '{}': {}", parent.display(), e))?;
        }

        let mut file = fs::File::create(&abs_path).await
            .map_err(|e| anyhow::anyhow!("Error creando archivo '{}': {}", abs_path.display(), e))?;

        file.write_all(content.as_bytes()).await
            .map_err(|e| anyhow::anyhow!("Error escribiendo archivo '{}': {}", abs_path.display(), e))?;

        Ok(ToolResult {
            output: format!("Archivo escrito: {} ({} bytes)", abs_path.display(), content.len()),
            truncated: false,
            exit_code: Some(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_tool() -> WriteTool {
        WriteTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_write_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_write.txt");

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "content": "contenido de prueba"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("Archivo escrito"));

        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "contenido de prueba");
    }

    #[tokio::test]
    async fn test_write_creates_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("nested").join("test.txt");

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "content": "test"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("Archivo escrito"));
        assert!(path.exists());
    }

    #[test]
    fn test_is_destructive_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nuevo_archivo.txt");
        assert!(!path.exists());

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "content": "test"
        });
        assert!(!tool.is_destructive(&params));
    }

    #[test]
    fn test_is_destructive_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existente.txt");
        std::fs::write(&path, "contenido").unwrap();
        assert!(path.exists());

        let tool = make_tool();
        let params = serde_json::json!({
            "file_path": path.to_str().unwrap(),
            "content": "sobrescrito"
        });
        assert!(tool.is_destructive(&params));
    }

    #[test]
    fn test_is_destructive_missing_path_param() {
        let tool = make_tool();
        let params = serde_json::json!({
            "content": "test"
        });
        assert!(!tool.is_destructive(&params));
    }
}
