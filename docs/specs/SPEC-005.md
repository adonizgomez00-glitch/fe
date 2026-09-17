---
spec_id: SPEC-005
title: Nuevo flujo en core.rs -- Acción (sin IA) vs Razonamiento (con IA)
version: 1.1.0
status: draft
author: adonis
created: 2026-09-08
updated: 2026-09-08
related_specs: [SPEC-001, SPEC-002, SPEC-003, SPEC-004]
related_adrs: []
tags: [core, flow, action, reasoning, integration]

---

# SPEC-005 -- Nuevo flujo en core.rs: Acción (sin IA) vs Razonamiento (con IA)

## Problema

El flujo actual en `core.rs::run()` delega acciones no-resueltas a la IA con tools,
permitiendo que el LLM decida ejecutar comandos. Esto viola el principio de que
**la IA solo debe usarse para razonamiento**, no para decidir acciones.

Necesitamos un flujo donde:
- **Acciones** se ejecutan exclusivamente por programación (reglas + fuzzy), sin IA.
- **Razonamiento** usa la IA con tools (pero no para decidir acciones).

## Contexto

- `core.rs::run()` es el punto de entrada principal del agente al ejecutar un prompt.
- `IntentClassifier` (SPEC-002) clasifica el prompt como `Action` o `Reasoning`.
  - La clasificación es DOLUE antes de entrar a cualquier flujo de ejecución.
- `DirectMatcher` (SPEC-003) maneja las reglas declarativas para acciones.
- `FuzzyMatcher` (SPEC-004) es una extensión de `DirectMatcher` que maneja detección por similaridad.
- El `Provider` (SPEC-001) soporta múltiples proveedores (local/cloud).

## Objetivo

El sistema hace X de forma binaria: **cada petición del usuario se clasifica primero en `IntentClassifier`, y según la intención, se ejecuta por programación (acción) o por IA (razonamiento), sin que el usuario necesite usar ningún comando o flag especial**.

ÚNICA SALIDA: `Action` → ejecución programática (reglas + fuzzy), `Reasoning` → IA.

## Alcance

### Incluye
- Modificar `core.rs::run()` para usar el `IntentClassifier` como única entrada de decisión.
- Vincular al `IntentClassifier` el `DirectMatcher` (incluyendo su extensión fuzzy).
- Para acciones: ejecutar sin pasar por IA, con confirmación si destructivo.
- Para razonamiento: usar el proveedor configurado (SPEC-001) con el modelo adecuado.
- Mostrar al usuario qué tipo de procesamiento se usa (feedback transparente).
- Manejar fallback de modelos en razonamiento (SPEC-001 AC-02).
- Tests de integración para validar que acciones no invocan IA.

### No incluye
- Implementación del `IntentClassifier` (es SPEC-002, se asume hecho).
- Implementación del motor de reglas (SPEC-003).
- Implementación del fuzzy (SPEC-004).
- Multi-paso complejo donde una acción genere otra acción (backlog).

## Comportamiento

### Flujo principal

```mermaid
graph TD
    A[Usuario escribe prompt] --> B{IntentClassifier}
    B -->|Action| C[Acción sin IA]
    B -->|Reasoning| D[Razonamiento con IA]
    C --> C1[DirectMatcher: reglas declarativas]
    C1 -->|coincide| C3[Ejecutar directamente]
    C1 -->|no coincide| C2[FuzzyMatcher: similaridad >0.85]
    C2 -->|coincide| C3
    C2 -->|no coincide| C4[Error: "No tengo regla"]
    C3 --> C5{Si destructivo}
    C5 -->|sí| C6[Confirmar y ejecutar]
    C5 -->|no| C7[Ejecutar sin confirmación]
    D --> D1[Usar proveedor/modelo configurado]
    D1 --> D2{Éxito}
    D2 -->|sí| D3[Responder]
    D2 -->|no| D4[Fallback según SPEC-001]
    D3 --> D5[Mostrar feedback]
    C6 --> C7
    C7 --> C8[Mostrar feedback]

    style A fill:#f9f9f9
    style C fill:#f9f9f9
    style D fill:#f9f9f9
```

