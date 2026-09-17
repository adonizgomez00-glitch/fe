use std::time::Duration;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;
use super::{Tool, ToolResult};
use crate::config::Config;

pub struct BashTool {
    timeout_secs: u64,
    max_output_lines: u32,
    max_output_bytes: u64,
    allow_destructive: bool,
}

impl BashTool {
    pub fn new(config: &Config) -> Self {
        Self {
            timeout_secs: config.tools.bash_timeout_secs,
            max_output_lines: config.tools.max_output_lines,
            max_output_bytes: config.tools.max_output_bytes,
            allow_destructive: config.tools.allow_destructive,
        }
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Ejecuta un comando bash en el sistema. Útil para administración del sistema, desarrollo, instalación de paquetes, etc."
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "name": "bash",
            "description": self.description(),
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Comando bash a ejecutar"
                    },
                    "description": {
                        "type": "string",
                        "description": "Explicación de qué hace el comando"
                    }
                },
                "required": ["command", "description"]
            }
        })
    }

    fn is_destructive(&self, params: &Value) -> bool {
        if self.allow_destructive {
            return false;
        }
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
            .ok_or_else(|| anyhow::anyhow!("Falta el campo 'command'"))?;

        tracing::info!("Ejecutando: {}", command);

        let output = timeout(
            Duration::from_secs(self.timeout_secs),
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Timeout después de {}s ejecutando: {}", self.timeout_secs, command))?
        .map_err(|e| anyhow::anyhow!("Error ejecutando comando: {}", e))?;

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

    fn make_tool() -> BashTool {
        BashTool::new(&Config::default())
    }

    #[tokio::test]
    async fn test_bash_echo() {
        let tool = make_tool();
        let params = serde_json::json!({
            "command": "echo 'hello world'",
            "description": "test"
        });
        let result = tool.execute(params).await.unwrap();
        assert!(result.output.contains("hello world"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_bash_exit_code() {
        let tool = make_tool();
        let params = serde_json::json!({
            "command": "exit 42",
            "description": "test"
        });
        let result = tool.execute(params).await.unwrap();
        assert_eq!(result.exit_code, Some(42));
    }

    #[tokio::test]
    async fn test_bash_missing_command() {
        let tool = make_tool();
        let params = serde_json::json!({});
        assert!(tool.execute(params).await.is_err());
    }

    #[test]
    fn test_is_destructive_rm() {
        let tool = make_tool();
        let params = serde_json::json!({"command": "rm -rf /tmp/test"});
        assert!(tool.is_destructive(&params));
    }

    #[test]
    fn test_is_destructive_safe() {
        let tool = make_tool();
        let params = serde_json::json!({"command": "ls -la"});
        assert!(!tool.is_destructive(&params));
    }

    #[test]
    fn test_is_destructive_ignored_when_allowed() {
        let mut config = Config::default();
        config.tools.allow_destructive = true;
        let tool = BashTool::new(&config);
        let params = serde_json::json!({"command": "rm -rf /"});
        assert!(!tool.is_destructive(&params));
    }
}
