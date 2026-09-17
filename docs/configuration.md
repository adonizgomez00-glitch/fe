# Configuración de Fe — Referencia Completa

Fe se configura mediante el archivo `~/.config/fe/config.toml`. El orden de precedencia es:

**Defaults en código < Archivo config.toml < Variables de entorno FE_* < Flags CLI**

---

## Archivo Completo de Ejemplo

```toml
# ~/.config/fe/config.toml

[ollama]
host = "http://localhost:11434"
timeout_secs = 300
max_retries = 3
keep_alive_secs = 300

[models]
complex = "qwen2.5:3b"
simple = "gemma3:4b"
reasoning = "qwen2.5:3b"
summarizer = "gemma3:4b"

[agent]
name = "fe"
confirm_tools = true
batch_mode = false
auto_save = true
summarize_every_n_turns = 4
max_history_turns = 20
max_context_tokens = 8192

[tools]
bash_timeout_secs = 120
max_output_lines = 5000
max_output_bytes = 1048576
allow_destructive = false

[paths]
sessions_dir = "~/.local/share/fe/sessions"
skills_dir = "~/.config/opencode/skills"
config_dir = "~/.config/fe"

[shell]
generate_completions = true
shells = ["bash", "zsh", "fish"]
```

---

## Sección `[ollama]`

Configura la conexión con el servidor Ollama.

| Campo | Tipo | Default | Env Var | Descripción |
|-------|------|---------|---------|-------------|
| `host` | string | `http://localhost:11434` | `FE_OLLAMA_HOST` | URL base de Ollama |
| `timeout_secs` | integer | `300` | `FE_OLLAMA_TIMEOUT_SECS` | Timeout máximo de requests HTTP |
| `max_retries` | integer | `3` | `FE_OLLAMA_MAX_RETRIES` | Reintentos con backoff exponencial |
| `keep_alive_secs` | integer | `300` | `FE_OLLAMA_KEEP_ALIVE_SECS` | Tiempo que el modelo persiste en RAM |

### keep_alive_secs
Controla cuánto tiempo el modelo permanece cargado en RAM después de una consulta.
- **300s (default)**: Buen balance entre memoria y velocidad
- **0**: Descarga inmediata (menor uso de RAM, más latencia)
- **-1**: Persistencia infinita (máxima velocidad, usa RAM permanentemente)

**Recomendación**: 300s para uso interactivo, -1 para daemon mode.

---

## Sección `[models]`

Define qué modelos usa Fe para cada rol.

| Campo | Tipo | Default | Env Var | Descripción |
|-------|------|---------|---------|-------------|
| `complex` | string | `qwen2.5:3b` | `FE_MODELS_COMPLEX` | Tareas complejas + tools |
| `simple` | string | `gemma3:4b` | `FE_MODELS_SIMPLE` | Tareas simples (sin tools) |
| `reasoning` | string | `qwen2.5:3b` | `FE_MODELS_REASONING` | Razonamiento profundo |
| `summarizer` | string | `gemma3:4b` | `FE_MODELS_SUMMARIZER` | Resúmenes automáticos |

### Criterios de Selección

| Condición | Modelo | Tools |
|-----------|--------|-------|
| Flag `--reasoning` | reasoning | ✗ |
| Flag `--fast` | simple | ✗ |
| Prompt simple (¿qué es?, define, explica) | simple | ✗ |
| Prompt con acción (crea, elimina, instala) | complex | ✓ |
| Cualquier otro | complex | ✓ |

### Modelos recomendados

| Modelo | Params | Tools | Velocidad | Uso |
|--------|--------|:-----:|:---------:|-----|
| qwen2.5:3b | 3.0B | ✓ | ★★★★ | Default complex |
| gemma3:4b | 4.3B | ✗ | ★★★★★ | Default simple/summarizer |
| deepseek-r1:8b | 8.2B | thinking | ★★ | Reasoning (lento) |
| qwen2.5:7b | 7.6B | ✓ | ★★★ | Complex alternativo |
| llama3.2:3b | 3.2B | ✓ | ★★★★ | Complex alternativo |

---

## Sección `[agent]`

Comportamiento del agente.

| Campo | Tipo | Default | Env Var | Descripción |
|-------|------|---------|---------|-------------|
| `name` | string | `fe` | `FE_AGENT_NAME` | Nombre del agente en system prompt |
| `confirm_tools` | bool | `true` | `FE_AGENT_CONFIRM_TOOLS` | Confirmar cada tool call |
| `batch_mode` | bool | `false` | `FE_AGENT_BATCH_MODE` | Modo batch por defecto |
| `auto_save` | bool | `true` | `FE_AGENT_AUTO_SAVE` | Guardar sesiones automáticamente |
| `summarize_every_n_turns` | integer | `4` | `FE_AGENT_SUMMARIZE_EVERY_N_TURNS` | Resumen cada N turns (0 desactiva) |
| `max_history_turns` | integer | `20` | `FE_AGENT_MAX_HISTORY_TURNS` | Turnos máximos en historial |
| `max_context_tokens` | integer | `8192` | `FE_AGENT_MAX_CONTEXT_TOKENS` | Context window (`num_ctx`) |

### confirm_tools
- `true` (default): Pregunta antes de ejecutar cada herramienta
- `false`: Ejecuta automáticamente (herramientas no destructivas igual se ejecutan sin preguntar)

### summarize_every_n_turns
- `0`: Desactiva resúmenes automáticos
- `4` (default): Resume cada 4 turnos
- `1`: Resume después de cada turno (más tokens, más contexto)

