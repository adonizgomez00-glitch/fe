use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use tokio::sync::Mutex;
use crate::cli::GlobalFlags;
use crate::config::Config;
use crate::agent::llm::{OllamaClient, Message, Provider};
use crate::agent::router::ModelRouter;
use crate::agent::session::SessionManager;
use crate::agent::skills::SkillsLoader;
use crate::agent::summarizer::Summarizer;
use crate::agent::tools::ToolRegistry;
use crate::mcp::registry::McpRegistry;
use crate::mcp::types::McpTool;
use crate::plugins::registry::PluginRegistry;
use crate::plugins::runtime::PluginRuntime;
use crate::utils::confirm::{self, ConfirmChoice};
use crate::agent::checkpoint::{Checkpoint, CheckpointManager};
use crate::agent::services::{CoreServices, default_impl::{DefaultToolsService, DefaultProvidersService, DefaultCacheService, DefaultSecurityService}};

pub struct Agent {
    flags: GlobalFlags,
    client: OllamaClient,
    router: ModelRouter,
    registry: ToolRegistry,
    core_services: CoreServices,
    skills: SkillsLoader,
    session: SessionManager,
    summarizer: Summarizer,
    config: Config,
    checkpoint_manager: CheckpointManager,
    mcp_registry: Option<Arc<Mutex<McpRegistry>>>,
    plugin_runtimes: HashMap<String, Arc<Mutex<PluginRuntime>>>,
}

impl Agent {
    pub async fn new(config: Config, flags: GlobalFlags) -> Self {
        let mut registry = ToolRegistry::new();
        registry.register_defaults(&config);
        
        let core_services = CoreServices::new(
            Arc::new(DefaultToolsService::new(registry)),
            Arc::new(DefaultProvidersService::new(&config)),
            Arc::new(DefaultCacheService::new()),
            Arc::new(DefaultSecurityService::new()),
        );

        let provider_arc = core_services.providers.default_provider().await;
        let provider: Arc<dyn Provider> = provider_arc.into();
        let client = OllamaClient::new(provider);
        let router = ModelRouter::new(&config);
        let skills = SkillsLoader::load_from(&config.paths.skills_dir).unwrap_or_default();
        let session = SessionManager::new(&config.paths.sessions_dir);
        let summarizer = Summarizer::new(&config);

        std::fs::create_dir_all(&config.paths.checkpoints_dir).ok();
        let checkpoint_manager = CheckpointManager::new(&config.paths.checkpoints_dir);

        tracing::info!("Skills cargadas: {}", skills.len());
        
        Self {
            flags,
            client,
            router,
            registry: ToolRegistry::new(),
            core_services,
            skills,
            session,
            summarizer,
            config,
            checkpoint_manager,
            mcp_registry: None,
            plugin_runtimes: HashMap::new(),
        }
    }

    async fn init_mcp(&mut self) {
        if self.mcp_registry.is_some() {
            return;
        }
        if !self.config.mcp.enabled || self.config.mcp.servers.is_empty() {
            return;
        }

        let enabled_servers: Vec<_> = self.config.mcp.servers.iter()
            .filter(|s| s.enabled)
            .cloned()
            .collect();

        if enabled_servers.is_empty() {
            return;
        }

        match McpRegistry::load_from_config(&enabled_servers).await {
            Ok(mut reg) => {
                let tools = reg.discover_all_tools().await.unwrap_or_default();
                let mut tools_by_server: HashMap<String, Vec<McpTool>> = HashMap::new();
                for (server_name, tool) in tools {
                    tools_by_server.entry(server_name).or_default().push(tool);
                }
                let reg_arc = Arc::new(Mutex::new(reg));
                for (server_name, server_tools) in &tools_by_server {
                    self.registry.register_mcp_tools(&reg_arc, server_name, server_tools);
                }
                self.mcp_registry = Some(reg_arc);
                tracing::info!("MCP initialized with {} server(s)", enabled_servers.len());
            }
            Err(e) => {
                tracing::warn!("MCP initialization failed: {}", e);
            }
        }
    }

    async fn init_plugins(&mut self) {
        if !self.plugin_runtimes.is_empty() {
            return;
        }

        let mut reg = PluginRegistry::new();
        reg.load_from(&self.config.paths.plugins_dir);

        if reg.is_empty() {
            return;
        }

        let plugins = reg.list();
        for plugin in plugins {
            let name = plugin.manifest.name.clone();
            let mut runtime = PluginRuntime::new(plugin.manifest.clone(), Some(plugin.dir.clone()));

            match runtime.spawn().await {
                Ok(()) => {
                    let tools = plugin.manifest.tools.clone();
                    let runtime_arc = Arc::new(Mutex::new(runtime));
                    if !tools.is_empty() {
                        self.registry.register_plugin_tools(&runtime_arc, &name, &tools);
                    }
                    self.plugin_runtimes.insert(name.clone(), runtime_arc);
                    tracing::info!("Plugin '{}' initialized with {} tool(s)", name, tools.len());
                }
                Err(e) => {
                    tracing::warn!("Failed to spawn plugin '{}': {}", name, e);
                }
            }
        }
    }