**FlujoAcción sin IA (secuencia):**
1. El `IntentClassifier` clasifica el prompt como `Action`.
2. `DirectMatcher` intenta regla declarativa primero (SPEC-003). Si coincide → ejecutar.
3. Si no coincide con regla declarativa, `DirectMatcher` (con fuzzy) busca por similaridad (>0.85) (SPEC-004). Si acierta → ejecutar.
4. Si acción encontrada es destructiva → pedir confirmación `[y/N]`.
5. Si acción segura → ejecutar sin confirmación.
6. Mostrar feedback: "Acción: [descripción]. Comando: [comando]."
7. Si no hay coincidencia ni en reglas ni fuzzy → error "No tengo regla para esto, no puedo ejecutar" y **NO invocar IA**.

**Flujo Razonamiento con IA (secuencia):**
1. El `IntentClassifier` clasifica el prompt como `Reasoning`.
2. Se usa el proveedor/modelo configurado (SPEC-001, por defecto qwen2.5-coder:3b).
3. Si proveedor falla, se aplica fallback configurado (SPEC-001 AC-02).
4. Si modelo soporta tools → loop ReAct con skills + histórico.
5. Si no soporta tools → chat simple.
6. Responder al usuario.
7. Mostrar feedback: "Razonamiento usando [modelo]."

### Flujos alternativos / Edge cases

- **Prompt ambiguo o no claramente action/reasoning**: `IntentClassifier` decide `Reasoning` por defecto (seguro) → NO ejecuta nada.
- **Fuzzy encuentra acción destructiva**: confirma siempre (SPEC-004).
- **Proveedor cloud sin API key**: error claro (SPEC-001).
- **Ollama no disponible**: si hay fallback configurado, usa fallback; si no, error claro.
- **"¿Qué hay en mi carpeta?"**: Preguntas sobre estado del sistema pueden ser `Action` (listar, cat) **SOLO SI** matchean regla declarativa o fuzzy. Si no, `Reasoning`.

## Criterios de aceptación

- **AC-01**: Dado un prompt clasificado como `Action`, cuando se ejecuta en `core.rs`, entonces **no se invoca ningún LLM** (se ejecuta por programación).
- **AC-02**: Dado un prompt clasificado como `Action` que devuelve un `DirectMatch` destructivo, cuando se ejecuta, entonces se pide confirmación `[y/N]` antes de ejecutar.
- **AC-03**: Dado un prompt clasificado como `Action` que devuelve un `DirectMatch` seguro, cuando se ejecuta, entonces se ejecuta sin confirmación.
- **AC-04**: Dado un prompt clasificado como `Reasoning`, cuando se ejecuta, entonces se usa el modelo configurado (SPEC-001, por defecto qwen2.5-coder:3b) para responder.
- **AC-05**: Dado un prompt que no coincide con ninguna regla ni fuzzy, cuando se clasifica como `Action`, entonces se avisa "no tengo regla para esto" y **no se invoca IA**.
- **AC-06**: Dado cualquier prompt, cuando se ejecuta, entonces se muestra al usuario qué tipo de procesamiento se usó (acción con comando/show, o razonamiento con modelo).
- **AC-07**: Dado que un proveedor en razonamiento falla, cuando se ejecuta, entonces se aplica fallback al siguiente proveedor configurado (SPEC-001). Si no hay fallback, error claro.
- **AC-08**: Dado un prompt ambiguo, cuando se clasifica, entonces `IntentClassifier` devuelve `Reasoning` (safe default).

## UX -- Detección automática sin comandos

### Principio UX

- El usuario **nunca** debe usar ningún comando, flag o sintaxis especial para indicar si quiere una acción o razonamiento.
- La detección se basa **únicamente en el contenido semántico del prompt**, no en cómo lo formule.
- Esto implica:
  - El `IntentClassifier` debe ser **robusto a variaciones lingüísticas** (ej: "crea un directorio", "haz un directorio", "quiero crear carpeta" → todos `Action`).
  - No debe depender de palabras clave exactas (eso es responsabilidad del motor de reglas/fuzzy).
  - Si el prompt puede interpretarse tanto como acción como razonamiento, se toma la interpretación más segura (generalmente `Reasoning` si hay ambigüedad).

### Ejemplos de detección automática sin comandos

