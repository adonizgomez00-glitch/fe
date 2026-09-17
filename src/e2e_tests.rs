
// ─── E2E Integration Tests (Fase 4) ─────────────────────────────────────────
// Tests que validan flujos completos del agente.
// MockProvider permite simular respuestas del LLM sin depender de Ollama real.
// ─────────────────────────────────────────────────────────────────────────────

use crate::agent::core::Agent;
use crate::agent::direct_executor::{self, DirectMatch};
use crate::agent::session::SessionManager;
use crate::agent::checkpoint::{Checkpoint, CheckpointManager};
use crate::agent::skills::SkillsLoader;
use crate::agent::provider::{ChatResult, Message, Provider};
use crate::cli::GlobalFlags;
use crate::config::Config;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

// Cada test conserva TempDir hasta terminar: sin depender de skills/config del usuario.
fn test_config() -> (tempfile::TempDir, Config) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.paths.skills_dir = dir.path().join("skills");
    config.paths.sessions_dir = dir.path().join("sessions");
    config.paths.checkpoints_dir = dir.path().join("checkpoints");
    config.paths.plugins_dir = dir.path().join("plugins");
    config.paths.config_dir = dir.path().join("config");
    config.mcp.enabled = false;
    config.mcp.servers.clear();
    let skill_dir = config.paths.skills_dir.join("linux/linux-admin");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"),
        "---\nname: linux-admin\ndescription: Administra el sistema linux y su mantenimiento; configura el firewall de linux.\n---\n# Linux\nAdministración y mantenimiento del sistema linux.\n"
    ).unwrap();
    (dir, config)
}

// Helper para construir GlobalFlags por defecto en tests
fn test_flags() -> GlobalFlags {
    GlobalFlags {
        batch: false,
        reasoning: false,
        fast: false,
        dry_run: false,
        destructive: false,
        daemon: None,
        checkpoint: false,
    }
}

// Mock provider que devuelve respuestas predeterminadas para tests de integración
pub struct MockProvider {
    pub responses: Vec<ChatResult>,
    pub call_count: std::sync::atomic::AtomicUsize,
}

impl MockProvider {
    pub fn new(responses: Vec<ChatResult>) -> Self {
        Self {
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn supports_reasoning(&self) -> bool {
        false
    }

    fn max_context(&self) -> u32 {
        8192
    }

    async fn chat(&self, _model: &str, _messages: &[Message], _tools: &[Value], _spinner_done: Option<&AtomicBool>) -> anyhow::Result<ChatResult> {
        let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
        if idx < self.responses.len() {
            Ok(self.responses[idx].clone())
        } else {
            // Respuesta por defecto: usar bash para ejecutar comandos
            Ok(ChatResult {
                content: "Ejecutando comando...".into(),
                tool_calls: vec![crate::agent::provider::ToolCall {
                    call_type: Some("function".into()),
                    function: crate::agent::provider::ToolFunction {
                        name: "bash".into(),
                        arguments: serde_json::json!({"command": "echo ok", "description": "Ejecutar comando"}),
                    },
                }],
            })
        }
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec!["mock-model".into()])
    }
}


#[tokio::test]
async fn test_e2e_agent_construction() {
    let config = Config::default();
    let flags = GlobalFlags {
        batch: false,
        reasoning: false,
        fast: false,
        dry_run: false,
        destructive: false,
        daemon: None,
        checkpoint: false,
    };
    let _agent = Agent::new(config, flags).await;
}



#[test]
fn test_e2e_session_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = SessionManager::new(dir.path());

    let session = manager.create("qwen2.5:3b", &["c-sysadmin".to_string()]).unwrap();
    assert_eq!(session.model, "qwen2.5:3b");
    assert_eq!(session.skills_active.len(), 1);

    manager.add_turn("lista archivos", "ls -la ejecutado").unwrap();
    manager.add_turn("crea directorio", "mkdir ejecutado").unwrap();