    pub async fn run(&mut self, prompt: &str) -> Result<()> {
        self.init_mcp().await;
        self.init_plugins().await;

        if let Some(result) = self.try_direct_execution(prompt).await {
            return result;
        }

        let (model, _skill_kind) = self.router.select(prompt, &self.flags);
        tracing::info!("Modelo seleccionado: {}", model);
        tracing::info!("Prompt: {}", prompt);

        let supports_tools = self.router.supports_tools(&model);

        let matched_skills: Vec<&crate::agent::skills::Skill> = self.skills.match_prompt(prompt).into_iter().map(|(s, _)| s).collect();
        if !matched_skills.is_empty() {
            tracing::info!("Skills detectadas: {:?}", matched_skills.iter().map(|s| &s.name).collect::<Vec<_>>());
            let skill_names: Vec<String> = matched_skills.iter().map(|s| s.name.clone()).collect();
            self.session.create(&model, &skill_names)?;
        } else {
            self.session.create(&model, &[])?;
        }

        let result = if !supports_tools {
            self.run_chat(&model, prompt, &matched_skills).await
        } else {
            self.run_agent_loop(&model, prompt, &matched_skills).await
        };

        if let Ok(()) = &result {
            let response_summary = "...";
            self.session.add_turn(prompt, response_summary)?;
            self.try_summarize().await;
        }

        result
    }

    pub fn get_session_id(&self) -> String {
        self.session.current().map(|s| s.id.clone()).unwrap_or_default()
    }

    pub fn get_session_manager(&self) -> &SessionManager {
        &self.session
    }

    async fn try_direct_execution(&self, prompt: &str) -> Option<Result<()>> {
        use crate::agent::direct_executor::{match_prompt, execute_direct, format_output};

        let dm = match_prompt(prompt)?;

        if self.flags.dry_run {
            let icon = if dm.is_destructive { "🔴" } else { "  " };
            println!("\n📋 DRY RUN - Plan de ejecución:");
            println!("  {}. {} {} — {}", 1, icon, "bash", dm.description);
            println!("     `{}`", dm.command);
            println!("  (modo dry-run: no se ejecutó nada)\n");
            return Some(Ok(()));
        }

        if dm.is_destructive && !self.flags.destructive && !self.config.tools.allow_destructive {
            println!("\n⚠️  ACCIÓN DESTRUCTIVA: {}", dm.description);
            print!("¿Estás seguro? [y/N]: ");
            use std::io::Write;
            std::io::stdout().flush().ok();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes" | "s" | "si") {
                println!("  Cancelado.");
                return Some(Ok(()));
            }
        }

