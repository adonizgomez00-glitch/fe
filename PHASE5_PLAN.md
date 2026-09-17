# Fase 5: MCP + Plugins + Daemon + Release v0.1.0

> Plan documentado para DeepSeek V4 Flash Free — Contexto completo + Skills aplicables

---

## Contexto del Proyecto Fe

**Fe** es un agente CLI en Rust que conecta modelos LLM locales (Ollama) con herramientas del sistema. Estado actual: **166 tests pasando**, Fases 1-4 completadas.

**Stack**: Rust 2021, Tokio, clap, reqwest, serde, toml, tracing, walkdir, globset, regex, async-trait, uuid, chrono, similar.

**Modelos Ollama**:
- `qwen2.5:3b` — tareas complejas + tools (default)
- `gemma3:4b` — tareas simples, resúmenes (sin tools)
- `deepseek-r1:8b` — backup reasoning (thinking)

**Skills opencode**: 18 skills en `~/.config/opencode/skills/` (categorías A, B, C, D)

---

## Decisiones Técnicas Confirmadas

| Decisión | Opción Elegida |
|----------|----------------|
| MCP Transport | **stdio** (subprocess) |
| Daemon Protocol | **JSON-RPC 2.0** sobre Unix socket |
| Plugin Runtime | **Scripts Python/Bash** ahora, WASM futuro |
| Configuración | Todo en `config.toml` |

---

## Arquitectura General Fase 5

```
┌─────────────────────────────────────────────────────────────┐
│                         fe CLI                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│  │  Direct     │ │   Daemon    │ │   MCP       │            │
│  │  Execution  │ │  Client     │ │  Client     │            │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘            │
└─────────┼───────────────┼───────────────┼────────────────────┘
          │               │               │
          ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────┐
│                      Agent Core                              │
│  ┌──────────────┐ ┌──────────────┐ ┌────────────────────┐   │
│  │ ToolRegistry │ │  Skills      │ │  Session Manager   │   │
│  │ (native +    │ │  Loader      │ │                    │   │
│  │  mcp +       │ │              │ │                    │   │
│  │  plugin)     │ │              │ │                    │   │
│  └──────────────┘ └──────────────┘ └────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
          │               │               │
          ▼               ▼               ▼
    ┌──────────┐    ┌──────────┐    ┌──────────┐
    │ Native   │    │ MCP      │    │ Plugins  │
    │ Tools    │    │ Servers  │    │ (scripts)│
    │ (bash,   │    │ (stdio)  │    │          │
    │  read,   │    │          │    │          │
    │  write,  │    │          │    │          │
    │  etc.)   │    │          │    │          │
    └──────────┘    └──────────┘    └──────────┘
```

---

## Componente 1: Release v0.1.0 (Días 1-2)

### Tareas
1. **Tests integración summarizer** — wiremock Ollama en `src/agent/summarizer.rs`
2. **Script instalador** — `install.sh` que hace `cargo build --release` + copia a `~/.cargo/bin/` o `/usr/local/bin/`
3. **Empaquetado** — `tar.gz` con binario + `config.toml.example` + `README.md` + checksums SHA256
4. **Documentación usuario** — `README.md` actualizado, `docs/user-guide.md`, `docs/configuration.md`
5. **CHANGELOG v0.1.0** — Entrada final

### Skills Aplicables
- **A-testing** — Tests integración con mocks, coverage
- **C-documentation** — Docs automáticas, user guide
- **D-git-workflow** — Tag v0.1.0, release notes

---

## Componente 2: Daemon Mode MVP (Días 3-5)

### Arquitectura

```
┌──────────────────────────────────────────────────────────────┐
│                      fe daemon start                          │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Daemon Server (tokio::net::UnixListener)               │  │
│  │  - Socket: ~/.local/share/fe/daemon.sock               │  │
│  │  - Protocolo: JSON-RPC 2.0 (line-delimited)            │  │
│  │  - Request handling: spawn task per connection         │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ Client 1 │ │ Client 2 │ │ Client N │
        │ fe "..." │ │ fe "..." │ │ fe "..." │
        └──────────┘ └──────────┘ └──────────┘
```

### Protocolo JSON-RPC 2.0

