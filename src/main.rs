#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Stdio;
use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod config;
mod agent;
mod daemon;
#[cfg(test)]
mod e2e_tests;
mod mcp;
mod plugins;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = cli::Cli::parse();
    let mut config = config::Config::load()?;

    if cli.flags.destructive {
        config.tools.allow_destructive = true;
    }

    if let Some(socket_path) = &cli.flags.daemon {
        let config = config::Config::load()?;
        let socket_path = std::path::PathBuf::from(socket_path);
        let server = daemon::server::DaemonServer::new(socket_path, config);
        println!("Fe daemon iniciado.");
        server.start().await?;
        return Ok(());
    }

    match &cli.command {
        cli::Command::Prompt(prompt_parts) => {
            let prompt = prompt_parts.join(" ");
            let socket_path = daemon::client::DaemonClient::default_path();
            let client = daemon::client::DaemonClient::new(socket_path);

            if client.is_running().await {
                tracing::info!("Daemon detectado, enrutando prompt via daemon");
                let flags = daemon::protocol::PromptFlags {
                    batch: cli.flags.batch,
                    reasoning: cli.flags.reasoning,
                    fast: cli.flags.fast,
                    dry_run: cli.flags.dry_run,
                    destructive: cli.flags.destructive,
                    checkpoint: cli.flags.checkpoint,
                };
                client.execute_prompt(&prompt, flags).await?;
            } else {
                let mut agent = agent::core::Agent::new(config, cli.flags).await;
                agent.run(&prompt).await?;
            }
        }
        cli::Command::Session(action) => {
            handle_session(action, &config.paths.sessions_dir).await?;
        }
        cli::Command::Config(action) => {
            handle_config(action, &config).await?;
        }
        cli::Command::Completions { shell } => {
            handle_completions(*shell)?;
        }
        cli::Command::Doctor => {
            handle_doctor(&config).await?;
        }
        cli::Command::Daemon(action) => {
            handle_daemon(action, &config).await?;
        }
        cli::Command::Mcp(action) => {
            handle_mcp(action, &config).await?;
        }
        cli::Command::Plugin(action) => {
            handle_plugin(action, &config).await?;
        }
    }

    Ok(())
}

