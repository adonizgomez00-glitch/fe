# Especificación Técnica de Fe

> Versión: 0.1.1
> Fecha: 2026-07-11
> Autor: Adoniz

---

## 1. Resumen Ejecutivo

Fe es un agente CLI escrito en Rust que conecta modelos LLM locales (Ollama) con herramientas del sistema. Su propósito es asistir en administración de sistemas, desarrollo de software, arquitectura y generación de documentos — todo desde la terminal, con confirmación humana, sin depender de servicios cloud.

**Nombre**: Fe (símbolo químico del hierro y la "fe" que tiene la humanidad en la Inteligencia Artificial)
**Tagline**: "Agente de hierro para tu terminal"
**Tests**: 309 unitarios (0 failures)
**Provider**: Provider trait + OllamaProvider (extensible)
**Lenguaje**: Rust (edition 2021)
**Runtime**: Tokio (async)
**Dependencia externa**: Ollama corriendo en localhost:11434

---

## 2. Requerimientos Funcionales

### RF01: Ejecución de prompts
- El agente acepta un prompt como argumento posicional
- Ejemplo: `fe "actualiza repositorios"`

### RF02: Selección automática de modelo
- qwen2.5:3b → tareas complejas con herramientas
- gemma3:4b → tareas simples sin herramientas
- deepseek-r1:8b → razonamiento profundo con thinking
- `select()` devuelve `(String model, SkillKind kind)` para routing por dominio
- Override explícito con flags `--reasoning`, `--fast`

### RF03: Streaming de respuesta
- Muestra tokens en vivo durante generación
- Indicador visual (spinner) durante procesamiento

### RF04: Ejecución de herramientas
- bash, read, write, edit, glob, grep, task, webfetch, websearch
- Cada tool tiene schema JSON para el LLM
- Timeout configurable por tool
- Output truncado a límites configurables

### RF05: Confirmación híbrida
- Por defecto: tool-by-tool (confirmar cada comando)
- Con `--batch`: plan completo → confirmar una vez → ejecutar todo
- Opciones: `Y` (yes), `n` (no), `a` (yes a todos), `b` (abort)
- Detección de comandos destructivos: `sudo`, `rm -rf`, `dd`, etc. → confirmación extra

### RF06: Skills de opencode
- Carga skills desde `~/.config/opencode/skills/*/SKILL.md`
- Matching por nombre + keywords
- Inyección en system prompt del LLM

### RF07: Persistencia de sesiones
- Cada invocación crea una sesión con UUID
- Guarda en `~/.local/share/fe/sessions/<id>.md`
- Formato markdown estructurado por turnos
- Resumen automático cada 4 turnos vía gemma3:4b

### RF08: Shell completions
- Generación para bash, zsh, fish
- Subcomando `fe completions <shell>`

### RF09: Diagnóstico
- `fe doctor`: verifica Ollama, configuración, permisos, skills

---

## 3. Requerimientos No Funcionales

### RNF01: Rendimiento
- Tiempo de respuesta dominado por LLM (no por overhead del agente)
- Single binary, sin runtime externo
- Memoria: < 50MB sin contar el LLM

### RNF02: Seguridad
- No ejecutar acciones destructivas sin confirmación explícita
- `--destructive` flag para override
- No exponer variables de entorno ni secretos en logs
- Validar comandos antes de ejecutar

### RNF03: Portabilidad
- Linux como target principal
- Architectures: x86_64, aarch64
- Sin dependencias externas más allá de Ollama

### RNF04: Mantenibilidad
- Un archivo = una responsabilidad
- Máximo 300 líneas por archivo
- Documentación en cada módulo
- Tests unitarios + integración

---

## 4. Arquitectura

Ver `ARCHITECTURE.md` para diagramas detallados.

### 4.1 Flujo Principal

```
1. Input: fe <prompt>
2. CLI parsea prompt + flags (clap)
3. Router selecciona modelo: select() devuelve (String model, SkillKind kind)
4. Skills matcher carga skills: match_prompt() devuelve Vec<(&Skill, SkillKind)>
5. Context builder construye system prompt: [skills + history + instructions]
6. LLM stream via Provider trait:
   a. OllamaClient.chat_with_tools() → Provider.chat()
   b. Tokens en vivo + tool calls
7. Tool engine: por cada tool call:
   a. Confirmación (si no es batch)
   b. Ejecución con timeout
   c. Resultado al LLM
8. Cuando LLM responde sin tools:
   a. Muestra respuesta final
   b. Guarda sesión
   c. Summariza si aplica
```

### 4.2 Provider Trait (Hybrid Plan Fase 1)

