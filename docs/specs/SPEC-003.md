---
spec_id: SPEC-003
title: Motor de reglas declarativas para acciones
version: 1.0.0
status: draft
author: adonis
created: 2026-09-08
related_specs: [SPEC-002, SPEC-004, SPEC-005]
related_adrs: []
tags: [commands, rules, regex, declarative]
---

# SPEC-003 — Motor de reglas declarativas para acciones

## Problema

El matching directo (`direct_executor.rs`) usa regex hardcodeadas en Rust, se escala
mal (hay que recompilar para añadir una frase) y no permite que el usuario añada
comandos sin tocar código. Se necesita una capa de **reglas declarativas** configurable.

## Contexto

- Usuario quiere decir "haz X" y que `fe` ejecute el comando correcto, sin IA.
- Ya existe `direct_executor` con ~12 intenciones hardcodeadas (ls, pwd, mkdir, rm, cat, buscar, crear app, etc.).
- Se quiere un `DirectMatcher` que mapee prompts a comandos vía plantillas con parámetros y `sh_quote()`.
- Se usará el fichero `~/.config/fe/commands.toml` (formato TOML, ya soportado por el proyecto), además de reglas built-in.

## Objetivo

El sistema hace X de forma binaria: **dado cualquier prompt, si existe una regla declarativa que coincida, devuelve un `DirectMatch { description, command, is_destructive }`** sin invocar IA.

## Alcance

### Incluye
- Nuevo archivo `src/agent/direct_matcher.rs` con `CommandRule` y `DirectMatcher`.
- Reglas built-in (replican las ~12 actuales de `direct_executor.rs`).
- Carga de reglas desde `~/.config/fe/commands.toml` opcional.
- Sustitución de parámetros `{param}` con `sh_quote()`.

### No incluye
- Fuzzy matching (SPEC-004).
- El flujo de confirmación/ejecución (SPEC-005).
- Migración de `direct_executor.rs` (se mantiene como fallback o se integra).

## Comportamiento

### Flujo principal
1. `DirectMatcher` se construye con reglas built-in + reglas de fichero.
2. Para un prompt, evalúa reglas en orden definido.
3. Si coincide, extrae capturas de regex y sustituye `{param}` en la plantilla de comando con `sh_quote()`.
4. Devuelve `DirectMatch`.

### Flujos alternativos / Edge cases
- Email/string con comillas o espacios → `sh_quote` lo protege.
- Prompt no coincide con ninguna regla → `None` (deriva a fuzzy).
- Regla destructiva marcada con `destructive: true`.

## Criterios de aceptación

- **AC-01**: Dado "lista los archivos", cuando se matchea, entonces devuelve `DirectMatch` no destructivo con comando que contiene `ls`.
- **AC-02**: Dado "crea un directorio llamado prueba", cuando se matchea, entonces devuelve comando con `mkdir prueba`.
- **AC-03**: Dado "elimina el archivo hola.py", cuando se matchea, entonces devuelve `DirectMatch.is_destructive == true` y comando con `rm`.
- **AC-04**: Dado "crea un archivo 'mi doc.txt' con hola", cuando se matchea, entonces el nombre se escapa correctamente (sh_quote).
- **AC-05**: Dado un prompt sin regla, cuando se matchea, entonces devuelve `None`.
- **AC-06**: Dado un fichero `commands.toml` con una regla nueva, cuando se matchea su prompt, entonces se resuelve la regla del fichero.

## Restricciones

- Técnicas: sin I/O en el matcher puro; carga de fichero separada.
- Seguridad: sustitución siempre `sh_quote`; destructivas marcadas para confirmar después.

## Notas

- Tabla de reglas: `CommandRule { intent, pattern: Regex, command: plantilla, destructive }`.
- `config.toml.example` documentará la sección `[[commands]]` para el fichero de reglas.
- Trazabilidad: cada AC → test en `src/agent/direct_matcher.rs`.