**Request (cliente → daemon):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "prompt",
  "params": {
    "text": "actualiza repositorios",
    "flags": {"batch": false, "reasoning": false, "fast": false, "dry_run": false, "destructive": false}
  }
}
```

**Response Streaming (daemon → cliente):**
```json
{"jsonrpc":"2.0","id":1,"result":{"type":"token","content":"Actualizando"}}
{"jsonrpc":"2.0","id":1,"result":{"type":"tool_call","tool":"bash","params":{"command":"apt update","description":"Actualiza lista de paquetes"}}}
{"jsonrpc":"2.0","id":1,"result":{"type":"tool_result","tool":"bash","output":"Hit:1 http://archive.ubuntu.com..."}}
{"jsonrpc":"2.0","id":1,"result":{"type":"token","content":"Repositorios actualizados."}}
{"jsonrpc":"2.0","id":1,"result":{"type":"done","session_id":"uuid"}}
```

**Métodos soportados:**
| Método | Params | Descripción |
|--------|--------|-------------|
| `prompt` | `{text, flags}` | Ejecuta agente con prompt |
| `session.list` | `{}` | Lista sesiones |
| `session.show` | `{id}` | Muestra sesión |
| `config.get` | `{}` | Config actual |
| `config.set` | `{key, value}` | Actualiza config |
| `mcp.discover` | `{}` | Descubre tools MCP |
| `plugins.list` | `{}` | Lista plugins |
| `health` | `{}` | Health check |

### Estructura de Archivos

```
src/
├── daemon/
│   ├── mod.rs           # Daemon server entry point
│   ├── server.rs        # UnixListener + connection handling
│   ├── protocol.rs      # JSON-RPC 2.0 types + serialization
│   ├── handler.rs       # Request routing → Agent
│   └── client.rs        # Cliente CLI para conectar al daemon
├── cli.rs               # + subcomando `daemon start|stop|status`
└── main.rs              # + routing daemon client
```

### CLI Daemon
```bash
fe daemon start          # Inicia daemon en background (fork + setsid)
fe daemon stop           # Mata proceso daemon (PID file)
fe daemon status         # Health check via socket
fe "prompt"              # Si daemon corriendo → usa daemon; si no → inline
```

### Skills Aplicables
- **A-project-architecture** — Separación daemon/client, protocol layer
- **A-secure-coding** — Validación JSON-RPC, sanitización input, no code execution
- **C-sysadmin** — Process management (daemonize, PID file, signals)
- **D-Agente-IA** — Token streaming eficiente, context management

---

## Componente 3: MCP Client (Días 6-8)

### Arquitectura

```
┌──────────────────────────────────────────────────────────────┐
│                      MCP Registry                             │
│  config.toml → [mcp.servers] → Vec<McpServerConfig>         │
└──────────────────────────────────────────────────────────────┘
                            │
               ┌────────────┼────────────┐
               ▼            ▼            ▼
         ┌──────────┐ ┌──────────┐ ┌──────────┐
         │ Server 1 │ │ Server 2 │ │ Server N │
         │ (stdio)  │ │ (stdio)  │ │ (stdio)  │
         └──────────┘ └──────────┘ └──────────┘
               │            │            │
               ▼            ▼            ▼
         ┌────────────────────────────────────┐
         │      McpClient (por server)        │
         │  - Process spawn (command + args)  │
         │  - stdin/stdout JSON-RPC           │
         │  - initialize + tools/list         │
         │  - tools/call proxy                │
         │  - resources/list + read           │
         │  - prompts/list + get              │
         └────────────────────────────────────┘
                            │
                            ▼
               ┌────────────────────────┐
               │ ToolRegistry (unified) │
               │  - native tools        │
               │  - mcp tools (prefixed)│
               │  - plugin tools        │
               └────────────────────────┘
```

### Configuración (config.toml)
```toml
[mcp]
enabled = true

[[mcp.servers]]
name = "filesystem"
command = "python3"
args = ["/home/user/mcp-servers/filesystem_server.py", "/home/user/projects"]
env = {}
enabled = true

