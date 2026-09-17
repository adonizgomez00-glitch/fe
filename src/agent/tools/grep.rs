use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;
use super::{Tool, ToolResult};
use crate::config::Config;
use walkdir::WalkDir;

pub struct GrepTool;

impl GrepTool {
    pub fn new(_config: &Config) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Busca texto en archivos usando expresiones regulares. Retorna archivos y líneas coincidentes."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "grep",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Expresión regular a buscar"
                    },
                    "include": {
                        "type": "string",
                        "description": "Patrón glob para filtrar archivos (ej: '*.rs', '*.{ts,tsx}')"
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

        let include = params.get("include").and_then(|c| c.as_str());
        let base_path = params.get("path").and_then(|c| c.as_str()).unwrap_or(".");

        let regex = regex::Regex::new(pattern)
            .map_err(|e| anyhow::anyhow!("Expresión regular inválida: {}", e))?;

        let glob_matcher = include
            .map(|p| globset::Glob::new(p).ok().map(|g| g.compile_matcher()))
            .flatten();

        let mut results: Vec<String> = Vec::new();

        for entry in WalkDir::new(base_path).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            if let Some(ref matcher) = glob_matcher {
                if !matcher.is_match(entry.path()) {
                    continue;
                }
            }

            let content = fs::read_to_string(entry.path()).await;
            let content = match content {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_num, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    results.push(format!("{}:{}: {}", entry.path().display(), line_num + 1, line.trim()));
                }
            }
        }

        let output = if results.is_empty() {
            "No se encontraron coincidencias.".to_string()
        } else {
            results.join("\n")
        };

        Ok(ToolResult {
            output,
            truncated: false,
            exit_code: Some(if results.is_empty() { 1 } else { 0 }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tokio::fs;

    fn make_tool() -> GrepTool {
        GrepTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_grep_finds_pattern() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {\n    println!(\"hello\");\n}").await.unwrap();

        let tool = make_tool();
        let params = serde_json::json!({
            "pattern": "println",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("println"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = tempfile::tempdir().unwrap();
        let tool = make_tool();
        let params = serde_json::json!({
            "pattern": "NONEXISTENT_PATTERN_12345_XYZ",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("No se encontraron"));
    }
}