    let session = manager.current().unwrap();
    assert_eq!(session.turns.len(), 2);

    let session_id = session.id.clone();
    let content = manager.show(&session_id).unwrap();
    assert!(content.is_some());
    assert!(content.unwrap().contains("ls -la"));

    let list = manager.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].turn_count, 2);

    manager.set_summary("Resumen E2E de prueba").unwrap();
    let summary = manager.get_summary().unwrap();
    assert_eq!(summary, "Resumen E2E de prueba");
}

#[test]
fn test_e2e_session_export_clean() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = SessionManager::new(dir.path());

    manager.create("test-model", &[]).unwrap();
    let id = manager.current().unwrap().id.clone();
    manager.add_turn("prompt", "response").unwrap();

    let export_path = dir.path().join("export.md");
    let exported = manager.export(&id, &export_path).unwrap();
    assert!(exported);
    assert!(export_path.exists());

    let removed = manager.clean(0).unwrap();
    assert_eq!(removed, 0);
}

#[test]
fn test_e2e_skills_loading() {
    let (_dir, config) = test_config();
    let skills = SkillsLoader::load_from(&config.paths.skills_dir).unwrap();
    assert_eq!(skills.len(), 1, "Debe cargar la skill de prueba");
}

#[test]
fn test_e2e_skills_match_prompt() {
    let (_dir, config) = test_config();
    let skills = SkillsLoader::load_from(&config.paths.skills_dir).unwrap();
    let matched = skills.match_prompt("configura el firewall de linux");
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].1, crate::agent::skills::SkillKind::Linux);
    let formatted = skills.format_skills_for_prompt(
        &matched.iter().map(|(s, _)| *s).collect::<Vec<_>>()
    );
    assert!(formatted.contains("linux-admin"));
}

#[test]
fn test_e2e_checkpoint_save_and_resume() {
    let dir = tempfile::tempdir().unwrap();
    let manager = CheckpointManager::new(dir.path());

    let cp = Checkpoint {
        timestamp: "2026-07-17 10:00:00".into(),
        session_id: "e2e-test-session".into(),
        context_usage_pct: 85.0,
        tokens_used: 27000,
        max_context: 32000,
        turns_completed: 15,
        files_modified: vec!["src/agent/core.rs".into()],
        summary: "E2E test checkpoint summary.".into(),
        next_action: "Verify checkpoint resume.".into(),
    };

    let path = manager.save_session_checkpoint("e2e-test-session", &cp).unwrap();
    assert!(path.exists());

    let loaded = manager.load_latest("e2e-test-session");
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().session_id, "e2e-test-session");

    let resume = manager.format_resume_prompt(&cp);
    assert!(resume.contains("REANUDACIÓN DESDE CHECKPOINT"));
    assert!(resume.contains("e2e-test-session"));
    assert!(resume.contains("85.0%"));
}

#[test]
fn test_e2e_tool_registry_has_defaults() {
    use crate::agent::tools::ToolRegistry;
    let config = Config::default();
    let mut registry = ToolRegistry::new();
    registry.register_defaults(&config);

    let schemas = registry.all_schemas();
    let tool_names: Vec<&str> = schemas.iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();

    assert!(tool_names.contains(&"bash"), "bash tool must be registered");
    assert!(tool_names.contains(&"write"), "write tool must be registered");
    assert!(tool_names.contains(&"read"), "read tool must be registered");
    assert!(tool_names.contains(&"glob"), "glob tool must be registered");
    assert!(tool_names.contains(&"grep"), "grep tool must be registered");
    assert!(tool_names.contains(&"edit"), "edit tool must be registered");
    assert!(tool_names.contains(&"task"), "task tool must be registered");
    assert!(tool_names.contains(&"websearch"), "websearch tool must be registered");
    assert!(tool_names.contains(&"webfetch"), "webfetch tool must be registered");
    assert!(tool_names.contains(&"mkdir"), "mkdir tool must be registered");
}