[[mcp.servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "ghp_xxx" }
enabled = true

[[mcp.servers]]
name = "custom"
command = "python3"
args = ["/home/user/mcp-servers/my_server.py"]
enabled = true
```

### Tipos MCP (src/mcp/types.rs)
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<McpPromptArgument>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptMessage {
    pub role: String,
    pub content: McpPromptContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    pub tools: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub prompts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}
```

### MCP Client (src/mcp/client.rs)
```rust
pub struct McpClient {
    config: McpServerConfig,
    process: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
    request_id: AtomicU64,
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
    prompts: Vec<McpPrompt>,
    initialized: bool,
    server_info: Option<ServerInfo>,
}

impl McpClient {
    pub async fn spawn(config: McpServerConfig) -> Result<Self>;
    pub async fn initialize(&mut self) -> Result<InitializeResult>;
    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>>;
    pub async fn call_tool(&mut self, name: &str, args: serde_json::Value) -> Result<serde_json::Value>;
    pub async fn list_resources(&mut self) -> Result<Vec<McpResource>>;
    pub async fn read_resource(&mut self, uri: &str) -> Result<String>;
    pub async fn list_prompts(&mut self) -> Result<Vec<McpPrompt>>;
    pub async fn get_prompt(&mut self, name: &str, args: Option<serde_json::Value>) -> Result<Vec<McpPromptMessage>>;
    pub async fn shutdown(&mut self) -> Result<()>;
}
```

### Integración en ToolRegistry
```rust
// En tool registry, tools MCP se registran con prefijo
"mcp:filesystem:read_file" -> McpToolWrapper { client: McpClient, tool_name: "read_file" }
"mcp:github:create_issue"  -> McpToolWrapper { client: McpClient, tool_name: "create_issue" }
```

### CLI MCP
```bash
fe mcp discover          # Lista todos los tools/resources/prompts de servers MCP
fe mcp servers           # Lista servers configurados + status
fe mcp call <tool> <json_args>  # Llama tool MCP directamente
fe mcp resources         # Lista solo recursos
fe mcp prompts           # Lista solo prompts
fe mcp read <server> <uri>      # Lee un recurso
fe mcp get-prompt <server> <name> [args]  # Obtiene un prompt
```

### Skills Aplicables
- **A-secure-coding** — Subprocess sandboxing, env var validation, input sanitization
- **B-javascript-clean** — JSON-RPC parsing, async stream handling
- **C-sysadmin** — Process management, stdio handling, signal propagation
- **D-Agente-IA** — Tool schema normalization, error recovery

---

## Día 8 — Integración MCP en Agent Core

### Objetivo
Hacer que los tools MCP estén disponibles para el LLM durante `run_agent_loop()` para que el agente pueda usarlos naturalmente como herramientas nativas.

### Archivos a Modificar

1. **`src/agent/core.rs`** — `Agent::new()`
   - Inicializar `McpRegistry` desde config si `mcp.enabled`
   - Registrar tools MCP en `ToolRegistry` vía `register_mcp_tools()`
   - Manejar lifecycle (shutdown en Drop del Agent)

2. **`src/agent/tools/mod.rs`** — Verificar/Completar
   - `register_mcp_tools()` ya existe y funciona
   - `McpToolWrapper.schema()` usa `input_schema` del MCP → OK

3. **`src/config.rs`** — Verificar
   - `McpConfig { enabled, servers }` ya existe

### Flujo de Integración
```rust
// En Agent::new():
let mcp_registry = if config.mcp.enabled {
    Some(McpRegistry::load_from_config(&config.mcp.servers).await?)
} else { None };

// Para cada server conectado:
for server_name in mcp_registry.server_names() {
    let client = mcp_registry.client(server_name).unwrap();
    let tools = client.tools().to_vec();
    registry.register_mcp_tools(&mcp_registry, server_name, &tools);
}

// El LLM verá tools con schema: mcp:server:tool
// Cuando llame a mcp:fs:read_file → McpToolWrapper.execute() → registry.call_tool()
```

### Validación Día 8
```bash
# Con filesystem MCP server real
fe "lista archivos en /home/user/proyectos usando el servidor MCP"
# → LLM debería llamar mcp:filesystem:list_dir
```

### Tests Unitarios a Añadir
- `cargo test mcp::integration` — test integración Agent + MCP
- Test que verifica que `all_schemas()` incluye tools MCP con prefijo correcto

---

## Componente 4: Plugin System Base (Días 9-10)

### Arquitectura

```
┌──────────────────────────────────────────────────────────────┐
│                      Plugin Loader                            │
│  ~/.config/fe/plugins/  →  scan dirs → PluginManifest        │
└──────────────────────────────────────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ Plugin 1 │ │ Plugin 2 │ │ Plugin N │
        │ (script) │ │ (script) │ │ (script) │
        └──────────┘ └──────────┘ └──────────┘
              │            │            │
              ▼            ▼            ▼
        ┌────────────────────────────────────┐
        │      PluginRuntime                 │
        │  - Executa binario/script          │
        │  - Stdout JSON protocol            │
        │  - Timeout + resource limits       │
        └────────────────────────────────────┘
                           │
                           ▼
              ┌────────────────────────┐
              │ ToolRegistry (unified) │
              │  - plugin:miplugin:cmd │
              └────────────────────────┘
```

### Estructura Plugin
```
~/.config/fe/plugins/
├── mi-plugin/
│   ├── plugin.toml          # Manifest
│   ├── bin/
│   │   └── mi-plugin        # Ejecutable (python3, bash, binario)
│   └── skills/              # Opcional: skills específicas
│       └── SKILL.md
```

### Plugin Manifest (plugin.toml)
```toml
name = "mi-plugin"
version = "1.0.0"
description = "Mi plugin personalizado"
author = "usuario"
license = "MIT"

# Comando principal (requerido)
command = "python3"
args = ["bin/mi-plugin"]
# O para bash:
# command = "bash"
# args = ["bin/mi-plugin.sh"]

# Variables de entorno
env = { PLUGIN_DATA_DIR = "~/.local/share/fe/plugins/mi-plugin" }

# Tools que expone (se registran en ToolRegistry como plugin:mi-plugin:nombre)
[[tools]]
name = "saludar"
description = "Saluda al usuario"
params = { type = "object", properties = { nombre = { type = "string" } }, required = ["nombre"] }

[[tools]]
name = "backup"
description = "Hace backup de directorio"
params = { type = "object", properties = { origen = { type = "string" }, destino = { type = "string" } }, required = ["origen", "destino"] }

# Comandos CLI que expone (subcomandos: fe plugin mi-plugin <cmd>)
[[commands]]
name = "config"
description = "Configura el plugin"

[[commands]]
name = "status"
description = "Estado del plugin"

# Skills que aporta (opcional)
skills = ["mi-plugin-skill"]
```

### Plugin Protocol (stdin/stdout JSON)
**Request (Fe → Plugin):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tool.call",
  "params": {
    "tool": "saludar",
    "arguments": {"nombre": "Mundo"}
  }
}
```

**Response (Plugin → Fe):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "output": "¡Hola, Mundo!",
    "exit_code": 0
  }
}
```