async fn handle_mcp(action: &cli::McpAction, config: &config::Config) -> Result<()> {
    use crate::mcp::registry::McpRegistry;

    let servers_empty = config.mcp.servers.is_empty();

    match action {
        cli::McpAction::Discover => {
            if servers_empty {
                println!("ℹ️  No hay servidores MCP configurados.");
                println!("   Agrega [[mcp.servers]] a tu config.toml");
                return Ok(());
            }

            let mut registry = McpRegistry::load_from_config(&config.mcp.servers).await?;
            let count = registry.client_count();

            let all_tools = registry.discover_all_tools().await.unwrap_or_default();
            let all_resources = registry.discover_all_resources().await.unwrap_or_default();
            let all_prompts = registry.discover_all_prompts().await.unwrap_or_default();

            if all_tools.is_empty() && all_resources.is_empty() && all_prompts.is_empty() {
                println!("ℹ️  No se descubrieron recursos en los servidores MCP.");
            } else {
                if !all_tools.is_empty() {
                    println!("🔧 Herramientas:");
                    for (server, tool) in &all_tools {
                        let full_name = McpRegistry::format_tool_name(server, &tool.name);
                        let desc = tool.description.as_deref().unwrap_or("sin descripción");
                        println!("  {} — {}", full_name, desc);
                    }
                    println!();
                }

                if !all_resources.is_empty() {
                    println!("📄 Recursos:");
                    for (server, resource) in &all_resources {
                        println!("  {}: {} ({})", server, resource.name, resource.uri);
                        if let Some(desc) = &resource.description {
                            println!("    {}", desc);
                        }
                    }
                    println!();
                }

                if !all_prompts.is_empty() {
                    println!("💬 Prompts:");
                    for (server, prompt) in &all_prompts {
                        println!("  {}: {}", server, prompt.name);
                        if let Some(desc) = &prompt.description {
                            println!("    {}", desc);
                        }
                    }
                    println!();
                }

                println!("Total: {} herramientas, {} recursos, {} prompts de {} servidor(es)",
                    all_tools.len(), all_resources.len(), all_prompts.len(), count);
            }

            registry.shutdown_all().await;
        }
        cli::McpAction::Servers => {
            if servers_empty {
                println!("ℹ️  No hay servidores MCP configurados.");
                return Ok(());
            }

            println!("📋 Servidores MCP configurados:");
            for server in &config.mcp.servers {
                let status = if server.enabled { "✅ habilitado" } else { "⏸️  deshabilitado" };
                println!("  {} ({})", server.name, status);
                println!("     Comando: {} {}", server.command, server.args.join(" "));
                if !server.env.is_empty() {
                    println!("     Variables: {} variable(s)", server.env.len());
                }
                println!();
            }
        }
        cli::McpAction::Call(args) => {
            let (server_name, tool_name) = McpRegistry::parse_tool_name(&args.tool)
                .ok_or_else(|| anyhow::anyhow!("Formato inválido. Usa mcp:<server>:<tool>"))?;

            let server_config = config.mcp.servers.iter()
                .find(|s| s.name == server_name && s.enabled)
                .ok_or_else(|| anyhow::anyhow!("Servidor MCP '{}' no encontrado o deshabilitado", server_name))?;

            let params: serde_json::Value = if let Some(args_str) = &args.args {
                serde_json::from_str(args_str)
                    .map_err(|e| anyhow::anyhow!("Error parseando args JSON: {}", e))?
            } else {
                serde_json::json!({})
            };

            match McpRegistry::load_from_config(std::slice::from_ref(server_config)).await {
                Ok(mut registry) => {
                    println!("🔧 Llamando {server_name}:{tool_name}...");
                    match registry.call_tool(&server_name, &tool_name, params).await {
                        Ok(result) => {
                            println!("{}", serde_json::to_string_pretty(&result)?);
                        }
                        Err(e) => {
                            println!("❌ Error: {}", e);
                        }
                    }
                    registry.shutdown_all().await;
                }
                Err(e) => {
                    println!("❌ Error conectando al servidor MCP '{}': {}", server_name, e);
                }
            }
        }
        cli::McpAction::Resources => {
            if servers_empty {
                println!("ℹ️  No hay servidores MCP configurados.");
                return Ok(());
            }

            let mut registry = McpRegistry::load_from_config(&config.mcp.servers).await?;
            let all_resources = registry.discover_all_resources().await.unwrap_or_default();

            if all_resources.is_empty() {
                println!("ℹ️  No se descubrieron recursos MCP.");
            } else {
                println!("📄 Recursos MCP:");
                for (server, resource) in &all_resources {
                    let mime = resource.mime_type.as_deref().unwrap_or("application/octet-stream");
                    println!("  {}: {} ({})", server, resource.name, resource.uri);
                    if let Some(desc) = &resource.description {
                        println!("    {}", desc);
                    }
                    println!("    Tipo: {}", mime);
                }
                println!();
                println!("Total: {} recursos de {} servidor(es)", all_resources.len(), registry.client_count());
            }

            registry.shutdown_all().await;
        }
        cli::McpAction::Prompts => {
            if servers_empty {
                println!("ℹ️  No hay servidores MCP configurados.");
                return Ok(());
            }

            let mut registry = McpRegistry::load_from_config(&config.mcp.servers).await?;
            let all_prompts = registry.discover_all_prompts().await.unwrap_or_default();

            if all_prompts.is_empty() {
                println!("ℹ️  No se descubrieron prompts MCP.");
            } else {
                println!("💬 Prompts MCP:");
                for (server, prompt) in &all_prompts {
                    println!("  {}: {}", server, prompt.name);
                    if let Some(desc) = &prompt.description {
                        println!("    {}", desc);
                    }
                    if let Some(args) = &prompt.arguments {
                        println!("    Argumentos: {}", args.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "));
                    }
                }
                println!();
                println!("Total: {} prompts de {} servidor(es)", all_prompts.len(), registry.client_count());
            }

            registry.shutdown_all().await;
        }
        cli::McpAction::Read(args) => {
            let server_config = config.mcp.servers.iter()
                .find(|s| s.name == args.server && s.enabled)
                .ok_or_else(|| anyhow::anyhow!("Servidor MCP '{}' no encontrado o deshabilitado", args.server))?;

            match McpRegistry::load_from_config(std::slice::from_ref(server_config)).await {
                Ok(mut registry) => {
                    println!("📖 Leyendo recurso '{}' desde '{}'...", args.uri, args.server);
                    match registry.read_resource(&args.server, &args.uri).await {
                        Ok(content) => {
                            println!("{}", content);
                        }
                        Err(e) => {
                            println!("❌ Error: {}", e);
                        }
                    }
                    registry.shutdown_all().await;
                }
                Err(e) => {
                    println!("❌ Error conectando al servidor MCP '{}': {}", args.server, e);
                }
            }
        }
        cli::McpAction::GetPrompt(args) => {
            let server_config = config.mcp.servers.iter()
                .find(|s| s.name == args.server && s.enabled)
                .ok_or_else(|| anyhow::anyhow!("Servidor MCP '{}' no encontrado o deshabilitado", args.server))?;

            let params: Option<serde_json::Value> = if let Some(args_str) = &args.args {
                Some(serde_json::from_str(args_str)
                    .map_err(|e| anyhow::anyhow!("Error parseando args JSON: {}", e))?)
            } else {
                None
            };

            match McpRegistry::load_from_config(std::slice::from_ref(server_config)).await {
                Ok(mut registry) => {
                    println!("💬 Obteniendo prompt '{}' desde '{}'...", args.name, args.server);
                    match registry.get_prompt(&args.server, &args.name, params).await {
                        Ok(messages) => {
                            for msg in &messages {
                                println!("[{}] {}", msg.role, msg.content.text);
                            }
                        }
                        Err(e) => {
                            println!("❌ Error: {}", e);
                        }
                    }
                    registry.shutdown_all().await;
                }
                Err(e) => {
                    println!("❌ Error conectando al servidor MCP '{}': {}", args.server, e);
                }
            }
        }
    }

    Ok(())
}

async fn handle_plugin(action: &cli::PluginAction, config: &config::Config) -> Result<()> {
    use crate::plugins::registry::PluginRegistry;
    use crate::plugins::runtime::PluginRuntime;

    match action {
        cli::PluginAction::List => {
            let mut registry = PluginRegistry::new();
            registry.load_from(&config.paths.plugins_dir);

            if registry.is_empty() {
                println!("📦 No hay plugins instalados.");
                println!("   Crea plugins en: {:?}", config.paths.plugins_dir);
                return Ok(());
            }

            println!("📦 Plugins instalados ({}):", registry.count());
            println!();
            for plugin in registry.list() {
                let m = &plugin.manifest;
                let tool_count = m.tools.len();
                let cmd_count = m.commands.len();
                println!("  {} v{}", m.name, m.version);
                println!("    Descripción: {}", m.description);
                if !m.author.is_empty() {
                    println!("    Autor: {}", m.author);
                }
                if tool_count > 0 {
                    println!("    Tools: {}", tool_count);
                    for tool in &m.tools {
                        println!("      - {}: {}", tool.name, tool.description);
                    }
                }
                if cmd_count > 0 {
                    println!("    Comandos: {}", cmd_count);
                    for cmd in &m.commands {
                        println!("      - {}: {}", cmd.name, cmd.description);
                    }
                }
                println!();
            }
        }
        cli::PluginAction::Reload => {
            let mut registry = PluginRegistry::new();
            registry.reload(&[config.paths.plugins_dir.clone()]);
            let count = registry.count();
            println!("🔄 Plugins recargados: {} plugin(s) encontrado(s)", count);
        }
        cli::PluginAction::Run(args) => {
            let mut registry = PluginRegistry::new();
            registry.load_from(&config.paths.plugins_dir);

            let plugin = registry.get(&args.name)
                .ok_or_else(|| anyhow::anyhow!("Plugin '{}' no encontrado. Usa 'fe plugin list' para ver los disponibles.", args.name))?;

            let has_command = plugin.manifest.commands.iter().any(|c| c.name == args.command);
            if !has_command {
                anyhow::bail!("Plugin '{}' no tiene el comando '{}'. Comandos disponibles: {}",
                    args.name, args.command,
                    plugin.manifest.commands.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
            }

            let mut runtime = PluginRuntime::new(plugin.manifest.clone(), Some(plugin.dir.clone()));
            runtime.spawn().await?;

            let json_args = parse_cli_args_to_json(&args.args);
            match runtime.call_command(&args.command, json_args).await {
                Ok(result) => {
                    if !result.output.is_empty() {
                        println!("{}", result.output);
                    }
                    if result.exit_code != 0 {
                        println!("⚠️  Comando '{}' salió con código {}", args.command, result.exit_code);
                    }
                }
                Err(e) => {
                    println!("❌ Error ejecutando comando '{}': {}", args.command, e);
                }
            }

            runtime.shutdown().await?;
        }
        cli::PluginAction::Install { source } => {
            println!("📥 Instalando plugin desde '{}'...", source);
            println!("   (Función no implementada aún)");
        }
        cli::PluginAction::Uninstall { name } => {
            println!("🗑️  Desinstalando plugin '{}'...", name);
            let plugin_dir = config.paths.plugins_dir.join(name);
            if plugin_dir.exists() {
                std::fs::remove_dir_all(&plugin_dir)?;
                println!("✅ Plugin '{}' desinstalado.", name);
            } else {
                println!("❌ Plugin '{}' no encontrado en {:?}", name, config.paths.plugins_dir);
            }
        }
    }

    Ok(())
}

fn parse_cli_args_to_json(args: &[String]) -> serde_json::Value {
    if args.is_empty() {
        return serde_json::json!({});
    }

    if args.len() == 1 {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&args[0]) {
            return parsed;
        }
    }

    let mut map = serde_json::Map::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            let key = args[i].trim_start_matches("--").to_string();
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                map.insert(key, serde_json::Value::String(args[i + 1].clone()));
                i += 2;
            } else {
                map.insert(key, serde_json::Value::Bool(true));
                i += 1;
            }
        } else if args[i].starts_with('-') && args[i].len() == 2 {
            let key = args[i].trim_start_matches('-').to_string();
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                map.insert(key, serde_json::Value::String(args[i + 1].clone()));
                i += 2;
            } else {
                map.insert(key, serde_json::Value::Bool(true));
                i += 1;
            }
        } else {
            map.insert(format!("arg{}", i), serde_json::Value::String(args[i].clone()));
            i += 1;
        }
    }
    serde_json::Value::Object(map)
}

