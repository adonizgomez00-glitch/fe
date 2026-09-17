# ROADMAP - Fe v0.1.0

## Fase 1: Foundation ✅

### Objetivo
CLI funcional, config, cliente Ollama con streaming, router de modelos.

### Tareas
- [x] `cargo init` + Cargo.toml con dependencias
- [x] CLI skeleton (`clap`): `fe <prompt>`, `fe --batch`, `fe session`, `fe config`, `fe completions`
- [x] Config loading: `~/.config/fe/config.toml` + defaults + env vars
- [x] Cliente Ollama async (`reqwest` + `tokio-stream`): streaming, timeout, retry
- [x] Model router básico (flags + keyword heuristic)
- [x] 33 tests unitarios (config, router, llm con wiremock, cli)

### Entregable
```bash
fe "hola mundo"  # responde con LLM
fe doctor         # diagnóstico del sistema
fe config show    # muestra configuración
```

---

## Fase 2: Tools Engine ✅

### Objetivo
9 herramientas funcionales con confirmación híbrida.

### Tareas
- [x] Tool trait + registry + schema JSON
- [x] `bash.rs`: ejecuta comandos, detecta sudo → confirm extra
- [x] `read.rs`: lee archivos (con offset/limit)
- [x] `write.rs`: escribe archivos
- [x] `edit.rs`: edición precisa con oldString/newString
- [x] `glob.rs`: búsqueda de archivos por patrón
- [x] `grep.rs`: búsqueda de contenido con regex
- [x] `task.rs`: lanza subagentes para tareas complejas
- [x] `webfetch.rs`: fetch de URLs (markdown/text/html)
- [x] `websearch.rs`: búsqueda web (esqueleto)
- [x] Confirmación interactiva: `Y/n/a/b`
- [x] `--batch` mode: plan → confirm once → execute all
- [x] `--dry-run`: muestra plan sin ejecutar
- [x] Tool schemas solo para modelos compatibles (fix gemma3/deepseek)
- [x] 59 tests unitarios

### Entregable
```bash
fe "crea un script de backup en rust"  # multi-tool interactivo
fe --batch "apt update && apt upgrade"  # una confirmación
fe --dry-run "lista archivos"           # plan sin ejecutar
fe --fast "hola"                        # gemma3 sin tools
```

---

## Fase 3: Skills + Sessions (Semana 3)

### Objetivo
Integración con skills de opencode, sesiones persistentes.

### Tareas
- [ ] Parser de skills: `SKILL.md` → frontmatter + contenido
- [ ] Keyword matcher (TF-IDF simple con `similar` crate)
- [ ] Inyección de skills en system prompt
- [ ] Session manager: create, append markdown, auto-save
- [ ] `fe session list`, `show`, `export`, `clean`
- [ ] Skill auto-detection desde prompt

### Entregable
```bash
fe "actualiza repositorios"  # auto-detecta C-sysadmin
fe session list              # lista sesiones previas
fe session show abc123       # muestra sesión completa
```

---

## Fase 4: Summarization + Polish ✅

### Objetivo
Historial comprimido, shell completions, release.

### Tareas
- [x] Summarizer: llama gemma3:4b cada N turns
- [x] Skills-enhanced summary (preserva skill_tags en resumen)
- [x] Sliding window + summarized history merge
- [x] Shell completions (bash/zsh/fish)
- [x] `fe doctor`: verifica ollama, config, permisos
- [ ] Tests de integración
- [ ] Release binary + install script

### Entregable
```bash
fe "analiza arquitectura del proyecto"  # summarize automático
fe completions bash                     # genera completions
fe doctor                               # diagnóstico sistema
```

---

## Fase 5: MCP + Plugins + Daemon (En progreso)

| Día | Componente | Estado |
|-----|------------|--------|
| 1-2 | Release v0.1.0 (tests, docs, empaquetado) | ✅ |
| 3-5 | Daemon Mode (protocol, server, handler, client) | ✅ |
| 6 | MCP Client Base (types, spawn, initialize) | ✅ |
| 7 | MCP Resources + Prompts + CLI completo | ✅ |
| 8 | MCP Integration in Agent Core | ✅ |
| **9** | **Plugin System Base (manifest, loader, registry)** | **✅** |
| **10** | **Plugin Runtime + CLI + Agent Integration** | **✅** |
| **F1** | **Hybrid Fase 1**: Provider trait + OllamaProvider + Router→SkillKind | ✅ |
| **11** | **Hybrid Fase 1 complete + 309 tests + Release build** | **✅** |
| — | Hybrid Fase 2 (Skills Reorg) + Fase 3 (Core Services) | ✅ |
| 12 | Polish + Release v0.1.0 final | ⬜ |

### Features Implementadas en Fase 5
- [x] **MCP Client**: Conexión con servidores MCP via stdio JSON-RPC
- [x] **MCP Tools**: Tools MCP disponibles para el LLM como `mcp:{server}:{tool}`
- [x] **MCP Resources**: Lectura de recursos desde servidores MCP
- [x] **MCP Prompts**: Obtención de prompts desde servidores MCP
- [x] **Daemon Mode**: Servidor persistente en background con socket Unix
- [x] **Plugin System Base**: Carga y gestión de plugins via plugin.toml
- [x] **Plugin Runtime**: Ejecución de scripts plugin via subprocess JSON-RPC
- [x] **Plugin CLI**: `fe plugin run <name> <cmd>`, `fe plugins reload`
- [x] **Plugin Agent Integration**: PluginToolWrapper + ToolRegistry + lazy init en Agent

### Pendientes Post v0.1.0
- [ ] **Multimodal**: visión + audio
- [ ] **Parallel tools**: ejecutar herramientas independientes en paralelo