| Prompt del usuario | Intención detectada | Comentario |
|-------------------|---------------------|------------|
| "actualiza repositorios" | `Action` | Acción reconocible (actualizar = comando) |
| "haz ls" | `Action` | Acción reconocible (ls = comando) |
| "crea un archivo llamado hola.txt" | `Action` | Acción reconocible (crear archivo = comando) |
| "muestra los archivos" | `Action` | Acción reconocible (mostrar = comando) |
| "¿qué es SOLID?" | `Reasoning` | Pregunta de conocimiento (razonamiento) |
| "explica el patrón factory" | `Reasoning` | Explicación (razonamiento) |
| "analiza este código" | `Reasoning` | Análisis (razonamiento, puede usar tools para leer) |
| "quiero saber cómo funciona la memoria" | `Reasoning` | Consulta de conocimiento (razonamiento) |
| "puedes crearme un directorio?" | `Action` | Acción implícita con cortesía (igual que "crea un directorio") |

### Requisitos UX derivados

- **AC-UX-01**: El usuario NO debe usar ningún flag (`--action`, `--reasoning`, etc.) para indicar intención; la detección es automática.
- **AC-UX-02**: El `IntentClassifier` (código) debe manejar variaciones lingüísticas del mismo concepto de acción/razonamiento sin que el usuario tenga que cambiar su forma de preguntar.
- **AC-UX-03**: Si el prompt es ambiguo, la interpretación por defecto debe ser segura (`Reasoning` si no es claramente acción destructiva).
- **AC-UX-04**: El usuario no debe tener que saber si su prompt será tratado como acción o razonamiento; `fe` decide y lo informa (AC-06).

## Restricciones

- Técnicas:
  - El flujo debe ser completamente testable (separar lógica de ejecución real de la decisión).
  - La clasificación (`IntentClassifier`) debe ser pura (sin I/O) para ser testeable (SPEC-002).
- Seguridad:
  - **Nunca invocar IA para acciones** (AC-01, AC-05).
  - Acciones destructivas 찾으면 항상 확인 (SPEC-004).
- UX:
  - Feedback sobre qué se hizo es obligatorio (AC-06); detección automática sin comandos especiales (AC-UX-01 a AC-UX-04).

## Notas

- `core.rs::run()` quedaría así en pseudocódigo:

```rust
async fn run(&mut self, prompt: &str) -> Result<()> {
    // 1. Clasificar intención (SIEMPRE primero, decide la rama)
    let intent = self.intent_classifier.classify(prompt);

    match intent {
        Intent::Action => {
            // 2a. Intentar reglas declarativas
            if let Some(match_result) = self.direct_matcher.match_prompt(prompt) {
                return self.execute_action(&match_result).await;
            }
            // 3a. Intentar fuzzy (parte del DirectMatcher)
            if let Some(match_result) = self.direct_matcher.fuzzy_match_prompt(prompt) {
                return self.execute_action(&match_result).await;
            }
            // 4a. No se encontró
            println!("No tengo regla para esto, no puedo ejecutar.");
            return Err(anyhow::anyhow!("acción no reconocida"));
        }
        Intent::Reasoning => {
            // 2b. Usar IA con proveedor configurado
            return self.execute_reasoning(prompt).await;
        }
    }
}
```

- `execute_action`: se ocupa de mostrar feedback, confirmar si destructivo, ejecutar el comando, retornar resultado.
- `execute_reasoning`: se ocupa de mostrar qué modelo usa, ejecutar con provider (incluyendo fallback), retornar resultado.
- La detección automática sin comandos es requisito UX explícito (non-negotiable).
- `DirectMatcher.fuzzy_match_prompt()` puede ser un método separado o un flag interno, pero conceptualmente el fuzzy es parte de la mism `DirectMatcher` (SPEC-004).

## SDD Checklist

Verficación según plantilla canónica SDD (docs/SDD.md §6.4):

| Check | Estado |
|-------|--------|
| Objetivo falsable | ✅ "cada prompt se clasifica y se ejecuta Action o Reasoning" |
| Contexto acotado | ✅ core.rs::run(), con componentes ya especificados |
| Exclusiones explícitas (Incluye/No incluye) | ✅ No incluye implementación de Intents, reglas, fuzzy ni multi-paso |
| ACs binarios verificables (Given/When/Then) | ✅ 8 ACs + 4 AC-UX |
| Trazabilidad indicada | ✅ linked: SPEC-001, SPEC-002, SPEC-003, SPEC-004 |
| Restricciones técnicas/negocio/seguridad | ✅ técnicas (testable, pura), negocio (IA solo raz.), seguridad (no IA para action) |
| Notas con decisiones y referencias | ✅ pseudocódigo, decisiones UX explícitas |