### CLI Plugins
```bash
fe plugins list                    # Lista plugins instalados
fe plugins reload                  # Hot-reload (notify crate)
fe plugins install <url|path>      # Instala plugin (git clone + setup)
fe plugins uninstall <name>        # Desinstala
fe plugin mi-plugin saludar --nombre "Mundo"  # Llama comando plugin
fe plugin mi-plugin config         # Subcomando del plugin
```

### Skills Aplicables
- **A-project-architecture** — Plugin system extensible, manifest-driven
- **A-secure-coding** — Sandbox execution, resource limits, input validation
- **C-sysadmin** — Process management, file watching (notify), hot-reload
- **D-Agente-IA** — Dynamic tool registration, schema discovery

---

## Estructura de Archivos Final (Fase 5)

```
/home/fe/
├── Cargo.toml                    # + deps: notify, tokio-stream, etc.
├── install.sh                    # Script instalador release
├── PHASE5_PLAN.md                # Este archivo
├── src/
│   ├── main.rs                   # + daemon client routing
│   ├── cli.rs                    # + daemon + mcp + plugins subcomandos
│   ├── config.rs                 # + McpConfig, PluginConfig
│   ├── daemon/
│   │   ├── mod.rs
│   │   ├── server.rs
│   │   ├── protocol.rs
│   │   ├── handler.rs
│   │   └── client.rs
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── types.rs
│   │   └── registry.rs
│   ├── plugins/
│   │   ├── mod.rs
│   │   ├── loader.rs
│   │   ├── manifest.rs
│   │   ├── runtime.rs
│   │   └── registry.rs
│   ├── agent/
│   │   ├── core.rs               # + MCP/Plugin tool integration
│   │   └── tools/mod.rs          # + McpToolWrapper, PluginToolWrapper
│   └── utils/
│       └── completions.rs        # + daemon/mcp/plugins completions
├── config.toml.example           # + [mcp] + [plugins] sections
└── docs/
    ├── user-guide.md
    ├── configuration.md
    ├── daemon.md
    ├── mcp.md
    └── plugins.md
```

