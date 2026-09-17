# Día 9 — Plugin System Base: Manifest + Loader + Registry

> Implementación completa: plugin.toml parser, filesystem scanner, plugin registry

---

## Objetivo

Crear la capa base del sistema de plugins de Fe:
1. **PluginManifest**: parser de `plugin.toml` con serde
2. **PluginLoader**: escaneo de `~/.config/fe/plugins/*/plugin.toml`
3. **PluginRegistry**: gestión en memoria de plugins cargados
4. **CLI**: `fe plugins list` para inspeccionar plugins instalados

**Validación objetivo:**
```bash
fe plugins list
# 📦 Plugins instalados (1):
#   mi-plugin v1.0.0
#     Descripción: Mi plugin personalizado
#     Tools: 2 (saludar, backup)
#     Comandos: 2 (config, status)
```

---

## Arquitectura

```
src/plugins/
├── mod.rs           # Re-exports módulo
├── manifest.rs      # PluginManifest + tipos serde
├── loader.rs        # scan ~/.config/fe/plugins/ → LoadedPlugin[]
└── registry.rs      # PluginRegistry (HashMap<String, LoadedPlugin>)
```

### Flujo de Carga

```
fe plugins list
  │
  ├─ PluginRegistry::new()
  │
  ├─ load_from(&plugins_dir)
  │   │
  │   └─ load_plugins(plugins_dir)
  │       │
  │       ├─ read_dir → por cada subdirectorio
  │       ├─ existe plugin.toml?
  │       │   ├─ Sí → PluginManifest::from_path() → LoadedPlugin
  │       │   └─ No → skip
  │       └─ Vec<LoadedPlugin>
  │
  └─ print plugins
```

---

## Archivos Creados/Modificados

### 1. `src/plugins/manifest.rs` — Tipos PluginManifest

```rust
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub tools: Vec<PluginTool>,
    pub commands: Vec<PluginCommand>,
    pub skills: Vec<String>,
}

pub struct PluginTool {
    pub name: String,
    pub description: String,
    pub params: serde_json::Value,
}

pub struct PluginCommand {
    pub name: String,
    pub description: String,
}
```

**Métodos:**
- `from_path(path: &Path) -> Result<Self>` — lee y parsea plugin.toml
- `from_str(toml_str: &str) -> Result<Self>` — parsea desde string

### 2. `src/plugins/loader.rs` — Carga de Plugins

```rust
pub struct LoadedPlugin {
    pub dir: PathBuf,
    pub manifest: PluginManifest,
}

pub fn load_plugins(plugins_dir: &Path) -> Vec<LoadedPlugin>
pub fn load_plugins_from(paths: &[PathBuf]) -> Vec<LoadedPlugin>
```

### 3. `src/plugins/registry.rs` — PluginRegistry

```rust
pub struct PluginRegistry {
    plugins: HashMap<String, LoadedPlugin>,
}

impl PluginRegistry {
    pub fn new()
    pub fn load_from(&mut self, plugins_dir: &Path)
    pub fn load_from_multiple(&mut self, dirs: &[PathBuf])
    pub fn reload(&mut self, dirs: &[PathBuf])
    pub fn get(&self, name: &str) -> Option<&LoadedPlugin>
    pub fn list(&self) -> Vec<&LoadedPlugin>
    pub fn count(&self) -> usize
    pub fn contains(&self, name: &str) -> bool
    pub fn names(&self) -> Vec<String>
    pub fn remove(&mut self, name: &str) -> bool
    pub fn clear(&mut self)
}
```

### 4. `src/plugins/mod.rs` — Module

```rust
pub mod manifest;
pub mod loader;
pub mod registry;
```

### 5. `src/config.rs` — plugins_dir

```rust
pub struct PathsConfig {
    pub sessions_dir: PathBuf,
    pub skills_dir: PathBuf,
    pub plugins_dir: PathBuf,   // NEW
    pub config_dir: PathBuf,
}
```

Default: `config_dir.join("plugins")` = `~/.config/fe/plugins`

### 6. `src/cli.rs` — Subcomando Plugin

```rust
enum Command {
    // ...
    Plugin(PluginAction),
}

enum PluginAction {
    List,
}
```

### 7. `src/main.rs` — Handler

