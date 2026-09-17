# TODO - Fe

> Estado actual: **311 tests — Fase 2 (Skills Reorg) completada con domain mapping correcto**

---

## ✅ Fase 1: Provider + Router (Días 1-3) — COMPLETADA

| Tarea | Archivo | Estado |
|-------|---------|--------|
| Provider trait + OllamaProvider | `src/agent/provider.rs` | ✅ |
| llm.rs delegar a Provider | `src/agent/llm.rs` | ✅ |
| router.rs select() devuelve (String, SkillKind) | `src/agent/router.rs` | ✅ |
| SkillKind enum en skills.rs | `src/agent/skills.rs` | ✅ |
| Tests provider + router + skills | — | ✅ 26 nuevos |

---

## ✅ Fase 2: Skills Reorg (Días 4-6) — COMPLETADA

| Tarea | Archivo | Estado |
|-------|---------|--------|
| Crear skills/{linux,docker,python,cobol,powerbi,kubernetes,postgresql,security,git}/ | `~/.config/opencode/skills/` | ✅ |
| Mover SKILL.md existentes a carpetas por dominio | skills/*/ | ✅ |
| `Skill` struct con campo `domain` | `src/agent/skills.rs:57` | ✅ |
| `load_recursive()` escanea con tracking de dominio | `src/agent/skills.rs:85-114` | ✅ |
| `match_prompt()` usa `skill.domain` en vez de nombre | `src/agent/skills.rs:165` | ✅ |
| Tests: `test_skill_kind_matches_domain_directory` | `src/agent/skills.rs` | ✅ 2 nuevos |
| Tests: `test_match_prompt_returns_skill_kind_from_domain` | `src/agent/skills.rs` | ✅ |

---

## ✅ Fase 3: Core Services (Días 7-10) — COMPLETADA

| Tarea | Archivo | Estado |
|-------|---------|--------|
| services/tools.rs — trait ToolsService | `src/agent/services/` | ✅ |
| services/providers.rs — trait ProvidersService (MCP+Plugin) | `src/agent/services/` | ✅ |
| services/cache.rs — trait CacheService (HashMap+TTL) | `src/agent/services/` | ✅ |
| services/security.rs — trait SecurityService (confirm) | `src/agent/services/` | ✅ |
| core.rs inyecta CoreServices | `src/agent/core.rs` | ✅ |
| main.rs construye CoreServices reales | `src/main.rs` | ✅ |
| Tests core con servicios mock | `src/agent/core.rs` | ✅ |

---

## 🔜 Siguiente: Pulido + Release v0.1.1

1. Validar E2E: `fe "lista archivos"`, `fe mcp discover`, `fe daemon status`
2. `cargo build --release` — 0 warnings
3. Ejecutar `./install.sh`
4. Tag release v0.1.1

---

## Backlog (Post v0.1.1)

- [ ] **System prompt adaptativo**: Detectar modelo y ajustar reglas
- [ ] **Parallel tool execution**: Ejecutar tools independientes en paralelo
- [ ] **Tool result caching**: Cachear resultados de tools idempotentes
- [ ] **Prompt compression**: Comprimir system prompt dinámicamente
- [ ] **Structured output enforcement**: JSON mode para modelos sin function calling
- [ ] **Adaptive temperature**: Temperatura baja para code/tools, alta para chat
- [ ] **Cloud providers**: OpenAI, Anthropic como fallback cuando no hay modelo local con tools
- [ ] **Checkpoint protocol**: Implementar lógica A-context-manager en core.rs