async fn handle_session(action: &cli::SessionAction, sessions_dir: &PathBuf) -> Result<()> {
    let manager = agent::session::SessionManager::new(sessions_dir);

    match action {
        cli::SessionAction::List => {
            let sessions = manager.list()?;
            if sessions.is_empty() {
                println!("📋 No hay sesiones guardadas.");
                return Ok(());
            }
            println!("📋 Sesiones guardadas ({}):", sessions.len());
            println!();
            for (i, s) in sessions.iter().enumerate() {
                println!("  {}. [{}] {} — {} turnos — \"{}…\"",
                    i + 1, s.id.chars().take(8).collect::<String>(),
                    s.start_time, s.turn_count, s.preview);
                if !s.model.is_empty() {
                    println!("     Modelo: {} | Skills: {}", s.model, s.skills);
                }
                println!();
            }
        }
        cli::SessionAction::Show { id } => {
            match manager.show(id)? {
                Some(content) => {
                    println!("{}", content);
                }
                None => {
                    println!("❌ Sesión '{}' no encontrada.", id);
                }
            }
        }
        cli::SessionAction::Export { id, output } => {
            if manager.export(id, output)? {
                println!("📤 Sesión '{}' exportada a {}", id, output.display());
            } else {
                println!("❌ Sesión '{}' no encontrada.", id);
            }
        }
        cli::SessionAction::Clean => {
            let removed = manager.clean(30)?;
            if removed > 0 {
                println!("🧹 Eliminadas {} sesiones antiguas (>30 días).", removed);
            } else {
                println!("🧹 No hay sesiones antiguas que limpiar.");
            }
        }
    }
    Ok(())
}

