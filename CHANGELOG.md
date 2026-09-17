# CHANGELOG - Fe

## [0.1.0] - 2026-07-10

### Release v0.1.0 — Fe

**173 tests**, 0 failures. Primer release estable de Fe, agente CLI local para Ollama.

### Features Principales

#### Foundation
- CLI completo con `clap` (flags globales + subcomandos)
- Config loader con merge de defaults + `config.toml` + env vars `FE_*`
- Cliente Ollama async con streaming, retry exponencial y timeouts
- Router de modelos por heurística (~100 conjugaciones verbales)
- 10 herramientas nativas: bash, read, write, edit, glob, grep, task, webfetch, websearch, mkdir
- Confirmación híbrida: tool-by-tool o `--batch` con detección de destructivos
- `--dry-run` para ver plan sin ejecutar

#### Direct Execution
- Comandos comunes (ls, pwd, mkdir, rm, cat, touch, apt update) ejecutados sin LLM
- Regex flexibles con variantes de lenguaje natural ("haz ls", "muestra archivos")
- Confirmación automática para comandos no destructivos

#### Skills + Sessions
- Parser de skills opencode (SKILL.md con frontmatter)
- Keyword matcher TF-IDF con auto-detección desde el prompt
- Inyección de skills en system prompt
- Sesiones persistentes en markdown con listado, exportación y limpieza
- 18 skills opencode soportadas (categorías A-D)

#### Summarizer
- Resumen automático cada N turns (default 4) con gemma3:4b
- Skills tags preservados en resúmenes
- Merge con resumen anterior para contexto incremental

#### Template System
- 13 generadores multi-lenguaje × tipo de aplicación
- Lenguajes: Python (CLI/Desktop/Web/Admin/Doc), Node (CLI/Web/Desktop), Rust (CLI/Web), Bash (CLI/Admin)
- Código funcional con tests, requirements, logging y manejo de errores
- Integración con direct execution y emergency recovery

#### Anti-Bucle + Recovery
- Detección de bucles por solapamiento de palabras (threshold 0.40)
- Recuperación en 2 pasos: ejecución directa → modelo simple
- System prompt con 19 reglas estrictas y prohibición de "haz una pausa"
- Emergency execution para "crea app X"

#### Rendimiento
- GPU Iris Xe vía Vulkan: 2-5x tokens/segundo
- OLLAMA_KEEP_ALIVE=300s: modelo persistente 5min en RAM
- KV Cache en q8_0 para menor uso de RAM
- OLLAMA_NUM_THREADS=4: solo P-cores del i7-1265U
- Swap 32GB para evitar OOM

#### Shell Completions + Doctor
- Generación de completions para bash/zsh/fish
- `fe doctor`: diagnóstico de Ollama, configuración, sesiones y skills

### MCP (Model Context Protocol) — Día 7 (Post v0.1.0)

- Cliente MCP completo con transporte stdio (JSON-RPC 2.0 line-delimited)
- Tipos MCP: McpServerConfig, McpTool, McpResource, McpPrompt, McpPromptArgument, InitializeResult
- McpClient: spawn, initialize, list_tools, call_tool, list_resources, read_resource, list_prompts, get_prompt
- McpRegistry: multi-server discovery, graceful error handling, cleanup en Drop
- ToolRegistry.register_mcp_tools(): registra tools MCP como `mcp:{server}:{tool}` en ToolRegistry
- MCP ToolWrapper implementa trait Tool delegando a registry.call_tool()
- CLI completo:
  - `fe mcp discover` — tools + resources + prompts de todos los servers
  - `fe mcp servers` — lista servers + estado
  - `fe mcp resources` — solo recursos
  - `fe mcp prompts` — solo prompts
  - `fe mcp call mcp:server:tool '{}'` — ejecuta tool
  - `fe mcp read server uri` — lee recurso
  - `fe mcp get-prompt server name '{}'` — obtiene prompt
