use clap::{Parser, Subcommand, Args};

#[derive(Parser, Debug)]
#[command(
    name = "fe",
    version = "0.1.0",
    about = "Agente CLI local con Ollama",
    long_about = "Fe conecta tu terminal con modelos LLM vía Ollama para administración de sistemas, desarrollo y arquitectura."
)]
pub struct Cli {
    #[command(flatten)]
    pub flags: GlobalFlags,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone)]
pub struct GlobalFlags {
    #[arg(long, global = true, help = "Modo batch: plan completo, una confirmación")]
    pub batch: bool,

    #[arg(long, global = true, help = "Usar modelo de razonamiento (deepseek-r1:8b)")]
    pub reasoning: bool,

    #[arg(long, global = true, help = "Usar modelo rápido (gemma3:4b)")]
    pub fast: bool,

    #[arg(long, global = true, help = "Mostrar plan sin ejecutar")]
    pub dry_run: bool,

    #[arg(long, global = true, help = "Permitir acciones destructivas sin confirmación extra")]
    pub destructive: bool,

    #[arg(long, global = true, hide = true)]
    pub daemon: Option<String>,

    #[arg(long, global = true, help = "Forzar checkpoint manual (guardar estado actual)")]
    pub checkpoint: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(external_subcommand, hide = true)]
    Prompt(Vec<String>),

    #[command(subcommand)]
    Session(SessionAction),

    #[command(subcommand)]
    Config(ConfigAction),

    #[command()]
    Completions {
        shell: ShellKind,
    },

    #[command()]
    Doctor,

    #[command(subcommand)]
    Daemon(DaemonAction),

    #[command(subcommand)]
    Mcp(McpAction),

    #[command(subcommand)]
    Plugin(PluginAction),
}

