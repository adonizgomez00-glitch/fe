# Contexto de Implementación

> Este archivo preserva el contexto completo de la planificación y diseño de Fe.
> Léelo al iniciar una nueva sesión en `/home/fe/` para continuar el desarrollo sin perder información.

---

## Resumen del Proyecto

**Fe** es un agente CLI local en Rust que conecta modelos LLM (Ollama) con herramientas del sistema. Diseñado para administración de sistemas, desarrollo, arquitectura y documentación.

---

## Estado Actual

**Fases 1-5 completadas + Hybrid Plan Fases 1-3 completadas** (2026-07-17). **329 tests pasando.**

**Hybrid Plan Fases 1-3 completadas**: Provider trait + OllamaProvider, Router→SkillKind, Skills Reorg, Core Services. Checkpoint system implementado.
**Próximo: Fase 4 — Integración + Release (E2E tests, polish, tag v0.1.0).**

---

## Día 11 — Hybrid Plan Fase 1 + Provider Trait + Router SkillKind (2026-07-11)

### 1. `src/agent/provider.rs` (NUEVO)
- `trait Provider: Send + Sync` con métodos: `name()`, `supports_tools()`, `supports_reasoning()`, `max_context()`, `chat()`, `list_models()`
- `OllamaProvider` implementa Provider con HTTP a Ollama API (streaming, retry, keep_alive)
- Tipos compartidos: `Message`, `ToolCall`, `ToolFunction`, `ChatResult`
- 10 tests unitarios

### 2. `src/agent/llm.rs` (MODIFICADO)
- `OllamaClient` ahora envuelve `Box<dyn Provider>` en vez de HTTP directo
- `OllamaClient::new(provider)` inyecta Provider
- Types re-exportados desde provider.rs
- Tests actualizados para usar OllamaProvider via wiremock

### 3. `src/agent/skills.rs` (MODIFICADO)
- `SkillKind` enum: Linux, Docker, Python, Cobol, PowerBI, Kubernetes, PostgreSQL, Security, Git, Generic
- `SkillKind::from_domain()` mapea string a variante
- `match_prompt()` ahora devuelve `Vec<(&Skill, SkillKind)>`
- 5 nuevos tests para SkillKind

### 4. `src/agent/router.rs` (MODIFICADO)
- `select()` retorna `(String, SkillKind)` en vez de solo `String`
- Tests actualizados para destructuring

### 5. Resultados
- 309 tests pasando (26 nuevos)
- `cargo build --release` 0 errors
- Warnings: pre-existentes (dead_code en APIs públicas)

---

## Componentes Implementados (Completo)

### Core
- `src/main.rs` — entry point con routing (mcp, plugin, daemon, prompt)
- `src/cli.rs` — CLI completo con clap (flags + subcomandos)
- `src/config.rs` — loader de config con defaults + env vars + file merge

### Agent
- `src/agent/core.rs` — Loop LLM ↔ Tools con init_mcp/init_plugins lazy, summarizer integrado, recovery de bucles
- `src/agent/provider.rs` — Provider trait + OllamaProvider (NUEVO)
- `src/agent/llm.rs` — OllamaClient wrapper sobre Box<dyn Provider>
- `src/agent/router.rs` — ModelRouter con select() → (String, SkillKind)
- `src/agent/skills.rs` — SkillsLoader + SkillKind enum + match_prompt
- `src/agent/session.rs` — SessionManager con summary persistence
- `src/agent/summarizer.rs` — Resumen automático cada N turns con gemma3:4b
- `src/agent/direct_executor.rs` — Ejecución directa sin LLM
- `src/agent/templates.rs` — 13 generadores multi-lenguaje × tipo
- `src/agent/tools/mod.rs` — Tool trait + ToolRegistry + McpToolWrapper + PluginToolWrapper
- `src/agent/tools/bash.rs`, read.rs, write.rs, edit.rs, glob.rs, grep.rs, task.rs, webfetch.rs, websearch.rs, mkdir.rs

### MCP
- `src/mcp/types.rs` — Tipos MCP + JSON-RPC
- `src/mcp/client.rs` — Cliente MCP stdio
- `src/mcp/registry.rs` — Multi-server registry

### Plugins
- `src/plugins/manifest.rs` — PluginManifest
- `src/plugins/loader.rs` — Filesystem scanner
- `src/plugins/registry.rs` — PluginRegistry HashMap
- `src/plugins/runtime.rs` — PluginRuntime subprocess JSON-RPC

### Daemon
- `src/daemon/server.rs` — Unix socket server
- `src/daemon/protocol.rs` — JSON-RPC 2.0 protocol
- `src/daemon/handler.rs` — Request routing
- `src/daemon/client.rs` — CLI client

### Utils
- `src/utils/confirm.rs` — Confirmación interactiva Y/n/a/b
- `src/utils/spinner.rs` — Indicador de carga

---

## Tests

- **309 tests unitarios** (config, router, llm, provider, cli, tools, confirm, spinner, direct_executor, skills, session, mkdir, task, summarizer, create_app, templates, mcp, plugins, daemon)
- Todos pasando, compilación release sin errores

---

## Modelos Ollama Disponibles