        println!("\n⚡ {}", dm.description);
        match execute_direct(&dm).await {
            Ok(output) => {
                println!("{}", format_output(&dm, &output));
                Some(Ok(()))
            }
            Err(e) => Some(Err(e)),
        }
    }

    async fn run_chat(&self, model: &str, prompt: &str, matched_skills: &[&crate::agent::skills::Skill]) -> Result<()> {
        let skills_prompt = self.skills.format_skills_for_prompt(matched_skills);
        let mut system = format!("Eres Fe, un agente CLI experto en administración de sistemas, desarrollo de software y arquitectura. Responde de forma concisa y precisa en español.\n\nSistema operativo: {}\n", detect_os());
        if !skills_prompt.is_empty() {
            system.push_str(&skills_prompt);
        }

        let messages = vec![
            Message {
                role: "system".into(),
                content: system,
                tool_calls: None,
            },
            Message {
                role: "user".into(),
                content: prompt.to_string(),
                tool_calls: None,
            },
        ];

        let spinner = self.start_spinner("Pensando...");
        let result = self.client.chat_with_tools(model, &messages, &[], Some(&spinner)).await?;
        if !result.content.is_empty() {
            println!("{}", result.content);
        }

        Ok(())
    }

    async fn run_agent_loop(&self, model: &str, prompt: &str, matched_skills: &[&crate::agent::skills::Skill]) -> Result<()> {
        let skills_prompt = self.skills.format_skills_for_prompt(matched_skills);
        let system_prompt = self.build_system_prompt(&skills_prompt);
        let mut messages = vec![
            Message {
                role: "system".into(),
                content: system_prompt,
                tool_calls: None,
            },
            Message {
                role: "user".into(),
                content: prompt.to_string(),
                tool_calls: None,
            },
        ];

        let tool_schemas = self.registry.all_schemas();
        let mut batch_confirmed = false;
        let mut last_responses: Vec<(String, String)> = Vec::new();
        let mut all_tool_calls: Vec<crate::agent::llm::ToolCall> = Vec::new();
        let mut loop_warnings = 0u32;

        for _round in 0..8 {
            self.check_context_before_call(model, prompt, &mut messages, &tool_schemas).await;

            let spinner = self.start_spinner("Pensando...");
            let result = self.client.chat_with_tools(model, &messages, &tool_schemas, Some(&spinner)).await?;

            messages.push(Message {
                role: "assistant".into(),
                content: result.content.clone(),
                tool_calls: if result.tool_calls.is_empty() { None } else { Some(result.tool_calls.clone()) },
            });

            let tc_sig: String = result.tool_calls.iter()
                .map(|tc| tc.function.name.clone())
                .collect::<Vec<_>>()
                .join(",");
            last_responses.push((result.content.clone(), tc_sig));
            if last_responses.len() > 5 {
                last_responses.remove(0);
            }
            if last_responses.len() >= 3 && self.is_response_looping(&last_responses) {
                loop_warnings += 1;
                if loop_warnings >= 3 {
                    println!("\n⚠️  Bucle persistente detectado. Intentando recuperacion...");
                    self.try_recover_from_loop(prompt, &mut messages).await;
                    break;
                }
                println!("\n⚠️  Detectado bucle de repetición. El modelo está repitiendo la misma respuesta sin ejecutar herramientas.");
                println!("🔧 ÚLTIMA advertencia: ejecuta la herramienta AHORA o se forzará la salida.\n");
                messages.push(Message {
                    role: "user".into(),
                    content: "⚠️ Estás en un bucle. Ejecuta el tool_call AHORA. No respondas con texto.".into(),
                    tool_calls: None,
                });
                continue;
            }

            if result.tool_calls.is_empty() {
                if !result.content.is_empty() {
                    println!("{}", result.content);
                }
                if self.prompt_requires_actions(prompt) {
                    let noop_count = last_responses.iter().filter(|(c, tc)| tc.is_empty() && (c.contains("lamento") || c.contains("lamentable") || c.contains("confusión") || c.contains("intent") || c.contains("voya") || c.len() < 80)).count();
                    if noop_count >= 4 {
                        println!("\n⚠️  Múltiples intentos sin progreso. Forzando ejecución directa.");
                        let emergency_cmd = self.try_emergency_execution(prompt).await;
                        if let Some(result) = emergency_cmd {
                            println!("{}", result.output);
                            messages.push(Message {
                                role: "tool".into(),
                                content: format!("Resultado: {}", result.output.trim()),
                                tool_calls: None,
                            });
                            continue;
                        }
                    }
                    messages.push(Message {
                        role: "user".into(),
                        content: "No ejecutaste ninguna herramienta. Debes usar tool_calls para realizar acciones. Ejecuta UN solo comando ahora, sin describirlo.".into(),
                        tool_calls: None,
                    });
                    continue;
                }
                return Ok(());
            }

            all_tool_calls.extend(result.tool_calls.clone());

            let should_execute = if self.flags.dry_run {
                self.dry_run_plan(&result.tool_calls);
                false
            } else if self.flags.batch {
                if !batch_confirmed {
                    let plan = self.format_plan(&all_tool_calls);
                    if !plan.is_empty() {
                        println!("\n📋 Plan completo ({} operaciones):", all_tool_calls.len());
                        for line in &plan {
                            println!("  {}", line);
                        }
                        print!("¿Ejecutar? [Y/n]: ");
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).ok();
                        batch_confirmed = matches!(input.trim().to_lowercase().as_str(), "" | "y" | "yes" | "s" | "si");
                        batch_confirmed
                    } else {
                        true
                    }
                } else {
                    true
                }
            } else if !self.config.agent.confirm_tools {
                true
            } else {
                self.confirm_tool_calls(&result.tool_calls)
            };

            if !should_execute {
                messages.push(Message {
                    role: "tool".into(),
                    content: "El usuario canceló la ejecución de herramientas.".into(),
                    tool_calls: None,
                });
                continue;
            }

            for tool_call in &result.tool_calls {
                let name = &tool_call.function.name;
                let params = &tool_call.function.arguments;
                let description = params
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("sin descripción");

                if !self.flags.batch || !batch_confirmed {
                    println!("\n🔧 {} — {}", name, description);
                }

                if self.flags.dry_run {
                    continue;
                }

                match self.registry.get(name) {
                    Some(tool) => {
                        let output_preview = match tool.execute(params.clone()).await {
                            Ok(tool_result) => {
                                let total_len = tool_result.output.len();
                                let preview = if total_len > 4000 {
                                    let truncated: String = tool_result.output.chars().take(4000).collect();
                                    let lines = tool_result.output.lines().count();
                                    format!(
                                        "[Output: {} bytes, {} líneas - mostrando primeras 4000]\n{}\n... (truncado, {} bytes restantes)",
                                        total_len, lines, truncated, total_len - truncated.len()
                                    )
                                } else {
                                    tool_result.output.clone()
                                };

                                if !tool_result.output.trim().is_empty() {
                                    println!("{}", tool_result.output);
                                }

                                tracing::info!("Tool '{}' exit_code: {:?}", name, tool_result.exit_code);
                                preview
                            }
                            Err(e) => {
                                let error_msg = format!("Error ejecutando '{}': {}", name, e);
                                println!("{}", error_msg);
                                tracing::error!("{}", error_msg);
                                format!("⚠️ Error: {}. El usuario puede intentar de nuevo o probar un enfoque diferente.", e)
                            }
                        };

                        messages.push(Message {
                            role: "tool".into(),
                            content: output_preview,
                            tool_calls: None,
                        });
                    }
                    None => {
                        messages.push(Message {
                            role: "tool".into(),
                            content: format!("Error: herramienta '{}' no encontrada", name),
                            tool_calls: None,
                        });
                    }
                }
            }
        }

        println!("\n⚠️  Límite de iteraciones alcanzado. La tarea podría estar incompleta.");
        Ok(())
    }

    fn build_system_prompt(&self, skills_section: &str) -> String {
        let mut prompt = String::from(
            "Eres Fe, un agente CLI experto en administración de sistemas, desarrollo de software y arquitectura.\n\n"
        );

        prompt.push_str(&format!("Sistema operativo: {}\n\n", detect_os()));

        if !skills_section.is_empty() {
            prompt.push_str("Aplica los siguientes skills según corresponda:\n");
            prompt.push_str(skills_section);
            prompt.push('\n');
        }

        if self.flags.dry_run {
            prompt.push_str("Modo DRY RUN: Muestra el plan de ejecución pero NO ejecutes ninguna herramienta.\n\n");
        }

        if self.flags.batch {
            prompt.push_str(
                "Modo BATCH: Ejecutarás herramientas automáticamente.\n\n\
                INSTRUCCIÓN CRÍTICA: Debes incluir TODOS los tool_calls necesarios en UNA SOLA respuesta. \
                No dividas las operaciones. Por ejemplo, si el usuario pide listar, crear, listar, borrar y listar,\n\
                debes emitir los 5 tool_calls en una sola respuesta.\n\n"
            );
        }

        prompt.push_str(
            "REGLAS ESTRICTAS:\n\
             1. ✅ Usa SIEMPRE tool_calls para CUALQUIER acción (crear, eliminar, listar, instalar, modificar, etc.).\n\
             2. ❌ NUNCA digas que vas a hacer algo sin ejecutar el tool_call. \"Voy a eliminar X\" NO elimina nada.\n\
             3. ❌ NUNCA describas comandos en texto o bloques de código markdown.\n\
             4. ✅ Si la tarea tiene UN SOLO paso, ejecuta UNA herramienta a la vez.\n\
             5. ✅ Si la tarea tiene MÚLTIPLES pasos (ej: listar, crear, listar, borrar, listar),\n\
                ejecuta los pasos secuencialmente uno por uno hasta completar todos.\n\
             6. ✅ Proporciona una descripción clara en cada tool_call.\n\
             7. ✅ Cuando hayas completado TODOS los pasos, responde en texto con un resumen.\n\
             8. ❌ NO generes tool_calls adicionales después de completar la tarea.\n\
             9. ❌ NO repitas operaciones ya realizadas.\n\
             10. ✅ Sé conciso y preciso en español.\n\
              11. ❌ NUNCA digas frases como \"Voy a...\", \"Vamos a...\", \"Lamento la confusión\",\n\
                  \"Haz una pausa\", \"Espera\", \"Detente\", \"Espera un momento\".\n\
                  SIMPLEMENTE ejecuta el tool_call sin preámbulos.\n\
              12. ✅ RECUPERACIÓN ANTE ERRORES: Si un comando falla, prueba alternativas antes de rendirte.\n\
                  Por ejemplo: si `python` no existe, prueba `python3`; si `apt` falla, prueba `apt-get`;\n\
                  si `pip` no funciona, prueba `pip3`. No sugieras lo mismo que ya falló.\n\
              13. ✅ PRE-CHECK: Antes de ejecutar un script con un intérprete (python, node, bash, etc.),\n\
                  verifica que el intérprete existe con `which <intérprete>` primero.\n\
              14. ✅ GESTIÓN DE PAQUETES: En sistemas Debian/Ubuntu/Mint usa SIEMPRE `apt` para\n\
                  instalar, actualizar o eliminar paquetes del sistema. NO uses npm, pip ni otros\n\
                  gestores para paquetes del sistema a menos que el usuario lo pida explícitamente.\n\
              15. ✅ CREACIÓN DE DIRECTORIOS: Usa `bash mkdir -p <ruta>` para crear directorios.\n\
                  NO uses la herramienta `write` para crear directorios.\n\
              16. ✅ LISTAR DIRECTORIOS: Usa `bash ls -la <ruta>` para listar contenidos.\n\
                  NO uses la herramienta `read` para leer directorios.\n\
              17. ✅ EJEMPLO CORRECTO: Usuario: \"crea una app python\" → Respuesta: tool_call bash\n\
                  \"mkdir -p app_python && cat > ...\". Sin preámbulos, sin pausas.\n\
              18. ✅ EJEMPLO CORRECTO: Usuario: \"lista archivos\" → tool_call bash \"ls -la\".\n\
              19. ✅ SIEMPRE ejecuta al menos un tool_call por turno si la tarea lo requiere. No\n\
                  respondas solo con texto cuando la tarea pide una acción."
        );

        prompt
    }

    fn confirm_tool_calls(&self, tool_calls: &[crate::agent::llm::ToolCall]) -> bool {
        let mut all_mode = false;
        for tool_call in tool_calls {
            let name = &tool_call.function.name;
            let params = &tool_call.function.arguments;
            let description = params
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("sin descripción");
            let is_destructive = self.registry.is_destructive(name, params);

            if all_mode || !is_destructive {
                if is_destructive && all_mode {
                    println!("  🔴 {}: {} ✅ (all)", name, description);
                } else if !is_destructive {
                    println!("  {}: {} ✅", name, description);
                }
                continue;
            }

            let choice = confirm::confirm_tool(name, description, true);
            match choice {
                ConfirmChoice::No => return false,
                ConfirmChoice::Abort => {
                    println!("  Operación abortada.");
                    return false;
                }
                ConfirmChoice::All => {
                    all_mode = true;
                    continue;
                }
                ConfirmChoice::Yes => continue,
            }
        }
        true
    }

    fn format_plan(&self, tool_calls: &[crate::agent::llm::ToolCall]) -> Vec<String> {
        let mut lines = Vec::new();
        for (i, tc) in tool_calls.iter().enumerate() {
            let params = &tc.function.arguments;
            let desc = params
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("sin descripción");
            let is_destructive = self.registry.is_destructive(&tc.function.name, params);
            let icon = if is_destructive { "🔴" } else { "  " };
            lines.push(format!("{}. {} {} — {}", i + 1, icon, tc.function.name, desc));
        }
        lines
    }

    async fn try_emergency_execution(&self, prompt: &str) -> Option<crate::agent::tools::ToolResult> {
        use crate::agent::tools::Tool;
        let lower = prompt.to_lowercase();
        let bash_tool = crate::agent::tools::bash::BashTool::new(&self.config);

        fn extract_name(s: &str) -> String {
            s.split_whitespace()
                .find(|w| !w.is_empty() && *w != "uno" && *w != "el" && *w != "un" && *w != "en" && *w != "este")
                .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
                .unwrap_or("prueba")
                .to_string()
        }

        // Detectar patron "crea app python que sume o reste" usando templates
        if lower.contains("crea") && (lower.contains("app") || lower.contains("script") || lower.contains("programa") || lower.contains("archivo")) {
            let lang = if lower.contains("python") || lower.contains("py") {
                "python"
            } else if lower.contains("node") || lower.contains("js") || lower.contains("javascript") {
                "node"
            } else if lower.contains("rust") || lower.contains("rs") {
                "rust"
            } else if lower.contains("bash") || lower.contains("sh") || lower.contains("shell") {
                "bash"
            } else {
                "python"
            };
            let app_name = if lower.contains("llamado") {
                lower.split("llamado").nth(1).map(extract_name).unwrap_or_else(|| format!("app_{}", lang))
            } else {
                format!("app_{}", lang)
            };
            let app_type = crate::agent::templates::detect_app_type(&lower);
            let template = crate::agent::templates::generate_app(lang, app_type, &app_name, &lower);
            let commands = crate::agent::templates::project_to_commands(&template);
            let (desc, cmd) = &commands[0];
            tracing::warn!("Ejecucion de emergencia (template): {}", desc);
            return bash_tool.execute(serde_json::json!({
                "command": cmd,
                "description": desc,
            })).await.ok();
        }

        let cmd = if lower.contains("crea") && lower.contains("directorio") {
            let name = lower.split("llamado").nth(1)
                .or_else(|| lower.split("directorio").nth(1))
                .map(extract_name)
                .unwrap_or_else(|| "prueba".to_string());
            format!("mkdir -p {}", name)
        } else if lower.contains("crea") && (lower.contains("archivo") || lower.contains("llamado")) {
            let name = lower.split("llamado").nth(1)
                .map(extract_name)
                .unwrap_or_else(|| "prueba.txt".to_string());
            format!("touch {}", name)
        } else if lower.contains("lista") || lower.contains("ls") {
            "ls -la".to_string()
        } else if lower.contains("elimina") || lower.contains("borra") {
            let name = if lower.contains("archivo") { lower.split("archivo").nth(1) }
                else if lower.contains("directorio") { lower.split("directorio").nth(1) }
                else { None }
                .map(extract_name)
                .unwrap_or_else(|| "prueba".to_string());
            if lower.contains("directorio") { format!("rm -rf {}", name) } else { format!("rm {}", name) }
        } else {
            return None;
        };

        tracing::warn!("Ejecución de emergencia: {}", cmd);
        bash_tool.execute(serde_json::json!({
            "command": cmd,
            "description": "Ejecución automática por emergencia"
        })).await.ok()
    }

    async fn try_recover_from_loop(&self, prompt: &str, messages: &mut Vec<Message>) {
        tracing::warn!("Bucle persistente: intentando recuperacion");
        println!("  ▶ Paso 1: Ejecucion directa de emergencia...");

        // 1. Try emergency execution
        if let Some(emergency_result) = self.try_emergency_execution(prompt).await {
            println!("{}", emergency_result.output);
            messages.push(Message {
                role: "tool".into(),
                content: format!("Ejecucion automatica completada: {}", emergency_result.output.trim()),
                tool_calls: None,
            });
            println!("  ✅ Recuperado via ejecucion directa.");
            return;
        }

        // 2. Try with simple model (no tools)
        let simple_model = &self.config.models.simple;
        tracing::info!("Bucle: cambiando a modelo simple: {}", simple_model);
        println!("  ▶ Paso 2: Cambiando a modelo '{}' (modo texto)...", simple_model);

        messages.push(Message {
            role: "user".into(),
            content: format!("Responde directamente en texto, de forma breve, a esto: {}", prompt),
            tool_calls: None,
        });

        let spinner = self.start_spinner("Recuperando...");
        match self.client.chat_with_tools(simple_model, messages, &[], Some(&spinner)).await {
            Ok(simple_result) => {
                if !simple_result.tool_calls.is_empty() {
                    tracing::info!("Modelo simple tambien uso tools: {:?}", simple_result.tool_calls.iter().map(|tc| &tc.function.name).collect::<Vec<_>>());
                    for tool_call in &simple_result.tool_calls {
                        let name = &tool_call.function.name;
                        let params = &tool_call.function.arguments;
                        if let Some(tool) = self.registry.get(name) {
                            if let Ok(tool_result) = tool.execute(params.clone()).await {
                                println!("{}", tool_result.output);
                                messages.push(Message {
                                    role: "tool".into(),
                                    content: tool_result.output,
                                    tool_calls: None,
                                });
                            }
                        }
                    }
                    messages.push(Message {
                        role: "assistant".into(),
                        content: simple_result.content.clone(),
                        tool_calls: Some(simple_result.tool_calls),
                    });
                    println!("  ✅ Recuperado via modelo simple con tools.");
                    return;
                }
                if !simple_result.content.is_empty() {
                    println!("{}", simple_result.content);
                }
                messages.push(Message {
                    role: "assistant".into(),
                    content: simple_result.content,
                    tool_calls: None,
                });
                println!("  ✅ Recuperado via modelo simple.");
            }
            Err(e) => {
                tracing::error!("Error en recuperacion con modelo simple: {}", e);
                println!("  ⚠️  Error en recuperacion: {}", e);
            }
        }
    }

    fn is_response_looping(&self, responses: &[(String, String)]) -> bool {
        if responses.len() < 3 {
            return false;
        }

        let last = &responses.last().unwrap().0;

        // Si la última respuesta ejecutó tool_calls, no es un bucle
        if !responses.last().unwrap().1.is_empty() {
            return false;
        }

        // Ignorar respuestas muy cortas (< 5 palabras únicas) para evitar falsos positivos
        let last_unique_words: std::collections::HashSet<&str> = last.split_whitespace().collect();
        if last_unique_words.len() < 5 {
            let all_no_tc = responses.iter().all(|(_, tc)| tc.is_empty());
            if all_no_tc && responses.len() >= 5 {
                return true;
            }
            return false;
        }

        let prev: Vec<&str> = responses.iter().rev().skip(1).map(|(c, _)| c.as_str()).collect();

        for earlier in &prev {
            let overlap = self.word_overlap_ratio(last, earlier);
            if overlap > 0.40 {
                return true;
            }
        }

        let all_no_tc = responses.iter().all(|(_, tc)| tc.is_empty());
        if all_no_tc && responses.len() >= 6 {
            return true;
        }

        false
    }

    fn word_overlap_ratio(&self, a: &str, b: &str) -> f64 {
        let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
        let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
        if a_words.is_empty() || b_words.is_empty() {
            return 0.0;
        }
        let intersection = a_words.intersection(&b_words).count();
        let min_len = a_words.len().min(b_words.len());
        intersection as f64 / min_len as f64
    }

    fn prompt_requires_actions(&self, prompt: &str) -> bool {
        let action_keywords = [
            "crea", "elimina", "borra", "instala", "configura",
            "actualiza", "ejecuta", "corre", "busca", "encuentra",
            "modifica", "edita", "escribe", "lee", "analiza",
            "lista", "muestra", "copia", "mueve", "renombra",
            "descarga", "sube", "limpia", "detén", "inicia",
            "reinic", "apaga", "construye", "compila", "formatea",
        ];
        let lower = prompt.to_lowercase();
        action_keywords.iter().any(|kw| lower.contains(kw))
    }

    fn start_spinner(&self, message: &str) -> Arc<AtomicBool> {
        let done = Arc::new(AtomicBool::new(false));
        let d = done.clone();
        let msg = message.to_string();
        tokio::spawn(async move {
            let chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let mut i = 0u64;
            while !d.load(std::sync::atomic::Ordering::Relaxed) {
                let c = chars[(i as usize) % chars.len()];
                print!("\r{} {}", c, msg);
                use std::io::Write;
                std::io::stdout().flush().ok();
                i += 1;
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
        });
        done
    }

    async fn try_summarize(&mut self) {
        let summarize_every = self.config.agent.summarize_every_n_turns;
        if summarize_every == 0 {
            return;
        }

        let (turn_count, skills_active, existing_summary, recent_turns) = {
            let session = match self.session.current() {
                Some(s) => s,
                None => return,
            };

            let tc = session.turns.len();
            if tc == 0 || tc % summarize_every as usize != 0 {
                return;
            }

            let skills = session.skills_active.clone();
            let prev_summary = session.summary.clone();
            let turns: String = session
                .turns
                .iter()
                .rev()
                .take(summarize_every as usize)
                .rev()
                .enumerate()
                .map(|(i, t)| {
                    format!(
                        "Turno {}:\nPrompt: {}\nRespuesta: {}\n\n",
                        tc - summarize_every as usize + i + 1,
                        t.prompt,
                        t.assistant_response,
                    )
                })
                .collect();
            (tc, skills, prev_summary, turns)
        };

        tracing::info!("Generando resumen (turno {}) con modelo {}", turn_count, self.summarizer.model());
        match self.summarizer.summarize(&recent_turns, existing_summary.as_deref(), &skills_active).await {
            Ok(summary) => {
                println!("\n📝 Resumen automático (turno {}):\n{}\n", turn_count, summary);
                if let Err(e) = self.session.set_summary(&summary) {
                    tracing::warn!("Error guardando resumen en sesión: {}", e);
                }
                tracing::info!("Resumen generado y guardado ({} chars)", summary.len());
            }
            Err(e) => {
                tracing::warn!("Error generando resumen: {}", e);
            }
        }
    }

    fn estimated_tokens(&self, messages: &[Message], tool_schemas: &[serde_json::Value]) -> u32 {
        use crate::utils::tokens::{estimate_messages_tokens, estimate_schemas_tokens};
        estimate_messages_tokens(messages) + estimate_schemas_tokens(tool_schemas)
    }

    fn get_checkpoint_max_context(&self) -> u32 {
        self.config.agent.max_context_tokens * 4
    }

    async fn check_context_before_call(&self, _model: &str, prompt: &str, messages: &mut Vec<Message>, tool_schemas: &[serde_json::Value]) {
        let max_context = self.get_checkpoint_max_context();
        let tokens_used = self.estimated_tokens(messages, tool_schemas);
        if max_context == 0 {
            return;
        }

        let usage_pct = (tokens_used as f64 / max_context as f64) * 100.0;
        let threshold = self.config.agent.max_context_tokens as f64 * 0.8;

        let needs_checkpoint = usage_pct > 80.0 || tokens_used as f64 > threshold || self.flags.checkpoint;

        if !needs_checkpoint {
            return;
        }

        let session_id = self.get_session_id();
        let turn_count = self.session.current().map(|s| s.turns.len() as u32).unwrap_or(0);

        println!("\n⚠️  CHECKPOINT: Contexto al {:.1}% ({}/{} tokens)", usage_pct, tokens_used, max_context);
        println!("  ▶ Paso 1: Pausando y re-leyendo proyecto...");

        let project_summary = self.read_project_summary().await;

        println!("  ▶ Paso 2: Guardando checkpoint...");

        let summary_text = if !project_summary.is_empty() {
            format!("Contexto: {:.1}% ({}/{} tokens)\nTurnos: {}\n\n{}",
                usage_pct, tokens_used, max_context, turn_count, project_summary)
        } else {
            format!("Contexto: {:.1}% ({}/{} tokens)\nTurnos: {}",
                usage_pct, tokens_used, max_context, turn_count)
        };

        let cp = Checkpoint {
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            session_id: session_id.clone(),
            context_usage_pct: usage_pct,
            tokens_used,
            max_context,
            turns_completed: turn_count,
            files_modified: Vec::new(),
            summary: summary_text,
            next_action: prompt.to_string(),
        };

        let _ = self.checkpoint_manager.save(&cp);
        let _ = self.checkpoint_manager.save_session_checkpoint(&session_id, &cp);

        println!("  ▶ Paso 3: Inyectando checkpoint en contexto...");

        let resume_prompt = self.checkpoint_manager.format_resume_prompt(&cp);

        messages.push(Message {
            role: "system".into(),
            content: resume_prompt,
            tool_calls: None,
        });

        if self.flags.checkpoint {
            println!("  ✅ Checkpoint manual guardado. Continuando...");
        } else {
            println!("  ✅ Checkpoint automático completado. Reanudando...");
        }
    }

    async fn read_project_summary(&self) -> String {
        let mut summary = String::new();

        if let Ok(entries) = std::fs::read_dir(&self.config.paths.skills_dir) {
            let skill_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .collect();

            if !skill_files.is_empty() {
                summary.push_str(&format!("Skills activos ({}): ", skill_files.len()));
                for f in skill_files.iter() {
                    if let Some(name) = f.path().file_stem() {
                        summary.push_str(&format!("{}, ", name.to_string_lossy()));
                    }
                }
                summary.push('\n');
            }
        }

        if let Ok(current_dir) = std::env::current_dir() {
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                let project_files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| matches!(ext, "rs" | "toml" | "md" | "json"))
                            .unwrap_or(false)
                    })
                    .collect();

                if !project_files.is_empty() {
                    summary.push_str(&format!("Archivos del proyecto ({}): ", project_files.len()));
                    for p in project_files.iter().take(10) {
                        if let Some(name) = p.file_name() {
                            summary.push_str(&format!("{}, ", name.to_string_lossy()));
                        }
                    }
                    summary.push('\n');
                }
            }
        }

        summary
    }

    fn dry_run_plan(&self, tool_calls: &[crate::agent::llm::ToolCall]) {
        println!("\n📋 DRY RUN - Plan de ejecución:");
        for (i, tool_call) in tool_calls.iter().enumerate() {
            let name = &tool_call.function.name;
            let description = tool_call.function.arguments
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("sin descripción");
            let command = tool_call.function.arguments
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            println!("  {}. {} - {}", i + 1, name, description);
            if !command.is_empty() {
                println!("     `{}`", command);
            }
        }
        println!("  (modo dry-run: no se ejecutó nada)\n");
    }

    pub fn get_core_services(&self) -> &CoreServices {
        &self.core_services
    }

    pub fn get_skills(&self) -> &SkillsLoader {
        &self.skills
    }

    pub fn get_router(&self) -> &ModelRouter {
        &self.router
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(ref mcp_reg) = self.mcp_registry {
            let reg = mcp_reg.clone();
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::warn!("Failed to create runtime for MCP shutdown: {}", e);
                        return;
                    }
                };
                rt.block_on(async move {
                    let mut registry = reg.lock().await;
                    registry.shutdown_all().await;
                });
            });
        }

        if !self.plugin_runtimes.is_empty() {
            let runtimes: Vec<_> = self.plugin_runtimes.drain().map(|(name, rt)| (name, rt)).collect();
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::warn!("Failed to create runtime for plugin shutdown: {}", e);
                        return;
                    }
                };
                rt.block_on(async move {
                    for (name, runtime_arc) in runtimes {
                        let mut runtime = runtime_arc.lock().await;
                        tracing::info!("Shutting down plugin '{}'", name);
                        let _ = runtime.shutdown().await;
                    }
                });
            });
        }
    }
}

