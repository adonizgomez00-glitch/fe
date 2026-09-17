# AGENT — Documento del agente `fe`

> Documento vivo SAQI (`D-Agente-IA` / `A-context-manager`) · v1.0.0 · Comportamiento, límites y operación del agente `fe`.

## 1. Rol

**Fe** (símbolo químico del hierro) es un agente CLI local que conecta la terminal con modelos LLM vía **Ollama**. Opera bajo el marco SAQI: es un agente **agentic** con skills gobernadas, iteraciones con calidad y gates humanos.

Objetivo: administración de sistemas, desarrollo de software, arquitectura y generación de documentos — todo desde la terminal, sin servicios externos.

---

## 2. Comportamiento en dos modos (regla clave)

**`fe` distingue entre Acción y Razonamiento**, y cada una usa un camino distinto:

| Caso | Qué hacer | ¿Usa IA? |
|------|-----------|----------|
| **Acción** | Ejecutar un comando / realizar una acción pedida por el usuario | **No.** Se ejecuta por **programación** (reglas + fuzzy), con confirmación |
| **Razonamiento** | Pregunta, análisis, explicación, consulta, comparación | **Sí.** Se usa el modelo local (razonamiento) |

### 2.1 Acciones (sin IA siempre)
- El prompt se interpreta por **reglas declarativas** de lenguaje natural → comando real.
- Si no hay regla exacta, se usa **fuzzy matching** sobre intenciones conocidas.
- La IA **nunca** se usa para decidir ni ejecutar acciones.
- Si una acción no se puede resolver por reglas → se **avisa al usuario** (no se inventa el comando) y se ofrece alternativa; no se llama al LLM.

### 2.2 Confirmación (gate humano)
- Acciones **destructivas** (borrar, eliminar, sobrescribir, sudo, etc.): **siempre** piden confirmación explícita `[y/N]`.
- Acciones **seguras** (ls, pwd, listar, leer, mkdir, etc.): se ejecutan directamente, sin confirmación.
- Puede forzarse la confirmación de todas con `--batch` y omitirla para destructivas con `--destructive`.

### 2.3 Razonamiento (usar IA)
- Preguntas y análisis que requieren entender contexto, comparar, explicar o razonar.
- Usa el modelo local configurado (por defecto **`qwen2.5-coder:3b`** en todos los slots).

---

## 3. Modelos (por defecto)

| Slot | Modelo | Uso |
|------|--------|-----|
| `complex` | `qwen2.5-coder:3b` | Razonamiento con tools |
| `simple` | `qwen2.5-coder:3b` | Consultas rápidas |
| `reasoning` | `qwen2.5-coder:3b` | Flag `--reasoning` |
| `summarizer` | `qwen2.5-coder:3b` | Resúmenes de sesión |

- Configurables en `~/.config/fe/config.toml` (sección `[models]`) o vars de entorno `FE_MODELS_*`.

---

## 4. Componentes del agente (SAQI-006)

| Componente | En `fe` |
|-----------|---------|
| **Planner** | `src/agent/router.rs` (selección de modelo), `direct_executor.rs`/matcher (acciones) |
| **Context Manager** | `src/agent/checkpoint.rs`, `src/agent/summarizer.rs`, `src/agent/session.rs` |
| **Skill Registry** | `src/agent/skills.rs` (carga desde `~/.config/opencode/skills/`) |
| **Tool Executor** | `src/agent/tools/*.rs` (10 herramientas) + `src/mcp/*` + `src/plugins/*` |
| **Mode Controller** | Loop `src/agent/core.rs` (acciones vs razonamiento) |

```mermaid
graph TD
    USER[Usuario CLI] --> FE[fe]
    FE --> INTENT[Clasificador de intención]
    INTENT -->|Acción| ACT[Reglas + Fuzzy + Confirmación]
    INTENT -->|Razonamiento| IA[Modelo local qwen2.5-coder:3b]
    ACT -->|comando| SHELL[Shell]
    IA --> TOOLS[Tools / Skills / MCP / Plugins]
---

## 5. Herramientas disponibles

**Nativas (10):** bash, read, write, edit, glob, grep, task, webfetch, websearch, mkdir.
**MCP:** via `mcp:{server}:{tool}` (si hay servidores configurados).
**Plugins:** via `plugin:{name}:{tool}` (si hay plugins instalados).

Reglas de uso:
- Cada tool valida parámetros, aplica timeout y trunca salida.
- Los comandos con `sudo`, `rm`, `dd`, `shutdown`, etc. se marcan **destructivos** y exigen confirmación.
- El output se controla (`max_output_lines`, `max_output_bytes`) para no saturar el contexto.

---

## 6. Skills

- Se cargan desde `~/.config/opencode/skills/` (organizadas por dominio: `linux/`, `git/`, `postgresql/`, `security/`, `generic/`, etc.).
- `skills.rs` parsea frontmatter (`SKILL.md`) y matchea el prompt contra skills con keyword/domain scoring → `SkillKind`.
- Las skills activas se inyectan en el system prompt para guiar el razonamiento.
- Para acciones, la detección por lenguaje natural usa reglas propias (no depende de skills).

---

## 7. Ciclo de vida de una petición

```
1. Input:   fe "actualiza repositorios"  o  fe "¿qué es SOLID?"
2. Intent:  clasificador → Acción | Razonamiento
3a. Acción:
      - match de regla → comando real → (si destructivo: confirmar) → ejecutar → mostrar
      - sin regla → fuzzy → si acierta → confirmar si destructivo → ejecutar
      - sin fuzzy → avisar, NO usar IA
3b. Razonamiento:
      - route: skills + contexto → modelo qwen2.5-coder:3b
      - loop ReAct (tools) o chat simple → responder
4. Sesión: se guarda en ~/.local/share/fe/sessions/<id>.md
5. Summarize: cada N turns (default 4) → resumen de contexto
```

---

## 8. Seguridad y límites

- **Las acciones destructivas siempre requieren confirmación explícita.**
- La IA solo se usa para razonamiento; **nunca** decide acciones (evita alucinaciones ejecutando comandos).
- Si una acción no tiene regla, el agente **no inventa** el comando: informa y pregunta.
- El flag `--dry-run` muestra el plan sin ejecutar nada.
- Modelo local 3B: output validado, few-shot y checkpointing agresivo (según `docs/SAQI.md` §6).

---

## 9. Comandos de gestión

```bash
fe doctor               # diagnóstico: Ollama, config, skills
fe session list/show/export/clean
fe config show / edit
fe mcp discover/servers/call ...
fe plugins list / plugin run
fe daemon start/status/stop
fe completions bash|zsh|fish
```

---

## 10. Referencias

- [`docs/SAQI.md`](docs/SAQI.md) — marco de metodología (Skills Agentic Quality Iteration).
- [`docs/SDD.md`](docs/SDD.md) — desarrollo dirigido por especificación.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — arquitectura técnica y flujos.

---

## 11. Changelog

### v1.0.0 — 2026-09-08
- Definición del comportamiento en dos modos: **Acción (sin IA, con confirmación)** vs **Razonamiento (IA)**.
- Modelo por defecto `qwen2.5-coder:3b` en todos los slots.
- Documento vivo SAQI enlazado a `docs/SAQI.md` y `docs/SDD.md`.
```