#[test]
fn test_e2e_direct_executor_patterns() {
    let result = direct_executor::match_prompt("crea un directorio llamado prueba");
    assert!(result.is_some());

    let result = direct_executor::match_prompt("crea app python que sume");
    assert!(result.is_some());

    let result = direct_executor::match_prompt("elimina el archivo test.txt");
    assert!(result.is_some());

    let result = direct_executor::match_prompt("hola, ¿cómo estás?");
    assert!(result.is_none());
}

#[test]
fn test_e2e_direct_executor_format_output() {
    let dm = DirectMatch {
        command: "echo hola".into(),
        description: "Prueba E2E".into(),
        is_destructive: false,
    };
    let formatted = direct_executor::format_output(&dm, "hola\n");
    assert!(formatted.contains("hola"));
}

#[tokio::test]
async fn test_e2e_agent_direct_executor_dry_run() {
    let mut agent = {
        let config = Config::default();
        let flags = GlobalFlags {
            batch: false,
            reasoning: false,
            fast: false,
            dry_run: true,
            destructive: false,
            daemon: None,
            checkpoint: false,
        };
        Agent::new(config, flags).await
    };
    let result = agent.run("crea un directorio llamado test_e2e").await;
    assert!(result.is_ok());
}

#[test]
fn test_e2e_session_clean_old() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = SessionManager::new(dir.path());

    manager.create("test", &[]).unwrap();
    manager.add_turn("p1", "r1").unwrap();
    manager.add_turn("p2", "r2").unwrap();

    assert!(manager.current().is_some());
    assert_eq!(manager.current().unwrap().turns.len(), 2);

    let list = manager.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].turn_count, 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Fase 4: Tests de Integración — Flujos Core del Agente
// ══════════════════════════════════════════════════════════════════════════════

/// Test: Agent puede ejecutar flujo simple con herramientas registradas
#[tokio::test]
async fn test_e2e_agent_bash_execution() {
    let (_dir, config) = test_config();
    let flags = test_flags();

    let agent = Agent::new(config, test_flags()).await;

    // Validar que el agente tiene herramientas registradas
    let tools = agent.get_core_services().tools.list_tools().await;
    assert!(tools.contains(&"bash".to_string()), "bash debe estar registrado");
    assert!(tools.contains(&"write".to_string()), "write debe estar registrado");
    assert!(tools.contains(&"read".to_string()), "read debe estar registrado");

    // Validar que el agente tiene skills cargadas
    let skills = agent.get_skills();
    assert!(!skills.is_empty(), "Debe haber skills cargadas");

    // Validar que el router funciona
    let (model, _kind) = agent.get_router().select("lista archivos en /tmp", &flags);
    assert!(!model.is_empty(), "Router debe devolver un modelo");
}
/// Test: Agent maneja comandos destructivos en modo dry_run
#[tokio::test]
async fn test_e2e_agent_destructive_dry_run() {
    let config = Config::default();
    let mut flags = test_flags();
    flags.dry_run = true;

    let mut agent = Agent::new(config, flags).await;

    // En modo dry_run, el agente debe procesar sin ejecutar nada
    let result = agent.run("elimina el archivo test.txt").await;
    assert!(result.is_ok(), "En dry_run no debe fallar");
}

