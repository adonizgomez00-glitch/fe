# Día 6 — MCP Client (Types + Spawn + Initialize + List Tools)

> Plan para DeepSeek V4 Flash Free — Implementación MCP Client con transporte stdio, servidores Python/Rust

---

## Confirmaciones del Usuario

| Decisión | Opción |
|----------|--------|
| MCP Transport | **stdio** (subprocess) |
| Servidor MCP | **Python/Rust** (no npx/Node) |
| Configuración | **config.toml** con `[[mcp.servers]]` |
| Tool naming | **`mcp:{server}:{tool}`** prefix |

---

## Archivos a Crear/Modificar

### 1. `src/mcp/types.rs` — Tipos MCP
```rust
// McpServerConfig, McpTool, McpResource, InitializeResult, ServerCapabilities, ServerInfo
// Serialización JSON-RPC 2.0 compatible
```

### 2. `src/mcp/client.rs` — McpClient
```rust
pub struct McpClient {
    config: McpServerConfig,
    process: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
    request_id: AtomicU64,
    tools: Vec<McpTool>,
    initialized: bool,
}

impl McpClient {
    pub async fn spawn(config: McpServerConfig) -> Result<Self>;
    pub async fn initialize(&mut self) -> Result<InitializeResult>;
    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>>;
    pub async fn call_tool(&mut self, name: &str, args: serde_json::Value) -> Result<serde_json::Value>;
    pub async fn list_resources(&mut self) -> Result<Vec<McpResource>>;
    pub async fn read_resource(&mut self, uri: &str) -> Result<String>;
    pub async fn shutdown(&mut self) -> Result<()>;
}
```

### 3. `src/mcp/registry.rs` — MCP Registry
```rust
pub struct McpRegistry {
    clients: HashMap<String, McpClient>,
}

impl McpRegistry {
    pub async fn load_from_config(config: &Config) -> Result<Self>;
    pub async fn discover_all_tools(&mut self) -> Result<Vec<McpTool>>;
    pub async fn call_tool(&mut self, full_name: &str, args: Value) -> Result<Value>;
    pub async fn shutdown_all(&mut self);
}
```

### 4. `src/config.rs` — Extender Config
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
name = "custom"
command = "cargo"
args = ["run", "--bin", "my_mcp_server"]
env = {}
enabled = true
```

### 5. `src/agent/tools/mod.rs` — McpToolWrapper + Registry Integration
```rust
// Registra tools MCP con prefijo "mcp:{server}:{tool}"
// Implementa trait Tool delegando a McpRegistry.call_tool()
```

### 6. `src/cli.rs` — Subcomandos MCP
```bash
fe mcp discover          # Lista tools/resources de todos los servers
fe mcp servers           # Lista servers configurados + status
fe mcp call <tool> <json_args>  # Llama tool MCP directamente
```

### 7. `src/main.rs` — Routing CLI MCP

---

## Servidores MCP de Referencia (Python/Rust)

### Python: `filesystem_server.py`
```python
#!/usr/bin/env python3
# MCP server básico para filesystem usando stdio JSON-RPC 2.0
# Implementa: initialize, tools/list, tools/call (read_file, write_file, list_dir)
```

### Rust: `mcp-filesystem` (crate independiente o binario en workspace)
```rust
// Cargo.toml separado o [[bin]] en workspace
// Usa tokio + serde_json para stdio JSON-RPC
```

---

## Validación Día 6

```bash
# Tests unitarios
cargo test mcp::types
cargo test mcp::client
cargo test mcp::registry

# Test manual con servidor Python simple
python3 test_mcp_server.py &
fe mcp discover
fe mcp call mcp:test:echo '{"msg": "hola"}'
```

---

## Dependencias Cargo.toml (nuevas)
```toml
# Ya existentes: tokio, serde, serde_json, anyhow
# No se requieren nuevas deps externas para MCP stdio básico
# Opcional futuro: mcp = { git = "https://github.com/modelcontextprotocol/rust-sdk" } cuando madure
```

---

## Notas de Implementación

1. **Process spawn**: `tokio::process::Command` con `stdin(Stdio::piped())`, `stdout(Stdio::piped())`
2. **JSON-RPC framing**: Line-delimited (un mensaje por línea, `\n` separator)
3. **Request/Response**: `request_id` atómico para correlacionar
4. **Initialize handshake**: Obligatorio antes de `tools/list`
4. **Error handling**: Timeout en spawn, reconexión graceful, cleanup en drop
5. **Security**: Validar `command`/`args` contra allowlist, sanitizar `env`
6. **Tool schema normalization**: Convertir MCP input_schema a formato Fe ToolSchema

---

## Orden de Implementación Sugerido

1. `src/mcp/types.rs` — Tipos base
2. `src/mcp/client.rs` — Cliente con spawn/initialize/list_tools/call_tool
3. `src/mcp/registry.rs` — Registry multi-server
4. `src/config.rs` — Añadir `McpConfig` + merge
5. `src/agent/tools/mod.rs` — `McpToolWrapper` + registro en `ToolRegistry`
6. `src/cli.rs` — Subcomandos `mcp`
7. `src/main.rs` — Routing CLI
8. Tests unitarios + test integración con servidor Python simple

---

## Próximos Días (Referencia)

| Día | Entregable |
|-----|------------|
| **7** | `list_resources`, `read_resource`, `prompts`, CLI completo |
| **8** | Integración completa en Agent core (tools MCP disponibles para LLM) |
| **9** | Plugin system (manifest, loader, registry) |
| **10** | Plugin runtime (scripts Python/Bash, hot-reload) |
| **11** | Tests E2E, integración completa |
| **12** | Polish, docs, release v0.1.0 final |

---

*Generado para DeepSeek V4 Flash Free — Contexto completo Fe + Phase 5 Día 6*