# Fe — Agente CLI Local v0.1.0

[![CI](https://github.com/adonizgomez00-glitch/fe/actions/workflows/ci.yml/badge.svg)](https://github.com/adonizgomez00-glitch/fe/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/adonizgomez00-glitch/fe)](https://github.com/adonizgomez00-glitch/fe/releases/latest)
![Lang](https://img.shields.io/badge/lang-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
![Tests](https://img.shields.io/badge/tests-348_pass-green)

**Fe** (símbolo químico del hierro) es un agente CLI local que conecta tu terminal con modelos LLM vía Ollama. Diseñado para administración de sistemas, desarrollo de software, arquitectura y generación de documentos — todo desde la terminal, sin depender de servicios externos.

```bash
fe "actualiza repositorios"                    # Tarea de sistema
fe --batch "apt update && apt upgrade"         # Modo batch
fe "crea una app python que sume y reste"      # Genera proyecto completo
fe "lista archivos"                            # Ejecución directa (sin LLM)
fe doctor                                      # Diagnóstico del sistema
```

---

## Características

- **🚀 Ejecución directa**: Comandos comunes (ls, pwd, mkdir) se ejecutan instantáneamente sin LLM
- **🔧 10 herramientas nativas**: bash, read, write, edit, glob, grep, task, webfetch, websearch, mkdir
- **🎯 Router inteligente**: Selecciona automáticamente el modelo según la tarea; devuelve `SkillKind` para routing por dominio
- **📦 Template system**: Genera proyectos multi-archivo en Python, Node.js, Rust y Bash
- **🔌 MCP (Model Context Protocol)**: Conecta servidores MCP, tools disponibles como `mcp:{server}:{tool}`
- **📦 Plugins**: Sistema de plugins con runtime subprocess (JSON-RPC), tools como `plugin:{name}:{tool}`
- **📋 Sesiones persistentes**: Cada conversación se guarda en markdown con resúmenes automáticos
- **🧠 18 skills opencode**: Auto-detección e inyección de skills en el system prompt
- **✅ Confirmación híbrida**: Tool-by-tool o `--batch`, detección de comandos destructivos
- **📊 Streaming en vivo**: Muestra tokens mientras el modelo genera
- **🔍 Diagnóstico**: `fe doctor` verifica Ollama, configuración y skills
- **🔌 Provider trait**: Arquitectura desacoplada de LLM (Provider + OllamaProvider), extensible a otros backends

---

## Instalación Rápida

```bash
# Requisito: Ollama corriendo en localhost:11434
ollama pull qwen2.5:3b
ollama pull gemma3:4b

# Instalar Fe
git clone https://github.com/adonizgomez00-glitch/fe.git fe
cd fe
./install.sh
```

Para más opciones, ver [docs/user-guide.md](docs/user-guide.md).

---

## Modelos Soportados

| Modelo | Rol | Contexto | Tools |
|--------|-----|----------|:-----:|
| qwen2.5:3b | Tareas complejas + razonamiento | 32K | ✓ |
| gemma3:4b | Rápido / resúmenes | 32K | ✗ |
| deepseek-r1:8b | Backup (razonamiento) | 128K | thinking |

Configurables vía `config.toml` o variables de entorno `FE_MODELS_*`.

---

## Comandos

```bash
# Uso básico
fe "actualiza repositorios"
fe --batch "apt update && apt upgrade -y"
fe --reasoning "analiza arquitectura del proyecto"
fe --fast "qué es SOLID"
fe --dry-run "crea un directorio"

# Gestión de sesiones
fe session list
fe session show <id>
fe session export <id> <output>
fe session clean

# Configuración
fe config show
fe config edit

# MCP (Model Context Protocol)
fe mcp discover
fe mcp servers
fe mcp call mcp:server:tool '{}'
fe mcp resources
fe mcp prompts
fe mcp read server uri
fe mcp get-prompt server name '{}'

# Plugins
fe plugins list
fe plugins reload
fe plugin run <name> <cmd> [args]

# Daemon Mode
fe daemon start
fe daemon status
fe daemon stop

# Shell completions
fe completions bash
fe completions zsh
fe completions fish

# Diagnóstico
fe doctor
```

---

## Flags Globales

| Flag | Descripción |
|------|-------------|
| `--batch` | Una confirmación para todo el plan |
| `--reasoning` | Usa modelo de razonamiento (qwen2.5:3b) |
| `--fast` | Usa modelo rápido (gemma3:4b, sin tools) |
| `--dry-run` | Muestra plan sin ejecutar |
| `--destructive` | Omite confirmación extra para acciones destructivas |

---

## CI/CD

El proyecto usa **GitHub Actions** con dos workflows:

| Workflow | Disparador | Qué hace |
|----------|-----------|----------|
| [`ci.yml`](.github/workflows/ci.yml) | Push a `main` y Pull Requests | `cargo build` + `cargo test` + build release + smoke test del binario |
| [`release.yml`](.github/workflows/release.yml) | Push de tag `v*` | Tests + `./release.sh` + verificación de integridad + publica GitHub Release con los 3 artefactos |

### Publicar una nueva versión

```bash
# 1. Actualiza VERSION en Cargo.toml, release.sh e install.sh
# 2. Commit y tag
git commit -am "chore: bump version a v0.2.0"
git tag v0.2.0
git push origin main --tags
```

El workflow `release.yml` empaqueta, verifica checksums y publica el release automáticamente.

> ⚠️ **Importante**: `release.sh` tiene la `VERSION` hardcodeada. El workflow valida que el
> paquete generado coincida con el tag y **falla con un mensaje claro** si no se actualizó.

---

## Documentación

| Documento | Descripción |
|-----------|-------------|
| [docs/user-guide.md](docs/user-guide.md) | Guía de usuario completa |
| [docs/configuration.md](docs/configuration.md) | Referencia de configuración |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Arquitectura del proyecto |
| [AGENT.md](AGENT.md) | Comportamiento y límites del agente |
| [docs/SAQI.md](docs/SAQI.md) | Metodología SAQI (Skills Agentic Quality Iteration) |
| [docs/SDD.md](docs/SDD.md) | Metodología SDD (Spec-Driven Development) |
| [SPECIFICATION.md](SPECIFICATION.md) | Especificación técnica |
| [ROADMAP.md](ROADMAP.md) | Hoja de ruta |
| [CHANGELOG.md](CHANGELOG.md) | Historial de cambios |

---

## Estado del Proyecto

- **348 tests** — todos pasando (unitarios + integración E2E), 0 warnings en `cargo build --release`
- **Hybrid Plan Fases 1-4 completadas**: Provider trait + OllamaProvider, Router con SkillKind, Skills Reorg (skills organizados por dominio), Core Services (Tools/Providers/Cache/Security traits + DI), y E2E de integración
- **Fases 1-5 completadas**: Foundation, Tools Engine, Skills + Sessions, Summarization + Polish, MCP + Plugins + Daemon
- **Checkpoint system**: Context monitoring >80%, auto-pause, save/restore session
- **Release v0.1.0**: Build release validado, ejecución directa, MCP/plugins/daemon verificados

---

## Licencia

MIT
