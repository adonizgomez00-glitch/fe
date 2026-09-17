use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub tools: Vec<PluginTool>,
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
    #[serde(default)]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTool {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    pub name: String,
    pub description: String,
}

impl PluginManifest {
    pub fn from_path(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        let contents = std::fs::read_to_string(path)?;
        let manifest: PluginManifest = toml::from_str(&contents)?;
        Ok(manifest)
    }

    pub fn from_str(toml_str: &str) -> Result<Self, anyhow::Error> {
        let manifest: PluginManifest = toml::from_str(toml_str)?;
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
command = "python3"
args = ["bin/plugin.py"]
"#;
        let manifest = PluginManifest::from_str(toml).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "A test plugin");
        assert_eq!(manifest.command, "python3");
        assert_eq!(manifest.args, vec!["bin/plugin.py"]);
        assert!(manifest.tools.is_empty());
        assert!(manifest.commands.is_empty());
        assert!(manifest.skills.is_empty());
        assert!(manifest.env.is_empty());
    }

    #[test]
    fn test_parse_full_manifest() {
        let toml = r#"
name = "mi-plugin"
version = "2.0.0"
description = "Mi plugin personalizado"
author = "usuario"
license = "MIT"
command = "bash"
args = ["bin/mi-plugin.sh"]
env = { DATA_DIR = "/tmp/data" }
skills = ["mi-skill"]

[[tools]]
name = "saludar"
description = "Saluda al usuario"
params = { type = "object", properties = { nombre = { type = "string" } }, required = ["nombre"] }

[[tools]]
name = "backup"
description = "Hace backup"
params = { type = "object", properties = { origen = { type = "string" } }, required = ["origen"] }

[[commands]]
name = "config"
description = "Configura el plugin"

[[commands]]
name = "status"
description = "Estado del plugin"
"#;
        let manifest = PluginManifest::from_str(toml).unwrap();
        assert_eq!(manifest.name, "mi-plugin");
        assert_eq!(manifest.command, "bash");
        assert_eq!(manifest.tools.len(), 2);
        assert_eq!(manifest.tools[0].name, "saludar");
        assert_eq!(manifest.tools[1].name, "backup");
        assert_eq!(manifest.commands.len(), 2);
        assert_eq!(manifest.commands[0].name, "config");
        assert_eq!(manifest.commands[1].name, "status");
        assert_eq!(manifest.skills, vec!["mi-skill"]);
        assert_eq!(manifest.env.get("DATA_DIR").unwrap(), "/tmp/data");
    }

    #[test]
    fn test_parse_invalid_toml() {
        let result = PluginManifest::from_str("not valid toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_required_fields() {
        let toml = r#"
name = "incomplete"
"#;
        let result = PluginManifest::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_params_defaults_to_null() {
        let toml = r#"
name = "test"
version = "1.0.0"
description = "desc"
command = "bash"
args = ["run.sh"]

[[tools]]
name = "simple"
description = "Simple tool"
"#;
        let manifest = PluginManifest::from_str(toml).unwrap();
        assert_eq!(manifest.tools[0].params, serde_json::Value::Null);
    }

    #[test]
    fn test_manifest_from_path_nonexistent() {
        let result = PluginManifest::from_path(std::path::Path::new("/nonexistent/plugin.toml"));
        assert!(result.is_err());
    }
}