---

## Sección `[tools]`

Controla la ejecución de herramientas.

| Campo | Tipo | Default | Env Var | Descripción |
|-------|------|---------|---------|-------------|
| `bash_timeout_secs` | integer | `120` | `FE_TOOLS_BASH_TIMEOUT_SECS` | Timeout máximo por comando |
| `max_output_lines` | integer | `5000` | `FE_TOOLS_MAX_OUTPUT_LINES` | Líneas máximas de output |
| `max_output_bytes` | integer | `1048576` | `FE_TOOLS_MAX_OUTPUT_BYTES` | Bytes máximos de output (1MB) |
| `allow_destructive` | bool | `false` | `FE_TOOLS_ALLOW_DESTRUCTIVE` | Permitir comandos destructivos |

### allow_destructive
- `false` (default): Comandos destructivos (`rm -rf`, `sudo`, `dd`, etc.) requieren confirmación extra
- `true`: No requiere confirmación extra (override con flag `--destructive`)

### Comandos Detectados como Destructivos
- `rm` / `rm -rf`
- `dd`
- `mkfs` / `format`
- `chmod 0`
- `chown`
- `sudo`
- `passwd`

---

## Sección `[paths]`

Directorios del sistema.

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `sessions_dir` | string | `~/.local/share/fe/sessions` | Sesiones persistentes |
| `skills_dir` | string | `~/.config/opencode/skills` | Skills de opencode |
| `plugins_dir` | string | `~/.config/fe/plugins` | Plugins instalados |
| `config_dir` | string | `~/.config/fe` | Directorio de configuración |

### skills_dir
Fe reutiliza las skills de opencode. Si usas otro gestor de skills, cambia esta ruta:
```toml
[paths]
skills_dir = "~/.config/mis-skills"
```

---

## Sección `[shell]`

Completions para el shell.

| Campo | Tipo | Default | Env Var | Descripción |
|-------|------|---------|---------|-------------|
| `generate_completions` | bool | `true` | `FE_SHELL_GENERATE_COMPLETIONS` | Generar completions |
| `shells` | array | `["bash", "zsh", "fish"]` | — | Shells para completions |

---

## Sección `[mcp]`

Configuración del Model Context Protocol.

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `enabled` | bool | `true` | Habilitar/deshabilitar MCP globalmente |
| `servers` | array | `[]` | Lista de servidores MCP |

### Servidores MCP

Cada servidor se configura como `[[mcp.servers]]`:

| Campo | Tipo | Default | Descripción |
|-------|------|---------|-------------|
| `name` | string | — | Nombre único del servidor |
| `command` | string | — | Comando para ejecutar el servidor |
| `args` | array | `[]` | Argumentos del comando |
| `env` | table | `{}` | Variables de entorno adicionales |
| `enabled` | bool | `true` | Habilitar/deshabilitar este servidor |

### Ejemplo

```toml
[mcp]
enabled = true

[[mcp.servers]]
name = "filesystem"
command = "python3"
args = ["/home/user/mcp-servers/filesystem_server.py", "/home/user/projects"]
enabled = true

[[mcp.servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "ghp_xxx" }
enabled = true
```

### Integración con el agente

Cuando el agente inicia, descubre automáticamente los tools de todos los servidores MCP configurados y los registra en `ToolRegistry` con el nombre `mcp:{server}:{tool}`. El LLM puede usarlos como herramientas nativas.

---

## Variables de Entorno (Referencia Rápida)

### Conexión Ollama
```bash
export FE_OLLAMA_HOST=http://localhost:11434
export FE_OLLAMA_TIMEOUT_SECS=300
export FE_OLLAMA_MAX_RETRIES=3
export FE_OLLAMA_KEEP_ALIVE_SECS=300
```

### Modelos
```bash
export FE_MODELS_COMPLEX=qwen2.5:3b
export FE_MODELS_SIMPLE=gemma3:4b
export FE_MODELS_REASONING=qwen2.5:3b
export FE_MODELS_SUMMARIZER=gemma3:4b
```

### Comportamiento Agente
```bash
export FE_AGENT_NAME=fe
export FE_AGENT_CONFIRM_TOOLS=true
export FE_AGENT_BATCH_MODE=false
export FE_AGENT_AUTO_SAVE=true
export FE_AGENT_SUMMARIZE_EVERY_N_TURNS=4
export FE_AGENT_MAX_HISTORY_TURNS=20
export FE_AGENT_MAX_CONTEXT_TOKENS=8192
```

### Herramientas
```bash
export FE_TOOLS_BASH_TIMEOUT_SECS=120
export FE_TOOLS_MAX_OUTPUT_LINES=5000
export FE_TOOLS_MAX_OUTPUT_BYTES=1048576
export FE_TOOLS_ALLOW_DESTRUCTIVE=false
```

---

## Configuración Avanzada

### Rendimiento

```toml
[ollama]
keep_alive_secs = 600    # Mantener modelo 10 minutos en RAM

[agent]
max_context_tokens = 4096  # Reducir contexto para más velocidad
summarize_every_n_turns = 0  # Desactivar resúmenes
```

### Modo Headless (CI/CD)

```toml
[agent]
confirm_tools = false
batch_mode = true
auto_save = false
summarize_every_n_turns = 0

[tools]
allow_destructive = true
```

### Mínima Configuración

```toml
[ollama]
host = "http://ollama.local:11434"
```

Todos los demás valores usan defaults.