```rust
mod plugins;

cli::Command::Plugin(action) => {
    handle_plugin(action, &config).await?;
}

async fn handle_plugin(action: &cli::PluginAction, config: &Config) -> Result<()> {
    // Load + print plugins
}
```

---

## Checklist Día 9

| Tarea | Archivo | Estado |
|-------|---------|--------|
| PluginManifest + tipos serde | `src/plugins/manifest.rs` | ✅ |
| PluginLoader (scan dirs) | `src/plugins/loader.rs` | ✅ |
| PluginRegistry (HashMap) | `src/plugins/registry.rs` | ✅ |
| Módulo plugins/mod.rs | `src/plugins/mod.rs` | ✅ |
| Config: plugins_dir | `src/config.rs` | ✅ |
| CLI: fe plugins list | `src/cli.rs` | ✅ |
| Handler plugins | `src/main.rs` | ✅ |
| Tests unitarios (23) | `src/plugins/*.rs` | ✅ |
| Doc update | Various .MD | ✅ |

---

## Validación

### Test Unitario
```rust
#[test]
fn test_load_single_plugin() {
    let dir = temp_dir();
    create_test_plugin(&dir, "test-plugin");
    let plugins = load_plugins(&dir);
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].manifest.name, "test-plugin");
}
```

### Test Manual
```bash
# Crear plugin de prueba
mkdir -p ~/.config/fe/plugins/mi-plugin
cat > ~/.config/fe/plugins/mi-plugin/plugin.toml << 'EOF'
name = "mi-plugin"
version = "1.0.0"
description = "Plugin de prueba"
command = "bash"
args = ["run.sh"]

[[tools]]
name = "saludar"
description = "Saluda al usuario"
params = { type = "object", properties = { nombre = { type = "string" } }, required = ["nombre"] }
EOF

# Listar plugins
fe plugins list
# → Debe mostrar mi-plugin con tool "saludar"
```

---

## Día 10 — Plugin Runtime + CLI + Agent Integration ✅

**Estado:** ✅ Completado  
**Tests:** 283 pasando (5 nuevos)  
**Release build:** ✅  
**Validación E2E:** ✅ `fe plugin run test-plugin greet --nombre Mundo` → `Hola, Mundo!`

### Entregables Día 10

| Entregable | Estado | Archivo |
|------------|--------|---------|
| PluginRuntime (subprocess JSON-RPC) | ✅ | `src/plugins/runtime.rs` |
| PluginToolWrapper + ToolRegistry | ✅ | `src/agent/tools/mod.rs` |
| Agent core: `init_plugins()` lazy | ✅ | `src/agent/core.rs` |
| Plugin CLI: `fe plugin run <name> <cmd>` | ✅ | `src/cli.rs` + `src/main.rs` |
| Plugin CLI: `fe plugins reload` | ✅ | `src/main.rs` |
| Plugin CLI: `fe plugins install/uninstall` | ✅ (esqueleto) | `src/main.rs` |
| Drop cleanup de runtimes | ✅ | `src/agent/core.rs` |
| Tests unitarios (5) | ✅ | `src/plugins/runtime.rs` |

### Comandos Nuevos

```bash
fe plugin list                     # Lista plugins instalados
fe plugin reload                   # Recarga plugins del disco
fe plugin run <name> <cmd> [args]  # Ejecuta comando de plugin
fe plugin install <source>         # (esqueleto)
fe plugin uninstall <name>         # Desinstala plugin
```

---

## Riesgos

| Riesgo | Mitigación |
|--------|------------|
| plugin.toml mal formado | `toml::from_str` captura errores, log warning, skip |
| Directorio plugins no existe | `load_plugins()` retorna vec vacío sin error |
| Duplicado de nombres | HashMap overwrite, último gana |
| Path traversal | Solo escanea `plugins_dir`, no sigue symlinks |

---

## Referencias

- `src/plugins/manifest.rs:1-80` — PluginManifest + tests
- `src/plugins/loader.rs:1-80` — load_plugins + tests
- `src/plugins/registry.rs:1-120` — PluginRegistry + tests
- `src/config.rs:56-62` — plugins_dir field

---

*Generado para DeepSeek V4 Flash Free — Contexto completo Fe + Phase 5 Día 9*
