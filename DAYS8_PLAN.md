# Día 8 — Integración MCP en Agent Core

> Plan para DeepSeek V4 Flash Free — Implementación completa: tools MCP disponibles para LLM

---

## Objetivo

Hacer que los tools MCP (`mcp:{server}:{tool}`) estén disponibles automáticamente para el LLM cuando:
1. El usuario tiene servidores MCP configurados en `config.toml`
2. El LLM usa un modelo con soporte de tools (qwen2.5:3b, deepseek-r1:8b)
3. El agent loop ejecuta `run_agent_loop()`

**Validación objetivo:**
```bash
fe "usa el servidor test-server para hacer echo de 'hola mundo'"
# → LLM ve tool mcp:test-server:echo en schemas y lo usa
```

---

## Arquitectura

```
Agent::new()
  │
  ├─ config.mcp.servers → Vec<McpServerConfig>
  │
  ├─ McpRegistry::load_from_config(&servers) → Arc<Mutex<McpRegistry>>
  │   │
  │   └─ Para cada server: spawn + initialize + list_tools
  │
  ├─ ToolRegistry::register_mcp_tools(&mcp_registry, server, tools)
  │   │
  │   └─ Para cada tool: registra McpToolWrapper con nombre "mcp:server:tool"
  │
  └─ registry.all_schemas() incluye tools MCP → enviados al LLM
```

---

## Archivos a Modificar

### 1. `src/agent/core.rs` — Principal

**Cambios en `Agent` struct:**
```rust
pub struct Agent {
    flags: GlobalFlags,
    client: OllamaClient,
    router: ModelRouter,
    registry: ToolRegistry,
    skills: SkillsLoader,
    session: SessionManager,
    summarizer: Summarizer,
    config: Config,
    // NEW: MCP Registry compartido para lifetime del Agent
    mcp_registry: Option<Arc<Mutex<crate::mcp::registry::McpRegistry>>>,
}
```

**Cambios en `Agent::new()`:**
```rust
pub fn new(config: Config, flags: GlobalFlags) -> Self {
    let mut registry = ToolRegistry::new();
    registry.register_defaults(&config);
    
    // NEW: Inicializar MCP Registry si hay servers configurados
    let mcp_registry = if config.mcp.enabled && !config.mcp.servers.is_empty() {
        let enabled_servers: Vec<_> = config.mcp.servers.iter()
            .filter(|s| s.enabled)
            .cloned()
            .collect();
        
        if !enabled_servers.is_empty() {
            Some(Arc::new(Mutex::new(
                McpRegistry::load_from_config(&enabled_servers).await.unwrap_or_else(|e| {
                    tracing::warn!("MCP init failed: {}", e);
                    McpRegistry::new()
                })
            )))
        } else {
            None
        }
    } else {
        None
    };
    
    // NEW: Registrar tools MCP en ToolRegistry
    if let Some(ref mcp_reg) = mcp_registry {
        // Clone para mover al registry
        let mcp_reg_clone = mcp_reg.clone();
        // Need to discover tools first, then register
        // This requires async, so we might need to defer or do it differently
    }
    
    // ... resto del código existente
}
```

**Problema:** `McpRegistry::load_from_config()` es async, pero `Agent::new()` es sync.

**Solución:** Hacer `Agent::new()` async, o inicializar MCP lazily en `run()`.

---

### 2. Opción A: `Agent::new()` async (Recomendado)

```rust
impl Agent {
    pub async fn new(config: Config, flags: GlobalFlags) -> Result<Self> {
        let mut registry = ToolRegistry::new();
        registry.register_defaults(&config);
        
        let mcp_registry = if config.mcp.enabled && !config.mcp.servers.is_empty() {
            let enabled_servers: Vec<_> = config.mcp.servers.iter()
                .filter(|s| s.enabled)
                .cloned()
                .collect();
            
            if !enabled_servers.is_empty() {
                let reg = McpRegistry::load_from_config(&enabled_servers).await
                    .map_err(|e| {
                        tracing::warn!("MCP init failed: {}", e);
                        anyhow::anyhow!("MCP init failed: {}", e)
                    })
                    .unwrap_or_else(|_| McpRegistry::new());
                
                // Registrar tools MCP
                let mut reg = reg;
                let tools = reg.discover_all_tools().await.unwrap_or_default();
                for (server, tool) in tools {
                    registry.register_mcp_tools(&Arc::new(Mutex::new(reg)), &server, &[tool]);
                }
                
                Some(Arc::new(Mutex::new(reg)))
            } else {
                None
            }
        } else {
            None
        };
        
        // ... resto
        Ok(Self { ..., mcp_registry })
    }
}
```

**Impacto:** Cambiar todos los call sites de `Agent::new()` a `.await`.

---

### 3. Opción B: Inicialización Lazy en `run()` (Menos breaking)