---

## Dependencias Cargo.toml (Nuevas)

```toml
[dependencies]
# Existentes: clap, tokio, reqwest, serde, toml, tracing, uuid, chrono, walkdir, globset, regex, async-trait, similar, anyhow

# Daemon
tokio-stream = "0.1"
tokio-util = "0.7"

# Plugins - File watching para hot-reload
notify = "6"

# MCP - JSON-RPC 2.0 implementation (propia, sin crate externo por madurez)
# Si madura: mcp = { git = "https://github.com/modelcontextprotocol/rust-sdk" }

# Serialization extra
serde_json = "1"
```

---

## Tests Objetivo (Meta: 200+ tests)

| Componente | Tests Unitarios | Tests Integración |
|------------|-----------------|-------------------|
| Release/Summarizer | 5 | 3 (wiremock) |
| Daemon Protocol | 15 | 5 (mock socket) |
| Daemon Server | 10 | 3 |
| Daemon Client | 8 | 2 |
| MCP Client | 12 | 4 (mock subprocess) |
| MCP Registry | 8 | 2 |
| Plugin Loader | 10 | 3 (temp dirs) |
| Plugin Runtime | 8 | 2 |
| Plugin Registry | 5 | 1 |
| **Total nuevo** | **~112** | **~25** |
| **Total proyecto** | **~278** | **~25** → **~303** |

---

## Cronograma (10 días laborables)

| Día | Entregable | Validación |
|-----|------------|------------|
| 1 | Tests summarizer + install.sh | `cargo test`, `./install.sh` |
| 2 | Empaquetado + docs + CHANGELOG v0.1.0 | `tar -tzf fe-v0.1.0.tar.gz`, tag git |
| 3 | Daemon protocol + types | `cargo test daemon::protocol` |
| 4 | Daemon server + handler | `cargo test daemon::server` |
| 5 | Daemon client + CLI integration | `fe daemon start && fe "hola" && fe daemon stop` |
| 6 | MCP types + client (spawn, initialize) | ✅ `cargo test mcp::client` |
| 7 | MCP list_tools + call_tool + registry | ✅ `fe mcp discover` |
| 8 | MCP integration en Agent Core | ✅ `fe "usa test-server echo"` |
| 9 | Plugin manifest + loader + registry | ✅ `fe plugins list` |
| 10 | Plugin runtime + CLI + Agent integration | ✅ `fe plugin run test-plugin greet --nombre "Mundo"` |
| — | **Hybrid Pragmatic Plan** (ver `HYBRID_PLAN.md`) | ✅ Provider trait + Router→SkillKind + Skills reorg + Core Services |
| 11 | Integración completa + tests E2E | `cargo test` (300+) |
| 12 | Polish + docs + release v0.1.0 final | Tag, binario, checksums |

---

## Hybrid Pragmatic Plan (Pre-Release)

En lugar de ir directo a Día 11-12, se decidió hacer 4 cambios core internos **antes del release** para evitar refactor post-v0.1.0.

| Cambio | Esfuerzo | Semana | No rompe CLI/features |
|--------|----------|--------|-----------------------|
| Provider trait + OllamaProvider | ~3 días | 1 | ✅ (solo interno) |
| Router → SkillKind enum | ~2 días | 1 | ✅ (misma API) |
| Skills reorg `skills/{domain}/` | ~1 día | 1-2 | ✅ (SkillsLoader ya escanea) |
| Core Services traits | ~5 días | 2 | ✅ (inyección en Agent) |

Ver `HYBRID_PLAN.md` para detalle completo.

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| JSON-RPC 2.0 implementation bugs | Media | Alto | Tests exhaustivos protocol, usar serde_json |
| Daemon zombie processes | Baja | Medio | Proper signal handling, PID file cleanup |
| MCP server spawn failures | Media | Alto | Timeout + retry, fallback graceful, logging |
| Plugin security (arbitrary exec) | Media | Crítico | Sandbox opcional, `allow_destructive` config, env validation |
| Hot-reload race conditions | Media | Medio | notify debounce, atomic registry swap |
| Context window overflow (más tools) | Alta | Medio | Tool filtering por relevance, max_tools config |