fn detect_os() -> String {
    if let Ok(output) = std::process::Command::new("sh")
        .arg("-c")
        .arg("uname -a 2>/dev/null || echo 'unknown'")
        .output()
    {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    async fn make_agent() -> Agent {
        Agent::new(Config::default(), GlobalFlags {
            batch: false,
            reasoning: false,
            fast: false,
            dry_run: false,
            destructive: false,
            daemon: None,
            checkpoint: false,
        }).await
    }

    #[tokio::test]
    async fn test_word_overlap_identical() {
        let agent = make_agent().await;
        let a = "Voy a crear el archivo prueba";
        let b = "Voy a crear el archivo prueba";
        let ratio = agent.word_overlap_ratio(a, b);
        assert!((ratio - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_word_overlap_similar() {
        let agent = make_agent().await;
        let a = "Lamento la confusión, vamos a intentarlo de nuevo";
        let b = "Lamento la confusión anterior, pero vamos a intentarlo nuevamente";
        let ratio = agent.word_overlap_ratio(a, b);
        assert!(ratio > 0.5);
    }

    #[tokio::test]
    async fn test_word_overlap_different() {
        let agent = make_agent().await;
        let a = "El directorio contiene los siguientes archivos";
        let b = "Creando el archivo prueba";
        let ratio = agent.word_overlap_ratio(a, b);
        assert!(ratio < 0.3);
    }

    #[tokio::test]
    async fn test_word_overlap_empty() {
        let agent = make_agent().await;
        assert_eq!(agent.word_overlap_ratio("", "test"), 0.0);
        assert_eq!(agent.word_overlap_ratio("test", ""), 0.0);
        assert_eq!(agent.word_overlap_ratio("", ""), 0.0);
    }

    #[tokio::test]
    async fn test_is_response_looping_detects_repetition() {
        let agent = make_agent().await;
        let responses = vec![
            ("Voy a crear el archivo".into(), "".into()),
            ("Voy a crear el archivo nuevamente".into(), "".into()),
            ("Voy a crear el archivo otra vez".into(), "".into()),
        ];
        assert!(agent.is_response_looping(&responses));
    }

    #[tokio::test]
    async fn test_is_response_looping_normal_progress() {
        let agent = make_agent().await;
        let responses = vec![
            ("Listando archivos...".into(), "bash".into()),
            ("Archivo creado".into(), "write".into()),
            ("Archivo eliminado".into(), "bash".into()),
        ];
        assert!(!agent.is_response_looping(&responses));
    }

    #[tokio::test]
    async fn test_is_response_looping_all_no_tc() {
        let agent = make_agent().await;
        let responses = vec![
            ("Respuesta uno".into(), "".into()),
            ("Respuesta dos".into(), "".into()),
            ("Respuesta tres".into(), "".into()),
            ("Respuesta cuatro".into(), "".into()),
            ("Respuesta cinco".into(), "".into()),
        ];
        assert!(agent.is_response_looping(&responses));
    }

    #[tokio::test]
    async fn test_is_response_looping_not_enough() {
        let agent = make_agent().await;
        let responses = vec![
            ("Una respuesta".into(), "".into()),
        ];
        assert!(!agent.is_response_looping(&responses));
    }

    #[tokio::test]
    async fn test_is_response_looping_two_only() {
        let agent = make_agent().await;
        let responses = vec![
            ("Uno".into(), "".into()),
            ("Dos".into(), "".into()),
        ];
        assert!(!agent.is_response_looping(&responses));
    }

    #[tokio::test]
    async fn test_prompt_requires_actions() {
        let agent = make_agent().await;
        assert!(agent.prompt_requires_actions("crea un archivo"));
        assert!(agent.prompt_requires_actions("elimina el directorio prueba"));
        assert!(agent.prompt_requires_actions("lista los archivos"));
        assert!(!agent.prompt_requires_actions("qué es rust"));
        assert!(!agent.prompt_requires_actions("hola mundo"));
    }

    #[tokio::test]
    async fn test_init_mcp_no_servers() {
        let mut agent = make_agent().await;
        // No servers configured, init_mcp should do nothing
        agent.init_mcp().await;
        assert!(agent.mcp_registry.is_none());
    }

    #[tokio::test]
    async fn test_init_mcp_disabled() {
        let mut config = Config::default();
        config.mcp.enabled = false;
        let mut agent = Agent::new(config, GlobalFlags {
            batch: false,
            reasoning: false,
            fast: false,
            dry_run: false,
            destructive: false,
            daemon: None,
            checkpoint: false,
        }).await;
        agent.init_mcp().await;
        assert!(agent.mcp_registry.is_none());
    }

    #[tokio::test]
    async fn test_agent_mcp_tools_registered_with_server() {
        let mut config = Config::default();
        config.mcp.enabled = true;
        config.mcp.servers = vec![crate::mcp::types::McpServerConfig {
            name: "test".into(),
            command: "python3".into(),
            args: vec!["test_mcp_server.py".into()],
            enabled: true,
            env: HashMap::new(),
        }];

        let mut agent = Agent::new(config, GlobalFlags {
            batch: false,
            reasoning: false,
            fast: false,
            dry_run: false,
            destructive: false,
            daemon: None,
            checkpoint: false,
        }).await;
        agent.init_mcp().await;

        let schemas = agent.registry.all_schemas();
        let has_mcp_tool = schemas.iter().any(|s| {
            s["function"]["name"]
                .as_str()
                .map(|n| n.starts_with("mcp:"))
                .unwrap_or(false)
        });
        assert!(has_mcp_tool, "MCP tools should be registered when server is configured");
    }

    #[tokio::test]
    async fn test_agent_new_creates_with_session() {
        let mut agent = make_agent().await;
        let _ = agent.run("hola").await;
        assert!(!agent.get_session_id().is_empty());
    }

    #[tokio::test]
    async fn test_format_plan_empty() {
        let agent = make_agent().await;
        let plan = agent.format_plan(&[]);
        assert!(plan.is_empty());
    }

    #[tokio::test]
    async fn test_format_plan_with_tool_calls() {
        let agent = make_agent().await;
        let tool_calls = vec![
            crate::agent::llm::ToolCall {
                call_type: None,
                function: crate::agent::llm::ToolFunction {
                    name: "bash".into(),
                    arguments: serde_json::json!({"command": "ls", "description": "list files"}),
                },
            },
        ];
        let plan = agent.format_plan(&tool_calls);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].contains("bash"));
    }

    #[tokio::test]
    async fn test_prompt_requires_actions_negative() {
        let agent = make_agent().await;
        assert!(!agent.prompt_requires_actions("qué es rust"));
        assert!(!agent.prompt_requires_actions("hola mundo"));
        assert!(!agent.prompt_requires_actions("gracias"));
    }

    #[tokio::test]
    async fn test_dry_run_prints_plan() {
        let agent = make_agent().await;
        let tool_calls = vec![
            crate::agent::llm::ToolCall {
                call_type: None,
                function: crate::agent::llm::ToolFunction {
                    name: "bash".into(),
                    arguments: serde_json::json!({"command": "echo test", "description": "test echo"}),
                },
            },
        ];
        agent.dry_run_plan(&tool_calls);
    }

    #[tokio::test]
    async fn test_word_overlap_high_overlap() {
        let agent = make_agent().await;
        let a = "Estoy intentando crear el archivo de configuración";
        let b = "Voy a crear el archivo de configuración del sistema";
        let ratio = agent.word_overlap_ratio(a, b);
        assert!(ratio > 0.5);
    }

    #[tokio::test]
    async fn test_word_overlap_no_overlap() {
        let agent = make_agent().await;
        let a = "El cielo es azul";
        let b = "Los archivos fueron eliminados";
        let ratio = agent.word_overlap_ratio(a, b);
        assert!(ratio < 0.3);
    }
}
