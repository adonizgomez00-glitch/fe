use std::time::Duration;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct TaskTool {
    timeout_secs: u64,
    max_output_lines: u32,
    max_output_bytes: u64,
}

impl TaskTool {
    pub fn new(config: &Config) -> Self {
        Self {
            timeout_secs: config.tools.bash_timeout_secs,
            max_output_lines: config.tools.max_output_lines,
            max_output_bytes: config.tools.max_output_bytes,
        }
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Ejecuta una tarea de desarrollo o administración. Acepta un comando bash para ejecutar. Útil para tareas que requieren múltiples pasos o instalación de dependencias."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "task",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Comando bash a ejecutar para realizar la tarea"
                    },
                    "description": {
                        "type": "string",
                        "description": "Descripción de la tarea que se va a ejecutar"
                    }
                },
                "required": ["command", "description"]
            }
        })
    }

    fn is_destructive(&self, params: &Value) -> bool {
        let cmd = params.get("command").and_then(|c| c.as_str()).unwrap_or("");
        let lower = cmd.to_lowercase();
        let destructive_keywords = [
            "rm ", "rm -rf", "dd ", "mkfs", "format", "> /dev/",
            "chmod 0", "chown", "sudo ", "passwd",
        ];
        destructive_keywords.iter().any(|kw| lower.contains(kw))
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let command = params
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'command'. La tool 'task' requiere un comando bash para ejecutar."))?;

        tracing::info!("Ejecutando tarea: {}", command);

        let output = timeout(
            Duration::from_secs(self.timeout_secs),
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Timeout después de {}s ejecutando tarea: {}", self.timeout_secs, command))?
        .map_err(|e| anyhow::anyhow!("Error ejecutando tarea: {}", e))?;

        let exit_code = output.status.code();
        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let total_lines = stdout.lines().count();
        let total_bytes = stdout.len();
        let truncated = total_lines > self.max_output_lines as usize
            || total_bytes > self.max_output_bytes as usize;

        if truncated {
            let truncated_lines: Vec<String> = stdout
                .lines()
                .take(self.max_output_lines as usize)
                .map(|s| s.to_string())
                .collect();
            stdout = truncated_lines.join("\n");
            stdout.push_str(&format!(
                "\n... (output truncado: {} líneas, máximo {})",
                total_lines,
                self.max_output_lines
            ));
        }

        let mut output = stdout;
        if !stderr.is_empty() {
            output.push_str("\n--- stderr ---\n");
            output.push_str(&stderr);
        }

        Ok(ToolResult {
            output,
            truncated,
            exit_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_tool() -> TaskTool {
        TaskTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_task_executes_command() {
        let tool = make_tool();
        let params = serde_json::json!({
            "command": "echo 'tarea completada'",
            "description": "tarea de prueba"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("tarea completada"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_task_missing_command() {
        let tool = make_tool();
        let params = serde_json::json!({
            "description": "tarea sin comando"
        });
        assert!(tool.execute(params).await.is_err());
    }

    #[test]
    fn test_is_destructive_rm() {
        let tool = make_tool();
        let params = serde_json::json!({
            "command": "rm -rf /tmp/test",
            "description": "peligroso"
        });
        assert!(tool.is_destructive(&params));
    }

    #[test]
    fn test_is_destructive_safe() {
        let tool = make_tool();
        let params = serde_json::json!({
            "command": "python3 holamundo.py",
            "description": "ejecutar script"
        });
        assert!(!tool.is_destructive(&params));
    }
}