---

## Skills Map para DeepSeek V4 Flash Free

| Archivo/Módulo | Skills Principales | Skills Secundarias |
|----------------|-------------------|-------------------|
| `daemon/protocol.rs` | A-secure-coding, B-javascript-clean | A-testing |
| `daemon/server.rs` | C-sysadmin, A-project-architecture | D-Agente-IA |
| `daemon/handler.rs` | A-project-architecture, D-Agente-IA | A-testing |
| `daemon/client.rs` | A-project-architecture | C-sysadmin |
| `mcp/client.rs` | C-sysadmin, A-secure-coding | B-javascript-clean |
| `mcp/types.rs` | A-project-architecture | A-secure-coding |
| `mcp/registry.rs` | A-project-architecture | C-sysadmin |
| `plugins/manifest.rs` | A-project-architecture, C-documentation | A-testing |
| `plugins/loader.rs` | C-sysadmin, A-project-architecture | A-testing |
| `plugins/runtime.rs` | C-sysadmin, A-secure-coding | D-Agente-IA |
| `plugins/registry.rs` | A-project-architecture | A-testing |
| `agent/core.rs` (MCP + Plugins) | D-Agente-IA, A-project-architecture | A-testing |
| `agent/tools/mod.rs` (McpToolWrapper + PluginToolWrapper) | D-Agente-IA, A-project-architecture | A-testing |
| `config.rs` (extensión) | A-project-architecture, C-documentation | A-testing |

---

## Notas para Implementación

1. **Orden de implementación**: Release → Daemon → MCP → Plugins (cada uno buildable independientemente)
2. **Daemon first**: Permite testear MCP/Plugins vía daemon sin reiniciar Fe
3. **MCP stdio**: Usar `tokio::process::Command` con `stdin(Stdio::piped())`, `stdout(Stdio::piped())`
4. **Plugin scripts**: Shebang detection (`#!/usr/bin/env python3`, `#!/bin/bash`), fallback a config `command` + `args`
5. **Tool naming**: Prefijos obligatorios — `mcp:{server}:{tool}`, `plugin:{name}:{tool}`, nativo sin prefijo
6. **Config merge**: Mantener patrón existente — defaults → file → env → CLI
7. **Error handling**: Todos los errors → `anyhow::Result`, logging con `tracing`, user-facing via JSON-RPC error codes
8. **Streaming**: Daemon responses usan `tokio::sync::mpsc` para stream tokens/tools desde Agent a client

---

## Comando de Verificación Final

```bash
# 1. Build release
cargo build --release

# 2. Tests completos
cargo test --workspace 2>&1 | tail -20

# 3. Instalación
./install.sh

# 4. Smoke test daemon
fe daemon start && sleep 2
fe "lista archivos en /tmp"
fe daemon stop

# 5. Smoke test MCP (requiere npx)
fe mcp discover

# 6. Smoke test plugins
mkdir -p ~/.config/fe/plugins/test-plugin/bin
cat > ~/.config/fe/plugins/test-plugin/plugin.toml << 'EOF'
name = "test-plugin"
version = "1.0.0"
command = "bash"
args = ["bin/test-plugin.sh"]
[[tools]]
name = "echo"
description = "Echo test"
params = { type = "object", properties = { msg = { type = "string" } }, required = ["msg"] }
EOF
cat > ~/.config/fe/plugins/test-plugin/bin/test-plugin.sh << 'EOF'
#!/bin/bash
read -r request
# Simple echo response
echo '{"jsonrpc":"2.0","id":1,"result":{"output":"Plugin echo: '"$(echo $request | jq -r '.params.arguments.msg')"'","exit_code":0}}'
EOF
chmod +x ~/.config/fe/plugins/test-plugin/bin/test-plugin.sh
fe plugins reload
fe plugin test-plugin echo --msg "hola desde plugin"

# 7. Todo verde → tag release
git tag v0.1.0 && git push --tags
```

---

*Plan generado para DeepSeek V4 Flash Free — Contexto completo de Fe v0.1.0 + Fase 5*