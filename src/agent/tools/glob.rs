use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct GlobTool;

impl GlobTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Busca archivos por PATRÓN GLOB específico (ej: '**/*.rs'). NO es para listar directorios. Usa bash con 'ls' para listar contenido de directorios."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "glob",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Patrón glob para buscar archivos"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directorio base para la búsqueda (default: directorio actual)"
                    }
                },
                "required": ["pattern"]
            }
        })
    }

    fn is_destructive(&self, _params: &Value) -> bool {
        false
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let pattern = params
            .get("pattern")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'pattern'"))?;

        let base_path = params
            .get("path")
            .and_then(|c| c.as_str())
            .unwrap_or(".");

        let walker = walkdir::WalkDir::new(base_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file());

        let glob_pattern = globset::Glob::new(pattern)
            .map_err(|e| anyhow::anyhow!("Patrón glob inválido: {}", e))?
            .compile_matcher();

        let mut matches: Vec<String> = walker
            .filter(|e| glob_pattern.is_match(e.path()))
            .map(|e| e.path().display().to_string())
            .collect();

        matches.sort();
        let output = if matches.is_empty() {
            "No se encontraron archivos.".to_string()
        } else {
            matches.join("\n")
        };

        Ok(ToolResult {
            output,
            truncated: false,
            exit_code: Some(if matches.is_empty() { 1 } else { 0 }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tokio::fs;

    fn make_tool() -> GlobTool {
        GlobTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_glob_finds_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "").await.unwrap();
        fs::write(dir.path().join("b.rs"), "").await.unwrap();
        fs::write(dir.path().join("c.txt"), "").await.unwrap();

        let tool = make_tool();
        let params = serde_json::json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("a.rs"));
        assert!(result.output.contains("b.rs"));
        assert!(!result.output.contains("c.txt"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let tool = make_tool();
        let params = serde_json::json!({
            "pattern": "*.nonexistent"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("No se encontraron"));
    }
}
