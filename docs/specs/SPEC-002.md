---
spec_id: SPEC-002
title: Clasificador de intención (Acción vs Razonamiento)
version: 1.0.0
status: draft
author: adonis
created: 2026-09-08
related_specs: [SPEC-001, SPEC-003, SPEC-004, SPEC-005]
related_adrs: []
tags: [intent, router, action, reasoning]
---

# SPEC-002 — Clasificador de intención (Acción vs Razonamiento)

## Problema

`fe` actualmente resuelve las acciones no-coincidentes cayendo a la IA (con tools),
lo que permite que el LLM "decida" ejecutar comandos — arriesgado y contrario a la
regla de usar la IA solo para razonamiento. Falta una frontera explícita entre
**acción** (ejecutar un comando, sin IA) y **razonamiento** (pregunta/análisis, con IA).

## Contexto

- Usuario escribe prompts en lenguaje natural en la terminal.
- Hoy `router.is_simple_query()` clasifica solo simple vs complejo; no distingue acción vs razonamiento.
- Se quiere: acciones se ejecutan por programación (reglas), razonamiento usa IA.
- Ante ambigüedad, el comportamiento seguro es tratar como razonamiento (pregunta).

## Objetivo

El sistema hace X de forma binaria: **dado cualquier prompt, el clasificador devuelve `Action` o `Reasoning`**, y para ello implementa un módulo `IntentClassifier` en `src/agent/intent.rs`.

## Alcance

### Incluye
- Nuevo archivo `src/agent/intent.rs` con enum `Intent { Action, Reasoning }` y `IntentClassifier`.
- Lógica de precedencia documentada.
- Registro del módulo en `src/agent/mod.rs`.

### No incluye
- La ejecución de acciones (SPEC-003/004).
- El enrutado en `core.rs` (SPEC-005).

## Comportamiento

### Flujo principal
1. Se recibe un prompt.
2. Se comprueba si matchea una regla directa → `Action`.
3. Si no, se buscan verbos de acción sin marcador de razonamiento → `Action`.
4. Si existe marcador de razonamiento (qué es, por qué, explica, analiza...) → `Reasoning`.
5. Si es ambiguo → `Reasoning`.

### Flujos alternativos / Edge cases
- Prompt vacío → `Reasoning` (seguro).
- "analiza el rendimiento tras instalar X" → `Reasoning` (marcador de razonamiento sobre acción).
- "lista los archivos" → coincide regla directa → `Action`.

## Criterios de aceptación

- **AC-01**: Dado "lista los archivos", cuando se clasifica, entonces devuelve `Action`.
- **AC-02**: Dado "¿qué es SOLID?", cuando se clasifica, entonces devuelve `Reasoning`.
- **AC-03**: Dado "actualiza repositorios", cuando se clasifica, entonces devuelve `Action`.
- **AC-04**: Dado "analiza la arquitectura", cuando se clasifica, entonces devuelve `Reasoning`.
- **AC-05**: Dado un prompt vacío, cuando se clasifica, entonces devuelve `Reasoning`.
- **AC-06**: Dado "explica cómo funciona", cuando se clasifica, entonces devuelve `Reasoning`.

## Restricciones

- Técnicas: puro Rust, sin I/O en la clasificación (función pura, testeable).
- Seguridad: por defecto seguro → tratar ambigüedad como `Reasoning` (no ejecuta).

## Notas

- La precedencia: regla directa > verbo de acción > marcador de razonamiento > por defecto razonamiento.
- Listas de verbos y marcadores se extraen/amplían de `router.rs::is_simple_query` y `direct_executor.rs`.
- Se requieren tests unitarios por AC (trazabilidad 1:1).