| Modelo | Params | Cuantización | Contexto | Tools | Uso |
|--------|--------|-------------|----------|-------|-----|
| qwen2.5:3b | 3.0B | Q4_K_M | 32K | ✓ | Tareas complejas (default), razonamiento |
| gemma3:4b | 4.3B | Q4_K_M | 32K | ✗ | Tareas simples, resúmenes |
| deepseek-r1:8b | 8.2B | Q4_K_M | 128K | thinking | ~~Razonamiento profundo~~ (reemplazado por qwen2.5:3b por velocidad) |

---

## Decisiones Arquitectónicas (ADRs)

### ADR-001: Rust como lenguaje
- **Contexto**: Binario eficiente, sin runtime pesado, portable.
- **Decisión**: Rust (Tokio async, clap CLI, reqwest HTTP, serde JSON).

### ADR-002: Confirmación híbrida
- **Contexto**: Seguridad vs velocidad.
- **Decisión**: Tool-by-tool por defecto, `--batch` para tareas rutinarias.

### ADR-003: Summarized history con skills
- **Contexto**: Context window limitada (32K).
- **Decisión**: Resumen automático cada 4 turns con gemma3:4b + skills_tags.

### ADR-004: Model router automático
- **Contexto**: 3 modelos con capacidades distintas.
- **Decisión**: Heurística por keywords + skills + flags + SkillKind.

### ADR-005: Skills de opencode
- **Contexto**: Ecosistema de skills en `~/.config/opencode/skills/`.
- **Decisión**: Reutilizar formato y directorio.

### ADR-006: Provider trait (Hybrid Fase 1)
- **Contexto**: OllamaClient con HTTP directo acoplaba Agent a Ollama.
- **Decisión**: Provider trait con implementación OllamaProvider. Agent solo conoce el trait.

### ADR-007: SkillKind enum
- **Contexto**: Router necesitaba devolver dominio para routing.
- **Decisión**: SkillKind enum en skills.rs, router.select() devuelve (String, SkillKind).

### ADR-008: Tool schemas solo para modelos compatibles
- **Contexto**: gemma3:4b no soporta function calling.
- **Decisión**: Solo modelo complex recibe tool schemas.

### ADR-009: Direct Execution
- **Contexto**: LLM tarda 10-30s para comandos triviales.
- **Decisión**: Patrones regex detectan comandos comunes, ejecutan bash directo.

---

## Prompt para Continuar el Desarrollo

Copia y pega esto en tu próxima sesión de opencode:

```markdown
Estás en el proyecto Fe en /home/fe/.
Lee IMPL_CONTEXT.md, TODO.md, SESSION.md y CHANGELOG.md para contexto completo.
El proyecto es un agente CLI en Rust que usa Ollama para conectar LLM locales con herramientas del sistema.
Skills de opencode disponibles en ~/.config/opencode/skills/.
Modelo actual: qwen2.5:3b (tools soportadas).
309 tests pasando.

Fases 1-5 completadas ✅
Hybrid Plan Fase 1 completada ✅:
- src/agent/provider.rs: trait Provider + OllamaProvider
- src/agent/llm.rs: delegar a Provider
- src/agent/skills.rs: SkillKind enum + match_prompt devuelve kinds
- src/agent/router.rs: select() devuelve (String, SkillKind)
- src/agent/services/: Core Services traits implementados
- src/agent/checkpoint.rs: CheckpointManager (context >80% → auto-save + resume)
- 329 tests pasando

Próximo: **Fase 4 — Integración + Release** (ver HYBRID_PLAN.md):
- E2E tests: validar todos los flujos core
- Polish: `cargo build --release` con 0 warnings
- Release: tag v0.1.0 + install.sh + checksums
```

---

## Estructura del Proyecto (Árbol Completo)

```
/home/fe/
├── Cargo.toml
├── config.toml.example
├── install.sh
├── README.md
├── ARCHITECTURE.md
├── ROADMAP.md
├── SPECIFICATION.md
├── IMPL_CONTEXT.md
├── TODO.md
├── CHANGELOG.md
├── HYBRID_PLAN.md
├── SESSION.md
├── DAYS8_PLAN.md
├── DAYS9_PLAN.md
├── PHASE5_PLAN.md
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── agent/
│   │   ├── mod.rs
│   │   ├── core.rs
│   │   ├── llm.rs
│   │   ├── provider.rs        # Provider trait + OllamaProvider
│   │   ├── router.rs
│   │   ├── skills.rs           # SkillKind enum + match_prompt
│   │   ├── session.rs
│   │   ├── summarizer.rs
│   │   ├── direct_executor.rs
│   │   ├── templates.rs
│   │   └── tools/
│   │       ├── mod.rs
│   │       ├── bash.rs / read.rs / write.rs / edit.rs
│   │       ├── glob.rs / grep.rs / task.rs
│   │       ├── webfetch.rs / websearch.rs / mkdir.rs
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── types.rs / client.rs / registry.rs
│   ├── plugins/
│   │   ├── mod.rs
│   │   ├── manifest.rs / loader.rs / registry.rs / runtime.rs
│   ├── daemon/
│   │   ├── mod.rs
│   │   ├── server.rs / protocol.rs / handler.rs / client.rs
│   └── utils/
│       ├── confirm.rs / spinner.rs
├── docs/
│   ├── user-guide.md
│   └── configuration.md
└── test_mcp_server.py
```