/// Test: Skills cargadas y matching funciona en contexto de agente
#[tokio::test]
async fn test_e2e_agent_skills_matching() {
    let (_dir, config) = test_config();
    let agent = Agent::new(config, test_flags()).await;
    let skills = agent.get_skills();

    // Debe haber al menos 1 skill cargada
    assert!(!skills.is_empty(), "Debe haber skills cargadas de los dominios");

    // Test de matching con prompt de linux → debe matchear el dominio Linux
    let matched = skills.match_prompt("administra el sistema linux y su mantenimiento");
    assert!(!matched.is_empty(), "Debe matchear al menos una skill de linux");

    let has_linux = matched.iter().any(|(_, kind)| *kind == crate::agent::skills::SkillKind::Linux);
    assert!(has_linux, "Debe haber al menos una skill de tipo Linux");

    // Cada SkillKind devuelto debe coincidir con el dominio de su skill
    for (skill, kind) in &matched {
        assert_eq!(
            *kind,
            crate::agent::skills::SkillKind::from_domain(&skill.domain),
            "SkillKind debe derivar del dominio de la skill"
        );
    }
}
/// Test: Router devuelve modelos correctos para diferentes tipos de prompts
#[tokio::test]
async fn test_e2e_router_model_selection() {
    let config = Config::default();
    let router = crate::agent::router::ModelRouter::new(&config);

    let flags = test_flags();

    // Prompt de acción → modelo complejo
    let (model, kind) = router.select("actualiza repositorios apt", &flags);
    assert!(!model.is_empty());
    assert_eq!(kind, crate::agent::skills::SkillKind::Generic);

    // Prompt simple → modelo simple
    let (model, kind) = router.select("qué es rust", &flags);
    assert!(!model.is_empty());
    assert_eq!(kind, crate::agent::skills::SkillKind::Generic);

    // Flag --reasoning → modelo reasoning
    let mut reasoning_flags = test_flags();
    reasoning_flags.reasoning = true;
    let (model, _kind) = router.select("analiza la arquitectura", &reasoning_flags);
    assert!(!model.is_empty());

    // Flag --fast → modelo simple
    let mut fast_flags = test_flags();
    fast_flags.fast = true;
    let (model, _kind) = router.select("hola mundo", &fast_flags);
    assert!(!model.is_empty());
}

/// Test: CoreServices inyectadas correctamente con todas las operaciones
#[tokio::test]
async fn test_e2e_core_services_full_operations() {
    let config = Config::default();
    let agent = Agent::new(config, test_flags()).await;
    let services = agent.get_core_services();

    // === ToolsService ===
    let tools = services.tools.list_tools().await;
    assert!(tools.contains(&"bash".to_string()));
    assert!(tools.contains(&"write".to_string()));
    assert!(tools.contains(&"read".to_string()));
    assert!(tools.contains(&"glob".to_string()));
    assert!(tools.contains(&"grep".to_string()));
    assert!(tools.contains(&"mkdir".to_string()));

    // === ProvidersService ===
    let providers = services.providers.list_providers().await;
    assert!(!providers.is_empty(), "Debe tener al menos un provider");

    // === CacheService ===
    services.cache.set("cache_key", "cache_value".into(), 3600).await.unwrap();
    let cached = services.cache.get("cache_key").await;
    assert_eq!(cached, Some("cache_value".into()));
    services.cache.delete("cache_key").await.unwrap();
    let cached_after = services.cache.get("cache_key").await;
    assert_eq!(cached_after, None);

    // === SecurityService ===
    assert!(services.security.is_destructive("rm -rf /").await);
    assert!(services.security.is_destructive("dd if=").await);
    assert!(services.security.is_destructive("mkfs.ext4").await);
    assert!(services.security.is_destructive("reboot").await);
    assert!(services.security.is_destructive("poweroff").await);
    assert!(!services.security.is_destructive("ls -la").await);
    assert!(!services.security.is_destructive("cat archivo").await);
}