#[derive(Subcommand, Debug, Clone)]
pub enum SessionAction {
    #[command()]
    List,
    #[command()]
    Show {
        id: String,
    },
    #[command()]
    Export {
        id: String,
        output: std::path::PathBuf,
    },
    #[command()]
    Clean,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    #[command()]
    Show,
    #[command()]
    Edit,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DaemonAction {
    #[command()]
    Start,
    #[command()]
    Stop,
    #[command()]
    Status,
}

#[derive(Subcommand, Debug, Clone)]
pub enum McpAction {
    #[command()]
    Discover,
    #[command()]
    Servers,
    #[command()]
    Call(McpCallArgs),
    #[command()]
    Resources,
    #[command()]
    Prompts,
    #[command()]
    Read(McpReadArgs),
    #[command(name = "get-prompt")]
    GetPrompt(McpGetPromptArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct McpCallArgs {
    pub tool: String,
    pub args: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct McpReadArgs {
    pub server: String,
    pub uri: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct McpGetPromptArgs {
    pub server: String,
    pub name: String,
    pub args: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PluginAction {
    #[command()]
    List,
    #[command()]
    Reload,
    #[command()]
    Run(PluginExecArgs),
    #[command(name = "install")]
    Install {
        source: String,
    },
    #[command(name = "uninstall")]
    Uninstall {
        name: String,
    },
}

#[derive(clap::Args, Debug, Clone)]
pub struct PluginExecArgs {
    pub name: String,
    pub command: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prompt() {
        let cli = Cli::try_parse_from(["fe", "hola mundo"]).unwrap();
        match &cli.command {
            Command::Prompt(words) => assert_eq!(words.join(" "), "hola mundo"),
            _ => panic!("Expected Prompt command"),
        }
        assert!(!cli.flags.batch);
        assert!(!cli.flags.reasoning);
    }

    #[test]
    fn test_parse_prompt_with_flags() {
        let cli = Cli::try_parse_from(["fe", "--batch", "--reasoning", "apt update"]).unwrap();
        match &cli.command {
            Command::Prompt(words) => assert_eq!(words.join(" "), "apt update"),
            _ => panic!("Expected Prompt command"),
        }
        assert!(cli.flags.batch);
        assert!(cli.flags.reasoning);
    }

    #[test]
    fn test_parse_checkpoint_flag() {
        let cli = Cli::try_parse_from(["fe", "--checkpoint", "continue task"]).unwrap();
        assert!(cli.flags.checkpoint);
        match &cli.command {
            Command::Prompt(words) => assert_eq!(words.join(" "), "continue task"),
            _ => panic!("Expected Prompt command"),
        }
    }

    #[test]
    fn test_parse_doctor() {
        let cli = Cli::try_parse_from(["fe", "doctor"]).unwrap();
        assert!(matches!(cli.command, Command::Doctor));
    }

    #[test]
    fn test_parse_session_list() {
        let cli = Cli::try_parse_from(["fe", "session", "list"]).unwrap();
        match &cli.command {
            Command::Session(action) => assert!(matches!(action, SessionAction::List)),
            _ => panic!("Expected Session command"),
        }
    }

    #[test]
    fn test_parse_session_show() {
        let cli = Cli::try_parse_from(["fe", "session", "show", "abc123"]).unwrap();
        match &cli.command {
            Command::Session(SessionAction::Show { id }) => assert_eq!(id, "abc123"),
            _ => panic!("Expected Session::Show command"),
        }
    }

    #[test]
    fn test_parse_config_show() {
        let cli = Cli::try_parse_from(["fe", "config", "show"]).unwrap();
        match &cli.command {
            Command::Config(action) => assert!(matches!(action, ConfigAction::Show)),
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_parse_completions_bash() {
        let cli = Cli::try_parse_from(["fe", "completions", "bash"]).unwrap();
        match &cli.command {
            Command::Completions { shell } => assert!(matches!(shell, ShellKind::Bash)),
            _ => panic!("Expected Completions command"),
        }
    }

    #[test]
    fn test_parse_dry_run_flag() {
        let cli = Cli::try_parse_from(["fe", "--dry-run", "rm -rf /tmp/test"]).unwrap();
        assert!(cli.flags.dry_run);
        match &cli.command {
            Command::Prompt(words) => assert_eq!(words.join(" "), "rm -rf /tmp/test"),
            _ => panic!("Expected Prompt command"),
        }
    }

    #[test]
    fn test_parse_daemon_start() {
        let cli = Cli::try_parse_from(["fe", "daemon", "start"]).unwrap();
        match &cli.command {
            Command::Daemon(action) => assert!(matches!(action, DaemonAction::Start)),
            _ => panic!("Expected Daemon::Start command"),
        }
    }

    #[test]
    fn test_parse_daemon_stop() {
        let cli = Cli::try_parse_from(["fe", "daemon", "stop"]).unwrap();
        match &cli.command {
            Command::Daemon(action) => assert!(matches!(action, DaemonAction::Stop)),
            _ => panic!("Expected Daemon::Stop command"),
        }
    }

    #[test]
    fn test_parse_daemon_status() {
        let cli = Cli::try_parse_from(["fe", "daemon", "status"]).unwrap();
        match &cli.command {
            Command::Daemon(action) => assert!(matches!(action, DaemonAction::Status)),
            _ => panic!("Expected Daemon::Status command"),
        }
    }

    #[test]
    fn test_parse_mcp_discover() {
        let cli = Cli::try_parse_from(["fe", "mcp", "discover"]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::Discover) => {}
            _ => panic!("Expected Mcp::Discover command"),
        }
    }

    #[test]
    fn test_parse_mcp_servers() {
        let cli = Cli::try_parse_from(["fe", "mcp", "servers"]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::Servers) => {}
            _ => panic!("Expected Mcp::Servers command"),
        }
    }

    #[test]
    fn test_parse_mcp_call() {
        let cli = Cli::try_parse_from(["fe", "mcp", "call", "mcp:test:echo", r#"{"msg":"hi"}"#]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::Call(args)) => {
                assert_eq!(args.tool, "mcp:test:echo");
                assert_eq!(args.args.as_deref(), Some(r#"{"msg":"hi"}"#));
            }
            _ => panic!("Expected Mcp::Call command"),
        }
    }

    #[test]
    fn test_parse_mcp_resources() {
        let cli = Cli::try_parse_from(["fe", "mcp", "resources"]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::Resources) => {}
            _ => panic!("Expected Mcp::Resources command"),
        }
    }

    #[test]
    fn test_parse_mcp_prompts() {
        let cli = Cli::try_parse_from(["fe", "mcp", "prompts"]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::Prompts) => {}
            _ => panic!("Expected Mcp::Prompts command"),
        }
    }

    #[test]
    fn test_parse_mcp_read() {
        let cli = Cli::try_parse_from(["fe", "mcp", "read", "test-server", "file:///test.txt"]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::Read(args)) => {
                assert_eq!(args.server, "test-server");
                assert_eq!(args.uri, "file:///test.txt");
            }
            _ => panic!("Expected Mcp::Read command"),
        }
    }

    #[test]
    fn test_parse_mcp_get_prompt() {
        let cli = Cli::try_parse_from(["fe", "mcp", "get-prompt", "test-server", "greet", r#"{"name":"Mundo"}"#]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::GetPrompt(args)) => {
                assert_eq!(args.server, "test-server");
                assert_eq!(args.name, "greet");
                assert_eq!(args.args.as_deref(), Some(r#"{"name":"Mundo"}"#));
            }
            _ => panic!("Expected Mcp::GetPrompt command"),
        }
    }

    #[test]
    fn test_parse_mcp_call_no_args() {
        let cli = Cli::try_parse_from(["fe", "mcp", "call", "mcp:test:ping"]).unwrap();
        match &cli.command {
            Command::Mcp(McpAction::Call(args)) => {
                assert_eq!(args.tool, "mcp:test:ping");
                assert!(args.args.is_none());
            }
            _ => panic!("Expected Mcp::Call command"),
        }
    }
}
