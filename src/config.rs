use std::path::PathBuf;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ollama: OllamaConfig,
    pub models: ModelsConfig,
    pub agent: AgentConfig,
    pub tools: ToolsConfig,
    pub paths: PathsConfig,
    pub shell: ShellConfig,
    pub mcp: McpConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub host: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub keep_alive_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    pub complex: String,
    pub simple: String,
    pub reasoning: String,
    pub summarizer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub name: String,
    pub confirm_tools: bool,
    pub batch_mode: bool,
    pub auto_save: bool,
    pub summarize_every_n_turns: u32,
    pub max_history_turns: u32,
    pub max_context_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    pub bash_timeout_secs: u64,
    pub max_output_lines: u32,
    pub max_output_bytes: u64,
    pub allow_destructive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub sessions_dir: PathBuf,
    pub skills_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub config_dir: PathBuf,
    pub checkpoints_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub generate_completions: bool,
    pub shells: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub enabled: bool,
    pub servers: Vec<crate::mcp::types::McpServerConfig>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: env_or_default("FE_OLLAMA_HOST", "http://localhost:11434"),
            timeout_secs: env_or_default_parse("FE_OLLAMA_TIMEOUT_SECS", 300),
            max_retries: env_or_default_parse("FE_OLLAMA_MAX_RETRIES", 3),
            keep_alive_secs: env_or_default_parse("FE_OLLAMA_KEEP_ALIVE_SECS", 300),
        }
    }
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            complex: env_or_default("FE_MODELS_COMPLEX", "qwen2.5:3b"),
            simple: env_or_default("FE_MODELS_SIMPLE", "gemma3:4b"),
            reasoning: env_or_default("FE_MODELS_REASONING", "deepseek-r1:8b"),
            summarizer: env_or_default("FE_MODELS_SUMMARIZER", "gemma3:4b"),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: env_or_default("FE_AGENT_NAME", "fe"),
            confirm_tools: env_or_default_parse("FE_AGENT_CONFIRM_TOOLS", true),
            batch_mode: env_or_default_parse("FE_AGENT_BATCH_MODE", false),
            auto_save: env_or_default_parse("FE_AGENT_AUTO_SAVE", true),
            summarize_every_n_turns: env_or_default_parse("FE_AGENT_SUMMARIZE_EVERY_N_TURNS", 4),
            max_history_turns: env_or_default_parse("FE_AGENT_MAX_HISTORY_TURNS", 20),
            max_context_tokens: env_or_default_parse("FE_AGENT_MAX_CONTEXT_TOKENS", 8192),
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            bash_timeout_secs: env_or_default_parse("FE_TOOLS_BASH_TIMEOUT_SECS", 120),
            max_output_lines: env_or_default_parse("FE_TOOLS_MAX_OUTPUT_LINES", 5000),
            max_output_bytes: env_or_default_parse("FE_TOOLS_MAX_OUTPUT_BYTES", 1048576),
            allow_destructive: env_or_default_parse("FE_TOOLS_ALLOW_DESTRUCTIVE", false),
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("fe");
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("fe");
        let skills_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("opencode")
            .join("skills");

        Self {
            sessions_dir: data_dir.join("sessions"),
            skills_dir,
            plugins_dir: config_dir.join("plugins"),
            config_dir,
            checkpoints_dir: data_dir.join("checkpoints"),
        }
    }
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            generate_completions: env_or_default_parse("FE_SHELL_GENERATE_COMPLETIONS", true),
            shells: vec!["bash".into(), "zsh".into(), "fish".into()],
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: env_or_default_parse("FE_MCP_ENABLED", true),
            servers: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ollama: OllamaConfig::default(),
            models: ModelsConfig::default(),
            agent: AgentConfig::default(),
            tools: ToolsConfig::default(),
            paths: PathsConfig::default(),
            shell: ShellConfig::default(),
            mcp: McpConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let defaults = Config::default();
        let config_path = defaults.paths.config_dir.join("config.toml");
        Self::load_from(&config_path)
    }

    pub fn load_from(config_path: &std::path::Path) -> Result<Self> {
        let mut config = Config::default();

        if config_path.exists() {
            let contents = std::fs::read_to_string(config_path)?;
            let file_config: Config = toml::from_str(&contents)?;
            config.merge(file_config);
        }

        Ok(config)
    }

    fn merge(&mut self, other: Config) {
        if other.ollama.host != String::default() {
            self.ollama.host = other.ollama.host;
        }
        self.ollama.timeout_secs = other.ollama.timeout_secs;
        self.ollama.max_retries = other.ollama.max_retries;
        self.models = other.models;
        self.agent = other.agent;
        self.tools = other.tools;
        if other.paths.config_dir != PathsConfig::default().config_dir {
            self.paths = other.paths;
        }
        self.shell = other.shell;
        self.mcp = other.mcp;
    }
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_or_default_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ollama_config() {
        let cfg = OllamaConfig::default();
        assert_eq!(cfg.host, "http://localhost:11434");
        assert_eq!(cfg.timeout_secs, 300);
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.keep_alive_secs, 300);
    }

    #[test]
    fn test_default_models_config() {
        let cfg = ModelsConfig::default();
        assert_eq!(cfg.complex, "qwen2.5:3b");
        assert_eq!(cfg.simple, "gemma3:4b");
        assert_eq!(cfg.reasoning, "deepseek-r1:8b");
        assert_eq!(cfg.summarizer, "gemma3:4b");
    }

    #[test]
    fn test_default_agent_config() {
        let cfg = AgentConfig::default();
        assert_eq!(cfg.name, "fe");
        assert!(cfg.confirm_tools);
        assert!(!cfg.batch_mode);
        assert!(cfg.auto_save);
        assert_eq!(cfg.summarize_every_n_turns, 4);
        assert_eq!(cfg.max_history_turns, 20);
        assert_eq!(cfg.max_context_tokens, 8192);
    }

    #[test]
    fn test_default_tools_config() {
        let cfg = ToolsConfig::default();
        assert_eq!(cfg.bash_timeout_secs, 120);
        assert_eq!(cfg.max_output_lines, 5000);
        assert_eq!(cfg.max_output_bytes, 1048576);
        assert!(!cfg.allow_destructive);
    }

    #[test]
    fn test_default_shell_config() {
        let cfg = ShellConfig::default();
        assert!(cfg.generate_completions);
        assert_eq!(cfg.shells, vec!["bash", "zsh", "fish"]);
    }

    #[test]
    fn test_env_var_overrides() {
        unsafe {
            std::env::set_var("FE_OLLAMA_HOST", "http://custom:11434");
            std::env::set_var("FE_OLLAMA_TIMEOUT_SECS", "600");
            std::env::set_var("FE_AGENT_CONFIRM_TOOLS", "false");
            std::env::set_var("FE_TOOLS_ALLOW_DESTRUCTIVE", "true");
        }

        let cfg = Config::default();
        assert_eq!(cfg.ollama.host, "http://custom:11434");
        assert_eq!(cfg.ollama.timeout_secs, 600);
        assert!(!cfg.agent.confirm_tools);
        assert!(cfg.tools.allow_destructive);

        unsafe {
            std::env::remove_var("FE_OLLAMA_HOST");
            std::env::remove_var("FE_OLLAMA_TIMEOUT_SECS");
            std::env::remove_var("FE_AGENT_CONFIRM_TOOLS");
            std::env::remove_var("FE_TOOLS_ALLOW_DESTRUCTIVE");
        }
    }

    #[test]
    fn test_load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, r#"
            [ollama]
            host = "http://custom:11434"
            timeout_secs = 600
            [models]
            complex = "llama3:70b"
        "#).unwrap();

        let loaded = Config::load_from(&config_path).unwrap();
        assert_eq!(loaded.ollama.host, "http://custom:11434");
        assert_eq!(loaded.ollama.timeout_secs, 600);
        assert_eq!(loaded.models.complex, "llama3:70b");
        assert_eq!(loaded.models.simple, "gemma3:4b");
    }

    #[test]
    fn test_load_from_nonexistent_file() {
        let path = std::path::Path::new("/tmp/nonexistent_config_fe_test_12345.toml");
        let loaded = Config::load_from(path).unwrap();
        assert_eq!(loaded.ollama.host, "http://localhost:11434");
    }

    #[test]
    fn test_merge_replaces_host() {
        let mut base = Config::default();
        let mut other = Config::default();
        other.ollama.host = "http://other:11434".into();
        base.merge(other);
        assert_eq!(base.ollama.host, "http://other:11434");
    }

    #[test]
    fn test_merge_skips_empty_host() {
        let mut base = Config::default();
        base.ollama.host = "http://custom:11434".into();
        let mut other = Config::default();
        other.ollama.host = String::default();
        base.merge(other);
        assert_eq!(base.ollama.host, "http://custom:11434");
    }

    #[test]
    fn test_env_or_default_works() {
        assert_eq!(env_or_default("NONEXISTENT_VAR_12345", "default"), "default");
        unsafe { std::env::set_var("NONEXISTENT_VAR_12345", "custom"); }
        assert_eq!(env_or_default("NONEXISTENT_VAR_12345", "default"), "custom");
        unsafe { std::env::remove_var("NONEXISTENT_VAR_12345"); }
    }

    #[test]
    fn test_env_or_default_parse_works() {
        assert_eq!(env_or_default_parse("NONEXISTENT_PARSE_12345", 42u32), 42);
        unsafe { std::env::set_var("NONEXISTENT_PARSE_12345", "100"); }
        assert_eq!(env_or_default_parse("NONEXISTENT_PARSE_12345", 42u32), 100);
        unsafe { std::env::remove_var("NONEXISTENT_PARSE_12345"); }
    }
}