Fe abstrae el backend LLM mediante un trait Provider, desacoplando el agente de la implementación HTTP concreta:

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn supports_tools(&self) -> bool;
    fn supports_reasoning(&self) -> bool;
    fn max_context(&self) -> u32;
    async fn chat(&self, model: &str, messages: &[Message], tools: &[Value], spinner_done: Option<&AtomicBool>) -> Result<ChatResult>;
    async fn list_models(&self) -> Result<Vec<String>>;
}
```

**OllamaProvider** implementa Provider usando HTTP a Ollama API (`/api/chat` con streaming + `/api/tags`). En futuras fases pueden añadirse otros proveedores (OpenAI, Anthropic, etc.).

### 4.3 SkillKind Enum

El router devuelve `(String model, SkillKind kind)` para permitir routing por dominio:

```rust
pub enum SkillKind {
    Linux, Docker, Python, Cobol, PowerBI,
    Kubernetes, PostgreSQL, Security, Git, Generic,
}
```

### 4.4 Tool Execution

Cada tool implementa:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;  // JSON Schema
    fn is_destructive(&self, params: &Params) -> bool;
    async fn execute(&self, params: Params) -> Result<ToolResult>;
}
```

### 4.5 Session Format

Cada sesión se guarda como:

```markdown
# Session: <timestamp>
**ID**: <uuid>
**Model**: qwen2.5:7b
**Skills**: C-sysadmin, D-git-workflow
**Directory**: /home/user

## Turn N
**Prompt**: <text>
### Tool: bash
**Command**: `apt update`
**Skill Tags**: [C-sysadmin:package-management]
**Output**: <truncado>
**Confirmed**: ✓
**Assistant**: <text>

## Summary (auto)
<skills-enhanced summary>
```

---

## 5. Interfaz con Ollama

### 5.1 API Endpoints

| Endpoint | Método | Propósito |
|----------|--------|-----------|
| `/api/tags` | GET | Listar modelos disponibles |
| `/api/chat` | POST | Chat con streaming + tools |
| `/api/generate` | POST | Generación simple |

### 5.2 Request /api/chat

```json
{
  "model": "qwen2.5:7b",
  "messages": [
    {"role": "system", "content": "system_prompt"},
    {"role": "user", "content": "actualiza repositorios"}
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "bash",
        "description": "Ejecuta comando bash",
        "parameters": {
          "type": "object",
          "properties": {
            "command": {"type": "string"},
            "description": {"type": "string"}
          },
          "required": ["command", "description"]
        }
      }
    }
  ],
  "stream": true,
  "options": {
    "num_ctx": 32000,
    "temperature": 0.3
  }
}
```

### 5.3 Response (streaming)

```json
{"message":{"role":"assistant","content":"Usando apt..."},"done":false}
{"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"bash","arguments":"{\"command\":\"apt update\",\"description\":\"Actualiza lista de paquetes\"}"}}]},"done":false}
{"message":{"role":"tool","content":"Hit:1 http://archive.ubuntu.com noble InRelease\\n..."},"done":false}
{"message":{"role":"assistant","content":"Repositorios actualizados. 15 paquetes upgradables."},"done":true}
```

---

## 6. Configuración

### 6.1 Archivo: `~/.config/fe/config.toml`

```toml
[ollama]
host = "http://localhost:11434"
timeout_secs = 300
max_retries = 3
keep_alive_secs = 300

[models]
complex = "qwen2.5:3b"
simple = "gemma3:4b"
reasoning = "deepseek-r1:8b"
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

### 6.2 Orden de precedencia
1. Valores por defecto en código
2. Archivo `config.toml`
3. Variables de entorno `FE_*` (ej: `FE_OLLAMA_HOST`)
4. Flags CLI

---

## 7. Manejo de Errores

| Error | Comportamiento |
|-------|---------------|
| Ollama no responde | Retry 3 veces con backoff exponencial, error claro |
| Tool timeout | Cancelar tool, reportar al LLM, continuar |
| Output muy grande | Truncar, reportar al LLM la cantidad truncada |
| Comando no encontrado | Reportar al LLM, sugerir instalación |
| Session corrupta | Ignorar, crear nueva, log de warning |
| Config inválida | Log de error + defaults |

---

## 8. Skills Integration

### 8.1 Formato SKILL.md

```markdown
---
name: C-sysadmin
description: Administración profesional de sistemas Linux
---

# Contenido de la skill...
```

### 8.2 Matching

- Extraer `name`, `description`, keywords del contenido
- Comparar con el prompt del usuario
- Si match score > threshold: inyectar skill completa en system prompt
- Skills sin match: excluidas para ahorrar tokens

### 8.3 Skills-Enhanced Summary

El resumen incluye `[skill:categoria:subcategoria]` para preservar contexto:

> Usuario ejecutó apt update/upgrade [skill:C-sysadmin:package-management].
> Git pull sincronizó 3 ramas [skill:D-git-workflow:sync].
> Sistema: Linux Mint 24.04, 55G/95G usado.