async fn handle_config(action: &cli::ConfigAction, config: &config::Config) -> Result<()> {
    match action {
        cli::ConfigAction::Show => {
            println!("{}", toml::to_string_pretty(config).unwrap());
        }
        cli::ConfigAction::Edit => {
            let config_path = config.paths.config_dir.join("config.toml");
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
            let status = std::process::Command::new(&editor)
                .arg(&config_path)
                .status()?;
            if !status.success() {
                anyhow::bail!("Editor exited with status: {}", status);
            }
            tracing::info!("Configuración editada en {:?}", config_path);
        }
    }
    Ok(())
}

fn handle_completions(shell: cli::ShellKind) -> Result<()> {
    use clap::CommandFactory;
    let mut cmd = cli::Cli::command();
    let name = "fe";
    match shell {
        cli::ShellKind::Bash => {
            clap_complete::generate(clap_complete::shells::Bash, &mut cmd, name, &mut std::io::stdout());
        }
        cli::ShellKind::Zsh => {
            clap_complete::generate(clap_complete::shells::Zsh, &mut cmd, name, &mut std::io::stdout());
        }
        cli::ShellKind::Fish => {
            clap_complete::generate(clap_complete::shells::Fish, &mut cmd, name, &mut std::io::stdout());
        }
    }
    Ok(())
}