```rust
async fn run(&mut self, prompt: &str) -> Result<()> {
    // Lazy init MCP si no hecho
    if self.mcp_registry.is_none() && self.config.mcp.enabled && !self.config.mcp.servers.is_empty() {
        self.init_mcp().await?;
    }
    // ... resto
}

async fn init_mcp(&mut self) -> Result<()> {
    let enabled_servers: Vec<_> = self.config.mcp.servers.iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect();
    
    let reg = McpRegistry::load_from_config(&enabled_servers).await?;
    let tools = reg.discover_all_tools().await.unwrap_or_default();
    
    let mcp_reg = Arc::new(Mutex::new(reg));
    for (server, tool) in tools {
        self.registry.register_mcp_tools(&mcp_reg, &server, &[tool]);
    }
    
    self.mcp_registry = Some(mcp_reg);
    Ok(())
}
```

---

### 4. Cleanup en Drop

```rust
impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(ref mut mcp_reg) = self.mcp_registry {
            let mcp_reg = mcp_reg.clone();
            // Shutdown en background thread
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new();
                if let Ok(rt) = rt {
                    rt.block_on(async move {
                        let mut reg = mcp_reg.lock().await;
                        reg.shutdown_all().await;
                    });
                }
            });
        }
    }
}
```

---

## Validación

### Test Unitario
```rust
#[tokio::test]
async fn test_agent_mcp_tools_registered() {
    let mut config = Config::default();
    config.mcp.enabled = true;
    config.mcp.servers = vec![McpServerConfig {
        name: "test".into(),
        command: "python3".into(),
        args: vec!["test_mcp_server.py".into()],
        enabled: true,
        env: HashMap::new(),
    }];
    
    let agent = Agent::new(config, GlobalFlags::default()).await.unwrap();
    let schemas = agent.registry.all_schemas();
    
    // Verificar que tools MCP están en schemas
    let has_mcp_tool = schemas.iter().any(|s| {
        s["function"]["name"].as_str().map(|n| n.starts_with("mcp:")).unwrap_or(false)
    });
    assert!(has_mcp_tool, "MCP tools should be registered");
}
```

### Test E2E Manual
```bash
# Terminal 1: configurar
cat > ~/.config/fe/config.toml << 'EOF'
[[mcp.servers]]
name = "test-server"
command = "python3"
args = ["/home/fe/test_mcp_server.py"]
enabled = true
EOF

# Terminal 2: probar
fe "usa el servidor test-server para hacer echo de 'hola mundo'"
# El LLM debería usar mcp:test-server:echo

fe "usa el servidor test-server para saludar a Juan en español"
# El LLM debería usar mcp:test-server:greet

fe "lee el recurso greeting://hello del servidor test-server"
# El LLM no tiene tool para resources aún (eso es futuro)
```

---

## Checklist Día 8

| Tarea | Archivo | Estado |
|-------|---------|--------|
| Agent struct + mcp_registry field | `src/agent/core.rs` | ✅ |
| init_mcp() lazy en run() (Opción B) | `src/agent/core.rs` | ✅ |
| Registrar tools MCP en ToolRegistry | `src/agent/core.rs` | ✅ |
| Drop impl para shutdown | `src/agent/core.rs` | ✅ |
| Call sites sin cambios (new sync) | `src/main.rs`, `src/daemon/handler.rs` | ✅ |
| Test: init_mcp_no_servers | `src/agent/core.rs` | ✅ |
| Test: init_mcp_disabled | `src/agent/core.rs` | ✅ |
| Test: agent_mcp_tools_registered_with_server | `src/agent/core.rs` | ✅ |
| Doc update | `DAYS8_PLAN.md` | ✅ |

### Nota sobre la implementación

Se usó **Opción B (lazy init en `run()`)** para evitar breaking changes en `Agent::new()`.

- `Agent::new()` sigue siendo sync — ningún call site necesitó cambios
- `init_mcp()` se llama al inicio de `run()`
- `McpRegistry::load_from_config()` + `discover_all_tools()` + `register_mcp_tools()` en secuencia
- `Drop` impl shutdown graceful en background thread

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Mitigación |
|--------|--------------|------------|
| MCP spawn falla en Agent::new() | Media | `unwrap_or_else` con registry vacío, log warning |
| Deadlock Mutex en tool execution | Baja | `tokio::sync::Mutex` + `await` correcto |
| Schema mismatch MCP → Fe | Baja | `McpToolWrapper::schema()` ya normaliza input_schema |
| Breaking change Agent::new() async | Media | Opción B (lazy init) si muy invasivo |

---

## Referencias

- `src/agent/tools/mod.rs:87` — `register_mcp_tools()` ya implementado
- `src/mcp/registry.rs:46` — `load_from_config()` async
- `src/mcp/registry.rs:70` — `discover_all_tools()` async
- `test_mcp_server.py` — Server test con 3 tools

---

*Generado para DeepSeek V4 Flash Free — Contexto completo Fe + Phase 5 Día 8*