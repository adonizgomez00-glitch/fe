# Día 7 — MCP Resources + Prompts + CLI Completo

> Implementado para DeepSeek V4 Flash Free — Contexto completo Fe + Fase 5 Día 7

---

## Resumen Ejecutivo

**Fecha:** 2026-07-11  
**Estado:** ✅ Completado  
**Tests:** 252 pasando (11 nuevos vs Día 6)  
**Release build:** ✅  
**Integración E2E:** ✅ Verificada con `test_mcp_server.py`

---

## Entregables Día 7

| Entregable | Estado | Archivo |
|------------|--------|---------|
| `McpPrompt` type + deserialización | ✅ | `src/mcp/types.rs` |
| `list_prompts()` / `get_prompt()` | ✅ | `src/mcp/client.rs` |
| `discover_all_resources()` / `read_resource()` | ✅ | `src/mcp/registry.rs` |
| `discover_all_prompts()` / `get_prompt()` | ✅ | `src/mcp/registry.rs` |
| CLI: `fe mcp resources` | ✅ | `src/cli.rs` + `src/main.rs` |
| CLI: `fe mcp prompts` | ✅ | `src/cli.rs` + `src/main.rs` |
| CLI: `fe mcp read <server> <uri>` | ✅ | `src/cli.rs` + `src/main.rs` |
| CLI: `fe mcp get-prompt <server> <name> [args]` | ✅ | `src/cli.rs` + `src/main.rs` |
| `fe mcp discover` muestra tools+resources+prompts | ✅ | `src/main.rs` |
| Servidor test con resources + prompts | ✅ | `test_mcp_server.py` |
| Tests unitarios prompts | ✅ | `src/mcp/types.rs` |

---

## Archivos Modificados

### `src/mcp/types.rs`
- Añadidos: `McpPrompt`, `McpPromptArgument`, `McpPromptMessage`, `McpPromptContent`
- 3 tests nuevos

### `src/mcp/client.rs`
- Campo `prompts: Vec<McpPrompt>` en `McpClient`
- Métodos: `list_prompts()`, `get_prompt(name, args)`

### `src/mcp/registry.rs`
- Métodos resources: `discover_all_resources()`, `read_resource(server, uri)`
- Métodos prompts: `discover_all_prompts()`, `get_prompt(server, name, args)`

### `src/cli.rs`
- `McpAction::Resources`, `Prompts`, `Read`, `GetPrompt`
- Structs: `McpReadArgs`, `McpGetPromptArgs`
- 9 tests CLI nuevos

### `src/main.rs`
- `handle_mcp()` expandido con 4 nuevos brazos
- Output formateado para resources/prompts

### `test_mcp_server.py`
- `resources/list`: 2 resources (greeting://hello, info://version)
- `resources/read`: contenido text/plain + application/json
- `prompts/list`: 2 prompts (say_hello con args, show_info sin args)
- `prompts/get`: messages con role/content estructurado

---

## Validación Manual (E2E)

```bash
# Config
mkdir -p ~/.config/fe
cat > ~/.config/fe/config.toml << 'EOF'
[[mcp.servers]]
name = "test-server"
command = "python3"
args = ["/home/fe/test_mcp_server.py"]
enabled = true
EOF

# Tests
fe mcp discover
# 🔧 Herramientas: 3 (echo, ping, greet)
# 📄 Recursos: 2 (Hello Resource, Version Info)
# 💬 Prompts: 2 (say_hello, show_info)

fe mcp resources
# 📄 Recursos MCP:
#   test-server: Hello Resource (greeting://hello) - text/plain
#   test-server: Version Info (info://version) - application/json

fe mcp prompts
# 💬 Prompts MCP:
#   test-server: say_hello - Generate a greeting message - Argumentos: name, language
#   test-server: show_info - Show server information - Argumentos: 

fe mcp read test-server greeting://hello
# 📖 Leyendo recurso 'greeting://hello' desde 'test-server'...
# { "mimeType": "text/plain", "text": "Hello from MCP test server!", "uri": "greeting://hello" }

fe mcp get-prompt test-server say_hello '{"name":"Mundo","language":"es"}'
# 💬 Obteniendo prompt 'say_hello' desde 'test-server'...
# [assistant] ¡Hola, Mundo! Bienvenido al servidor MCP de prueba.

fe mcp call mcp:test-server:echo '{"message":"hola"}'
# 🔧 Llamando test-server:echo...
# { "content": [ { "text": "Echo: hola", "type": "text" } ] }
```

---

## Comandos MCP Finales (Día 6 + 7)

```bash
fe mcp discover                    # Todo: tools + resources + prompts
fe mcp servers                     # Servidores configurados
fe mcp call mcp:srv:tool '{}'      # Ejecuta tool
fe mcp resources                   # Lista resources
fe mcp prompts                     # Lista prompts
fe mcp read servidor uri           # Lee resource
fe mcp get-prompt servidor name [args]  # Obtiene prompt
```

---

## Próximos Días (Referencia)

| Día | Entregable |
|-----|------------|
| **8** | Integración completa en Agent core (tools MCP disponibles para LLM) |
| **9** | Plugin system (manifest, loader, registry) |
| **10** | Plugin runtime (scripts Python/Bash, hot-reload) |
| **11** | Tests E2E, integración completa |
| **12** | Polish, docs, release v0.1.0 final |

---

## Día 8 — Plan Preliminar

**Objetivo:** MCP tools disponibles para el LLM en el agent loop

### Tareas:
1. **Agent::new()** — Inicializar `McpRegistry` desde config
2. **ToolRegistry.register_mcp_tools()** — Ya existe, invocarlo
3. **McpRegistry** — Mantener vivo durante vida del Agent (Arc<Mutex<>>)
4. **Limpieza** — Shutdown graceful al dropear Agent
5. **Test E2E** — `fe "usa el servidor MCP filesystem para listar archivos"`

### Archivos a Modificar:
- `src/agent/core.rs` — Agent::new(), run(), Drop
- `src/agent/tools/mod.rs` — register_mcp_tools() ya implementado

### Validación:
```bash
fe "usa el servidor test-server para hacer echo de 'hola mundo'"
# El LLM debería ver tool mcp:test-server:echo y usarlo
```

---

*Generado para DeepSeek V4 Flash Free — Contexto completo Fe + Phase 5 Día 7*