async fn handle_doctor(config: &config::Config) -> Result<()> {
    println!("🔍 Fe Doctor - Diagnóstico del sistema");
    println!();

    let ollama_host = &config.ollama.host;
    print!("  Ollama ({}) ... ", ollama_host);
    std::io::Write::flush(&mut std::io::stdout())?;
    match reqwest::get(format!("{}/api/tags", ollama_host)).await {
        Ok(resp) if resp.status().is_success() => {
            println!("✅");
        }
        Ok(resp) => {
            println!("⚠️  HTTP {}", resp.status());
        }
        Err(e) => {
            println!("❌ {}", e);
        }
    }

    let config_path = config.paths.config_dir.join("config.toml");
    print!("  Config ... ");
    if config_path.exists() {
        println!("✅ {}", config_path.display());
    } else {
        println!("⚠️  No encontrado ({})", config_path.display());
    }

    let sessions_dir = &config.paths.sessions_dir;
    print!("  Sessions dir ... ");
    if sessions_dir.exists() {
        println!("✅ {}", sessions_dir.display());
    } else {
        println!("⚠️  No existe ({})", sessions_dir.display());
    }

    let skills_dir = &config.paths.skills_dir;
    print!("  Skills dir ... ");
    if skills_dir.exists() {
        println!("✅ {}", skills_dir.display());
    } else {
        println!("⚠️  No encontrado ({})", skills_dir.display());
    }

    Ok(())
}

async fn handle_daemon(action: &cli::DaemonAction, _config: &config::Config) -> Result<()> {
    let socket_path = daemon::client::DaemonClient::default_path();

    match action {
        cli::DaemonAction::Start => {
            if daemon::client::DaemonClient::new(socket_path.clone()).is_running().await {
                println!("⚡ El daemon ya está corriendo.");
                return Ok(());
            }

            let binary = std::env::current_exe()?;

            let child = std::process::Command::new(&binary)
                .args(["--daemon", &socket_path.to_string_lossy().to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("Error iniciando daemon: {}", e))?;

            let pid = child.id();
            daemon::client::DaemonClient::save_pid(pid)?;
            println!("✅ Daemon iniciado (PID: {})", pid);
            println!("   Socket: {:?}", socket_path);
        }
        cli::DaemonAction::Stop => {
            if let Some(pid) = daemon::client::DaemonClient::check_pid() {
                if daemon::client::DaemonClient::is_process_alive(pid) {
                    let kill_result = std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .status()?;
                    if kill_result.success() {
                        println!("✅ Daemon (PID: {}) detenido.", pid);
                    } else {
                        println!("⚠️  No se pudo detener el daemon (PID: {}).", pid);
                    }
                } else {
                    println!("⚠️  El proceso daemon (PID: {}) ya no existe.", pid);
                }
                daemon::client::DaemonClient::remove_pid();
            } else {
                println!("ℹ️  No hay PID file de daemon.");
            }

            if socket_path.exists() {
                tokio::fs::remove_file(&socket_path).await?;
                println!("   Socket eliminado.");
            }
        }
        cli::DaemonAction::Status => {
            let client = daemon::client::DaemonClient::new(socket_path.clone());
            match client.health().await {
                Ok(health) => {
                    println!("✅ Daemon corriendo");
                    println!("   Versión: {}", health.version);
                    println!("   Status: {}", health.status);
                    println!("   Ollama: {}", if health.ollama_connected { "✅ conectado" } else { "❌ desconectado" });
                    if let Some(count) = health.sessions_count {
                        println!("   Sesiones: {}", count);
                    }
                    if let Some(pid) = daemon::client::DaemonClient::check_pid() {
                        println!("   PID: {}", pid);
                    }
                    println!("   Socket: {:?}", socket_path);
                }
                Err(e) => {
                    println!("❌ Daemon no responde: {}", e);
                    if let Some(pid) = daemon::client::DaemonClient::check_pid() {
                        let alive = daemon::client::DaemonClient::is_process_alive(pid);
                        println!("   PID file existe: {} (proceso {})", pid, if alive { "vivo" } else { "muerto" });
                        if !alive {
                            daemon::client::DaemonClient::remove_pid();
                        }
                    }
                    if socket_path.exists() {
                        println!("   Socket existe pero no responde.");
                    } else {
                        println!("   Socket no encontrado.");
                    }
                }
            }
        }
    }

    Ok(())
}
