# Guía de Usuario — Fe v0.1.1

Fe es un agente CLI local que conecta tu terminal con modelos LLM vía Ollama para administración de sistemas, desarrollo de software, arquitectura y más.

---

## Índice

- [Instalación](#instalación)
- [Primeros Pasos](#primeros-pasos)
- [Comandos](#comandos)
- [Flags Globales](#flags-globales)
- [Gestión de Sesiones](#gestión-de-sesiones)
- [Gestión de Configuración](#gestión-de-configuración)
- [Shell Completions](#shell-completions)
- [Diagnóstico](#diagnóstico)
- [Ejecución Directa](#ejecución-directa)
- [Sistema de Templates](#sistema-de-templates)
- [Skills](#skills)
- [Resumen Automático](#resumen-automático)
- [MCP (Model Context Protocol)](#mcp-model-context-protocol)
- [Plugins](#plugins)
- [FAQ](#faq)

---

## Instalación

### Requisitos
- **Ollama** corriendo en `localhost:11434` ([guía de instalación](https://ollama.ai/download))
- **Rust** (solo para compilar desde fuente)

### Desde fuente
```bash
git clone <repo-url> fe
cd fe
cargo build --release
cp target/release/fe ~/.cargo/bin/
```

### Usando install.sh
```bash
git clone <repo-url> fe
cd fe
chmod +x install.sh
./install.sh
```

El script:
1. Compila `fe` con `cargo build --release`
2. Copia el binario a `~/.cargo/bin/fe`
3. Crea `~/.config/fe/config.toml` desde `config.toml.example` si no existe
4. Crea `~/.local/share/fe/sessions/`

### Verificar instalación
```bash
fe doctor
```

---

## Primeros Pasos

```bash
# Diagnosticar conectividad con Ollama
fe doctor

# Pregunta simple (usa gemma3:4b, rápido)
fe --fast "qué es rust"

# Tarea de sistema (usa qwen2.5:3b con herramientas)
fe "actualiza repositorios"

# Crear un proyecto
fe "crea una app python que sume y reste"

# Listar archivos (ejecución directa, sin LLM)
fe "lista archivos"
```

---

## Comandos

### `fe <prompt>`
Ejecuta el agente con un prompt en lenguaje natural. El router selecciona automáticamente el modelo adecuado según el contenido del prompt y los flags.

```bash
fe "crea un directorio llamado proyectos"
fe "muestra el contenido de README.md"
fe "analiza la arquitectura de src/"
```

### `fe session`
Gestión de sesiones persistentes. Cada invocación crea una sesión con UUID que guarda el historial de la conversación.

```bash
fe session list              # Lista todas las sesiones
fe session show <id>         # Muestra contenido completo
fe session export <id> <ruta>  # Exporta a archivo markdown
fe session clean             # Elimina sesiones >30 días
```

### `fe config`
Visualiza y edita la configuración.

```bash
fe config show               # Muestra configuración actual en TOML
fe config edit               # Abre config.toml en el editor (EDITOR env)
```

### `fe completions <shell>`
Genera completions para el shell.

```bash
fe completions bash          # Genera para bash
fe completions zsh           # Genera para zsh
fe completions fish          # Genera para fish
```

**Instalación de completions:**

```bash
# Bash
fe completions bash > /etc/bash_completion.d/fe

# Zsh (oh-my-zsh)
mkdir -p ~/.oh-my-zsh/completions
fe completions zsh > ~/.oh-my-zsh/completions/_fe

# Fish
fe completions fish > ~/.config/fish/completions/fe.fish
```

### `fe doctor`
Diagnóstico completo del sistema.

```bash
fe doctor
```

Verifica:
- Conectividad con Ollama (`/api/tags`)
- Archivo de configuración
- Directorio de sesiones
- Directorio de skills opencode

---

## Flags Globales

| Flag | Descripción |
|------|-------------|
| `--batch` | Modo batch: muestra plan completo, confirma una vez, ejecuta todo |
| `--reasoning` | Usa modelo de razonamiento (default: qwen2.5:3b) |
| `--fast` | Usa modelo rápido sin tools (default: gemma3:4b) |
| `--dry-run` | Muestra el plan de ejecución sin ejecutar nada |
| `--destructive` | Omite confirmación extra para acciones destructivas |

### Ejemplos por flag

```bash
# Batch: instalar paquetes con una confirmación
fe --batch "apt update && apt upgrade -y && apt autoremove"

# Reasoning: análisis profundo
fe --reasoning "diseña la arquitectura de un microservicio en rust"

# Fast: consulta rápida (sin tools)
fe --fast "qué significa SOLID en programación"

# Dry-run: ver plan sin ejecutar
fe --dry-run "crea un script de backup en bash"

# Destructive: omitir advertencias destructivas
fe --destructive "rm -rf /tmp/cache"
```

### Orden de Precedencia
1. Valores por defecto en código
2. Archivo `~/.config/fe/config.toml`
3. Variables de entorno `FE_*`
4. Flags CLI (máxima prioridad)

---

## Gestión de Sesiones

Cada interacción con Fe crea una sesión persistente en `~/.local/share/fe/sessions/<uuid>.md`.

### Formato de sesión
```markdown
# Sesión <uuid>
- **Inicio**: 2026-07-10 12:00:00
- **Modelo**: qwen2.5:3b
- **Skills**: C-sysadmin, D-git-workflow

## Resumen de Sesión
<resumen automático>

## Turno 1
**Hora**: 2026-07-10 12:00:01
### Prompt
actualiza repositorios
### Respuesta
<respuesta del asistente>
```

### Limpieza automática
```bash
fe session clean  # Elimina sesiones >30 días
```

---

## Ejecución Directa

Fe puede ejecutar comandos comunes **sin pasar por el LLM**, lo que hace que operaciones simples sean instantáneas:

| Prompt | Comando |
|--------|---------|
| "lista archivos" | `ls -la` |
| "donde estoy" | `pwd` |
| "crea directorio X" | `mkdir -p X` |
| "elimina archivo X" | `rm X` |
| "elimina directorio X" | `rm -rf X` |
| "muestra contenido de X" | `cat X` |
| "crea archivo vacío X" | `touch X` |
| "busca X" | `find . -name '*X*'` |
| "actualiza repositorios" | `apt update` |
| "crea app python que X" | Genera proyecto completo |

Si el prompt no coincide con ningún patrón, se delega automáticamente al LLM.

---

## Sistema de Templates

Fe puede generar proyectos completos multi-archivo con un solo prompt.

### Lenguajes y tipos soportados

| Lenguaje | CLI | Desktop | Web | Admin | Doc |
|----------|:---:|:-------:|:---:|:-----:|:---:|
| Python | ✅ | ✅ | ✅ | ✅ | ✅ |
| Node.js | ✅ | ✅ | ✅ | — | — |
| Rust | ✅ | — | ✅ | — | — |
| Bash | ✅ | — | — | ✅ | — |

### Ejemplos
```bash
fe "crea una app python cli que sume y reste"
fe "crea un script node web que sirva una api rest"
fe "crea una app python desktop con tkinter"
fe "crea un script bash de respaldo automático"
fe "crea una app rust cli que gestione tareas"
```

Cada template incluye:
- Código funcional con argparse/getopts
- Tests unitarios
- Requirements (Python: requirements.txt, Node: package.json, Rust: Cargo.toml)
- Manejo de errores y logging

---

## Skills

Fe reutiliza las skills de opencode desde `~/.config/opencode/skills/`. Cada skill es un directorio con un archivo `SKILL.md`.

### Skills detectadas automáticamente

Fe analiza tu prompt y activa las skills relevantes:

```bash
# Activa C-sysadmin
fe "configura el firewall con ufw"

# Activa D-git-workflow
fe "crea un commit y haz push a main"

# Activa A-testing
fe "escribe tests unitarios para el módulo parser"
```

### Formato SKILL.md
```markdown
---
name: C-sysadmin
description: Administración profesional de sistemas Linux
---

# Contenido de la skill...
```

### Skills incluidas
| Skill | Categoría | Propósito |
|-------|-----------|-----------|
| A-coding-standards | A | Estándares de código |
| A-project-architecture | A | Arquitectura de software |
| A-secure-coding | A | Seguridad (OWASP, NIST) |
| A-testing | A | Generación de pruebas |
| B-authentication-security | B | Autenticación |
| B-html-css | B | HTML semántico y CSS |
| B-javascript-clean | B | JavaScript moderno |
| B-ui-components | B | Componentes UI |
| C-database-design-offline | C | IndexedDB + Dexie |
| C-database-design-sql | C | SQL databases |
| C-debugging | C | Diagnóstico de errores |
| C-dexie-patterns | C | Patrones Dexie |
| C-documentation | C | Documentación |
| C-sysadmin | C | Administración Linux |
| D-Agente-IA | D | Mejora de agente IA |
| D-erp-offline | D | ERP offline |
| D-git-workflow | D | Git workflow |
| D-prompt-engineering | D | Ingeniería de prompts |

---

## Resumen Automático

Cada N turns (configurable vía `summarize_every_n_turns`, default: 4), Fe genera automáticamente un resumen de la sesión usando gemma3:4b.

```bash
# Configurar resumen cada 2 turns
export FE_AGENT_SUMMARIZE_EVERY_N_TURNS=2
# O en config.toml:
# [agent]
# summarize_every_n_turns = 2
```

### Qué incluye el resumen
- Comandos ejecutados
- Archivos creados/modificados
- Resultados principales
- Estado actual del proyecto
- Skills tags preservados: `[skill:C-sysadmin:package-management]`
- Merge con resumen anterior

---

---

## MCP (Model Context Protocol)

Fe soporta el **Model Context Protocol (MCP)**, un estándar abierto para conectar LLMs con herramientas y datos externos.

### Configuración

Agrega servidores MCP en `~/.config/fe/config.toml`:

```toml
[mcp]
enabled = true

[[mcp.servers]]
name = "filesystem"
command = "python3"
args = ["/path/to/mcp_server.py", "/allowed/path"]
enabled = true
```

### Comandos MCP

```bash
fe mcp discover              # Descubre tools, resources y prompts de todos los servers
fe mcp servers               # Lista servers configurados y su estado
fe mcp resources             # Lista solo recursos
fe mcp prompts               # Lista solo prompts
fe mcp call mcp:server:tool '{"arg":"value"}'  # Ejecuta tool directamente
fe mcp read server uri       # Lee un recurso
fe mcp get-prompt server name '{"arg":"value"}'  # Obtiene un prompt
```

### Integración con el agente

Cuando el agente detecta que necesita una herramienta MCP, la usa automáticamente con el nombre `mcp:{server}:{tool}`. Por ejemplo, si tienes un servidor MCP con una tool `read_file`, el LLM puede llamarla como `mcp:filesystem:read_file`.

**Ejemplo:**
```bash
fe "usa el servidor test-server para hacer echo de 'hola mundo'"
# → El LLM ve y usa la tool mcp:test-server:echo
```

---

## Plugins

Fe tiene un sistema de plugins basado en manifiestos `plugin.toml`. Los plugins se instalan en `~/.config/fe/plugins/`.

### Estructura de un plugin

```
~/.config/fe/plugins/mi-plugin/
├── plugin.toml          # Manifest del plugin
└── bin/
    └── mi-plugin        # Ejecutable (script o binario)
```

### Manifest plugin.toml

```toml
name = "mi-plugin"
version = "1.0.0"
description = "Mi plugin personalizado"
author = "usuario"
command = "bash"
args = ["bin/mi-plugin.sh"]
env = { DATA_DIR = "/tmp/data" }

[[tools]]
name = "saludar"
description = "Saluda al usuario"
params = { type = "object", properties = { nombre = { type = "string" } }, required = ["nombre"] }

[[commands]]
name = "config"
description = "Configura el plugin"
```

### Comandos Plugins

```bash
fe plugins list              # Lista plugins instalados con tools y comandos
```

### Ejemplo rápido

```bash
# Crear un plugin de prueba
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
```

### Comandos Avanzados

```bash
fe plugins list                    # Lista plugins instalados con tools y comandos
fe plugins reload                  # Recarga plugins del disco
fe plugin run <name> <cmd> [args]  # Ejecuta comando de plugin
fe plugin install <source>         # Instala plugin (esqueleto)
fe plugin uninstall <name>         # Desinstala plugin
```

### Ejemplo rápido con runtime

```bash
# Crear un plugin de prueba
mkdir -p ~/.config/fe/plugins/test-plugin/bin
cat > ~/.config/fe/plugins/test-plugin/plugin.toml << 'EOF'
name = "test-plugin"
version = "1.0.0"
description = "Plugin de prueba"
command = "bash"
args = ["bin/test-plugin.sh"]
[[tools]]
name = "echo"
description = "Echo test"
params = { type = "object", properties = { msg = { type = "string" } }, required = ["msg"] }
EOF

cat > ~/.config/fe/plugins/test-plugin/bin/test-plugin.sh << 'SCRIPT'
#!/bin/bash
read -r request
echo '{"jsonrpc":"2.0","id":1,"result":{"output":"Plugin echo: '"$(echo $request | jq -r '.params.arguments.msg')"'","exit_code":0}}'
SCRIPT

chmod +x ~/.config/fe/plugins/test-plugin/bin/test-plugin.sh
fe plugins reload
fe plugin test-plugin echo --msg "hola desde plugin"
```

---

## FAQ

### ¿Fe funciona sin conexión a internet?
Sí. Fe usa Ollama local, que corre completamente en tu máquina. No requiere conexión externa.

### ¿Qué modelos necesita Ollama?
Mínimo: `qwen2.5:3b` y `gemma3:4b`.
```bash
ollama pull qwen2.5:3b
ollama pull gemma3:4b
```

### ¿Fe es seguro?
- Acciones destructivas (`rm -rf`, `sudo`, `dd`) requieren confirmación explícita
- Flag `--destructive` para override
- Variables de entorno no se exponen en logs
- Comandos validados antes de ejecutar

### ¿Puedo usar Fe con otros modelos?
Sí, configura en `~/.config/fe/config.toml`:
```toml
[models]
complex = "mi-modelo:tag"
simple = "otro-modelo:tag"
```

### ¿Cómo acelero Fe?
- Usa `--fast` para consultas simples (evita carga de modelo grande)
- Usa `--batch` para tareas multi-paso (una confirmación en vez de N)
- Mantén el modelo en RAM: `keep_alive_secs = 300` (default)
- Habilita GPU: `OLLAMA_IGPU_ENABLE=1`
- Reduce `num_ctx` si usas contexto pequeño

### ¿Dónde están los logs?
Fe usa `tracing` con nivel `info`. Para más detalle:
```bash
FE_LOG=debug fe "comando"
# O
RUST_LOG=debug fe "comando"
```

---

## Variables de Entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `FE_OLLAMA_HOST` | `http://localhost:11434` | Host de Ollama |
| `FE_OLLAMA_TIMEOUT_SECS` | `300` | Timeout de requests |
| `FE_OLLAMA_MAX_RETRIES` | `3` | Reintentos máximos |
| `FE_OLLAMA_KEEP_ALIVE_SECS` | `300` | Persistencia del modelo en RAM |
| `FE_MODELS_COMPLEX` | `qwen2.5:3b` | Modelo para tareas complejas |
| `FE_MODELS_SIMPLE` | `gemma3:4b` | Modelo rápido |
| `FE_MODELS_REASONING` | `qwen2.5:3b` | Modelo de razonamiento |
| `FE_MODELS_SUMMARIZER` | `gemma3:4b` | Modelo de resúmenes |
| `FE_AGENT_NAME` | `fe` | Nombre del agente |
| `FE_AGENT_CONFIRM_TOOLS` | `true` | Confirmar herramientas |
| `FE_AGENT_BATCH_MODE` | `false` | Modo batch |
| `FE_AGENT_AUTO_SAVE` | `true` | Auto-guardar sesiones |
| `FE_AGENT_SUMMARIZE_EVERY_N_TURNS` | `4` | Resumen cada N turns |
| `FE_AGENT_MAX_HISTORY_TURNS` | `20` | Máximo histórico |
| `FE_AGENT_MAX_CONTEXT_TOKENS` | `8192` | Contexto máximo |
| `FE_TOOLS_BASH_TIMEOUT_SECS` | `120` | Timeout de comandos |
| `FE_TOOLS_MAX_OUTPUT_LINES` | `5000` | Líneas máximas de output |
| `FE_TOOLS_MAX_OUTPUT_BYTES` | `1048576` | Bytes máximos de output |
| `FE_TOOLS_ALLOW_DESTRUCTIVE` | `false` | Permitir destructivos |
| `FE_SHELL_GENERATE_COMPLETIONS` | `true` | Generar completions |