- Configuración: `[mcp]` section con `enabled` y `[[mcp.servers]]`
- Servidor test Python con tools (echo, ping, greet), resources (greeting://hello, info://version), prompts (say_hello, show_info)
- Tests: 11 nuevos (types + cli + registry), total 252 passing

### Lista Completa de Archivos

```
fe/
├── Cargo.toml
├── config.toml.example
├── install.sh
├── README.md
├── CHANGELOG.md
├── ARCHITECTURE.md
├── ROADMAP.md
├── SPECIFICATION.md
├── IMPL_CONTEXT.md
├── TODO.md
├── PHASE5_PLAN.md
├── docs/
│   ├── user-guide.md
│   └── configuration.md
└── src/
    ├── main.rs
    ├── cli.rs
    ├── config.rs
    ├── agent/
    │   ├── mod.rs
    │   ├── core.rs
    │   ├── llm.rs
    │   ├── router.rs
    │   ├── direct_executor.rs
    │   ├── templates.rs
    │   ├── skills.rs
    │   ├── session.rs
    │   ├── summarizer.rs
    │   └── tools/
    │       ├── mod.rs
    │       ├── bash.rs
    │       ├── read.rs
    │       ├── write.rs
    │       ├── edit.rs
    │       ├── glob.rs
    │       ├── grep.rs
    │       ├── task.rs
    │       ├── webfetch.rs
    │       ├── websearch.rs
    │       └── mkdir.rs
    └── utils/
        ├── confirm.rs
        └── spinner.rs
```

### MCP Integration in Agent Core — Día 8 (2026-07-10)

- **`src/agent/core.rs`**: `Agent` struct con `mcp_registry: Option<Arc<Mutex<McpRegistry>>>`
- Inicialización lazy vía `init_mcp()` en `run()` (Opción B, sin breaking changes en `Agent::new()`)
- MCP tools registradas en `ToolRegistry` como `mcp:{server}:{tool}` automáticamente
- `Drop` impl para graceful shutdown de servidores MCP en background
- Tests: `test_init_mcp_no_servers`, `test_init_mcp_disabled`, `test_agent_mcp_tools_registered_with_server`
- **255 tests** pasando (anterior: 252)

### Plugin System Base — Día 9 (2026-07-10)

- **`src/plugins/manifest.rs`**: `PluginManifest`, `PluginTool`, `PluginCommand` con serde deserialización desde `plugin.toml`
- **`src/plugins/loader.rs`**: `load_plugins()` escanea `~/.config/fe/plugins/*/plugin.toml` → `Vec<LoadedPlugin>`
- **`src/plugins/registry.rs`**: `PluginRegistry` con `HashMap<String, LoadedPlugin>`, métodos CRUD + `reload()`
- **`src/config.rs`**: `PathsConfig` ahora con `plugins_dir` (default: `~/.config/fe/plugins`)
- **`src/cli.rs`**: Nuevo subcomando `Plugin(PluginAction)` con `fe plugins list`
- **`src/main.rs`**: `handle_plugin()` — lista plugins instalados con tools y comandos
- 23 tests nuevos (6 manifest + 7 loader + 10 registry)
- **278 tests** pasando

### Plugin Runtime + CLI + Agent Integration — Día 10 (2026-07-11)

- **`src/plugins/runtime.rs`**: `PluginRuntime` ejecuta scripts via subprocess con protocolo JSON-RPC 2.0
  - `spawn()`: lanza proceso plugin con pipes stdin/stdout
  - `call_tool()` / `call_command()`: envía solicitudes JSON-RPC
  - `shutdown()`: graceful stop + kill
  - 5 tests unitarios con plugin Python inline
- **`src/agent/tools/mod.rs`**: `PluginToolWrapper` implementa `Tool` trait delegando a `PluginRuntime`
  - `register_plugin_tools()`: registra tools como `plugin:{name}:{tool}`
- **`src/agent/core.rs`**: `init_plugins()` lazy en `run()`
  - Campo `plugin_runtimes: HashMap<String, Arc<Mutex<PluginRuntime>>>`
  - Spawnea plugins y registra tools automáticamente
  - `Drop` limpia todos los runtimes en background
- **`src/cli.rs` + `src/main.rs`**: Nuevos subcomandos
  - `fe plugin reload` — recarga plugins del disco
  - `fe plugin run <name> <cmd> [args...]` — ejecuta comando
  - `fe plugin install <source>` — esqueleto
  - `fe plugin uninstall <name>` — elimina directorio
  - `parse_cli_args_to_json()`: convierte `--key value` a JSON para plugins
- **283 tests** pasando (anterior: 278)

### Tests
- **283 tests unitarios**, 0 failures
- 5 nuevos tests de plugin runtime
- Validación E2E: `fe plugin run test-plugin greet --nombre Mundo` → `Hola, Mundo!`

## [0.1.1] - 2026-07-11

### Hybrid Plan Fase 1 — Provider Trait + Router SkillKind

#### Provider Trait
- `src/agent/provider.rs`: `trait Provider` + `OllamaProvider` (envuelve HTTP chat)
- `src/agent/llm.rs`: `OllamaClient` ahora inyecta `Box<dyn Provider>` (delega chat)
- 10 tests unitarios (chat, retry, list_models, edge cases)

#### SkillKind Enum
- `SkillKind`: Linux, Docker, Python, Cobol, PowerBI, Kubernetes, PostgreSQL, Security, Git, Generic
- `SkillKind::from_domain()` mapea string a variante
- `match_prompt()` devuelve `Vec<(&Skill, SkillKind)>`

#### Router + SkillKind
- `select()` retorna `(String, SkillKind)` en vez de solo `String`
- Tests actualizados para destructuring

#### Tests
- **309 tests unitarios**, 0 failures (26 nuevos, meta 300+)
- `cargo build --release` — 0 errors

## [0.1.2] - 2026-07-11

### Fase 2: Skills Reorg + SkillKind Domain Mapping

#### Skills Directory Reorganization
- `~/.config/opencode/skills/` reorganizado en subdirectorios por dominio:
  - `linux/`: C-sysadmin (administración de sistemas Linux)
  - `git/`: D-git-workflow (buenas prácticas de Git)
  - `postgresql/`: C-database-design-sql (bases de datos relacionales)
  - `security/`: A-secure-coding, B-authentication-security (programación segura + autenticación)
  - `generic/`: resto de skills (A-*, B-*, C-*, D-*)
- `SkillsLoader::load_from()` escanea recursivamente todos los subdirectorios

#### SkillKind Domain Mapping
- `match_prompt()` ahora devuelve `Vec<(&Skill, SkillKind)>` con mapeo automático
- Mapeo por prefijo del nombre del skill: `c-*` → dominios C, `d-*` → dominios D, etc.
- `SkillKind::from_domain()` resuelve: `linux` → Linux, `git` → Git, `postgresql` → PostgreSQL, `security` → Security, resto → Generic
- Tests unitarios para SkillKind + match_prompt con SkillKind

#### Tests
- **309 tests unitarios**, 0 failures
- `cargo build --release` — 0 errors

## [0.1.3] - 2026-07-11

### Fase 3: Core Services Architecture (Días 7-10)

#### Core Services Architecture
- **CoreServices struct**: Contenedor para inyección de dependencias de 4 servicios principales
- **ToolsService trait**: `get_tool`, `list_tools`, `register_tool`, `execute_tool`, `all_schemas`
- **ProvidersService trait**: `get_provider`, `list_providers`, `register_provider`, `default_provider`
- **CacheService trait**: `get`, `set`, `delete`, `clear` con soporte TTL
- **SecurityService trait**: `confirm_destructive`, `is_destructive`, `sanitize_command`

#### Default Implementations
- **DefaultToolsService**: Wrapper sobre `ToolRegistry` con `tokio::sync::Mutex`
- **DefaultProvidersService**: Gestión de providers (OllamaProvider por defecto) con `Arc<dyn Provider>`
- **DefaultCacheService**: HashMap con TTL y limpieza automática de entradas expiradas
- **DefaultSecurityService**: Detección de comandos destructivos y confirmación interactiva

#### Integración en Agent Core
- `Agent::new()` ahora es `async` y construye `CoreServices` con implementaciones por defecto
- `OllamaClient` usa `Arc<dyn Provider>` para soporte de múltiples providers
- `ToolRegistry` mantenido para compatibilidad con MCP/Plugins
- `Agent::new()` inyecta CoreServices y delega a servicios

#### Tests
- Tests de core actualizados a `#[tokio::test]` async
- Tests de llm.rs actualizados a usar `Arc<dyn Provider>`
- **309 tests passing**, 0 failures
- `cargo build --release` — 0 errors

## [0.1.1] - 2026-07-17

### Release v0.1.1 — Checkpoint + E2E + 0 Warnings

**340 tests**, 0 failures, 0 warnings en `cargo build --release`.

### Nuevo
- **Checkpoint System**: Context monitoring >80% → auto-pause, project re-read, save session checkpoint, inject resume prompt, continue loop. `--checkpoint` flag for manual checkpoint.
- **E2E Tests**: 11 new integration tests covering agent construction, direct execution, session lifecycle, skills loading/matching, tool registry, checkpoint save/resume.
- **Token estimation**: `estimate_tokens()`, `estimate_messages_tokens()`, `estimate_schemas_tokens()` utilities.
- **Release polish**: All 29 `cargo build --release` warnings eliminated (`#![allow(dead_code)]` + unused import cleanup).

### Cambios
- `src/agent/checkpoint.rs` — CheckpointManager + Checkpoint struct
- `src/agent/core.rs` — `check_context_before_call()` integrated in `run_agent_loop()`
- `src/config.rs` — `checkpoints_dir` path
- `src/cli.rs` — `--checkpoint` flag
- `src/utils/tokens.rs` — Token estimation utilities
- `src/e2e_tests.rs` — 11 integration tests
- `src/main.rs` — `#![allow(dead_code)]` for legitimate public API
- Cleaned unused imports across `provider.rs`, `services/mod.rs`, `session.rs`

### Documentación
- `HYBRID_PLAN.md`: Fase 3 marcada como ✅, Fase 4 como próximo paso
- `README.md`: 340 tests, Fases 1-3 completadas, checkpoint system
- `ROADMAP.md`, `PHASE5_PLAN.md`, `IMPL_CONTEXT.md`, `CONTEXTO_CONTINUACION.md`: Actualizados