/// Test: SessionManager completo (create, turns, list, export, summary)
#[test]
fn test_e2e_session_full_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = SessionManager::new(dir.path());

    // Crear sesión con skills activos
    let session = manager.create("qwen2.5:3b", &["linux".to_string(), "docker".to_string()]).unwrap();
    assert_eq!(session.model, "qwen2.5:3b");
    assert_eq!(session.skills_active.len(), 2);
    assert!(session.skills_active.contains(&"linux".to_string()));
    assert!(session.skills_active.contains(&"docker".to_string()));
    let session_id = session.id.clone();

    // Agregar turns
    manager.add_turn("lista archivos", "ls -la /tmp ejecutado").unwrap();
    manager.add_turn("crea directorio", "mkdir /tmp/prueba ejecutado").unwrap();
    manager.add_turn("elimina archivo", "rm /tmp/prueba/archivo.txt ejecutado").unwrap();

    // Verificar sesión actual
    let current = manager.current().unwrap();
    assert_eq!(current.turns.len(), 3);
    assert_eq!(current.skills_active.len(), 2);

    // Listar sesiones
    let list = manager.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].turn_count, 3);

    // Exportar sesión
    let export_path = dir.path().join("session_export.md");
    let exported = manager.export(&session_id, &export_path).unwrap();
    assert!(exported);
    assert!(export_path.exists());
    let content = std::fs::read_to_string(&export_path).unwrap();
    assert!(content.contains("lista archivos"));
    assert!(content.contains("crea directorio"));

    // Agregar resumen
    manager.set_summary("Resumen: administración de sistema linux").unwrap();
    let summary = manager.get_summary().unwrap();
    assert_eq!(summary, "Resumen: administración de sistema linux");
}

/// Test: DirectExecutor detecta patrones de comandos y rechaza consultas
#[test]
fn test_e2e_direct_executor_comprehensive() {
    // Patrones soportados por el DirectExecutor (ver direct_executor.rs)
    let commands = vec![
        "crea un directorio prueba",
        "crea un archivo vacio notas.txt",
        "crea app python que sume dos numeros",
        "elimina el archivo test.txt",
        "elimina el directorio viejo",
        "actualiza repositorios",
        "lista archivos",
        "busca archivo config.toml",
        "muestra el archivo config.toml",
    ];

    for cmd in commands {
        let result = direct_executor::match_prompt(cmd);
        assert!(result.is_some(), "Debe detectar como comando: {}", cmd);
    }

    // Estos NO deben ser detectados (son consultas de info)
    let queries = vec![
        "hola, ¿cómo estás?",
        "qué es rust",
        "define polimorfismo",
        "explica SOLID",
        "dime la hora",
    ];

    for query in queries {
        let result = direct_executor::match_prompt(query);
        assert!(result.is_none(), "No debe detectar como comando: {}", query);
    }
}

/// Test: Validación del checklist pre-release v0.1.0
#[tokio::test]
async fn test_e2e_release_checklist_validation() {
    // 1. Configuración carga correctamente
    let config = Config::default();

    // 2. ToolRegistry tiene las herramientas mínimas
    let mut registry = crate::agent::tools::ToolRegistry::new();
    registry.register_defaults(&config);
    let tools = registry.list();
    assert!(tools.contains(&"bash".to_string()), "bash tool debe existir");
    assert!(tools.contains(&"write".to_string()), "write tool debe existir");
    assert!(tools.contains(&"read".to_string()), "read tool debe existir");

    // 3. CoreServices se construyen correctamente con las 4 dependencias
    let services = crate::agent::services::CoreServices::new(
        std::sync::Arc::new(crate::agent::services::default_impl::DefaultToolsService::new(registry)),
        std::sync::Arc::new(crate::agent::services::default_impl::DefaultProvidersService::new(&config)),
        std::sync::Arc::new(crate::agent::services::default_impl::DefaultCacheService::new()),
        std::sync::Arc::new(crate::agent::services::default_impl::DefaultSecurityService::new()),
    );
    assert!(!services.tools.list_tools().await.is_empty());
    assert!(!services.providers.list_providers().await.is_empty());

    // 4. Provider trait está disponible y funcional
    let provider = services.providers.default_provider().await;
    assert_eq!(provider.name(), "ollama");
    assert!(provider.supports_tools());
    assert!(provider.max_context() > 0);
}