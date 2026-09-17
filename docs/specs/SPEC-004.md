---
spec_id: SPEC-004
title: Fuzzy matching para intenciones de acción
version: 1.0.0
status: draft
author: adonis
created: 2026-09-08
related_specs: [SPEC-002, SPEC-003, SPEC-005]
related_adrs: []
tags: [fuzzy, similar, fallback, action]
---

# SPEC-004 — Fuzzy matching para intenciones de acción

## Problema

El motor de reglas declarativas (SPEC-003) solo resuelve acciones exactas por regex.
Cuando un usuario dice algo como "borrame to" o "muestra lo que hay" y no hay
regla declarativa que coincida, el sistema debe resolver la intención sin invocar IA.
Hoy no hay mecanismo de fuzzy: el prompt cae a "no hay regla" y queda sin respuesta.

## Contexto

- El crate `similar` (simple fuzzy matching) ya está en `Cargo.toml`.
- La intención del usuario puede ser una variante lingüística de una acción conocida
  ("borrame to" → "elimina el archivo to"; "muestra lo que hay" → "lista archivos").
- Umbral conservador: >0.85 de similitud para actuar.
- Solo se aplica cuando el clasificador de intención (SPEC-002) ha etiquetado como `Action`.

## Objetivo

El sistema hace X de forma binaria: **si no hay regla declarativa para un prompt clasificado como `Action`, el fuzzy matching busca la intención más similar en un diccionario de intenciones conocidas y devuelve el `DirectMatch` correspondiente** (o `None` si la similitud no supera umbral).

## Alcance

### Incluye
- Extensión del `DirectMatcher` de SPEC-003 con función fuzzy.
- Diccionario de intenciones conocidas con score mínimo.
- Integración: fuzzy se usa después de reglas declarativas y antes de declarar "sin regla".
- Regla de seguridad: fuzzy solo ejecuta acciones no destructivas sin confirmar.
- Acciones destructivas encontradas por fuzzy → siempre piden confirmación extra.

### No incluye
- Incrementar umbral dinámicamente (v1.0.0 es umbral fijo).
- Aprendizaje automático de nuevas intenciones (backlog).

## Comportamiento

### Flujo principal
1. Prompt clasificado como `Action`.
2. Se busca regla declarativa (SPEC-003). Si existe → usar ese DirectMatch.
3. Si no, se calcula similaridad del prompt con cada intención conocida (usando `similar`).
4. Si la similitud máxima > 0.85 y la intención está registrada → se usa ese DirectMatch.
5. Si la acción encontrada es destructiva → se marca para confirmación (igual que cualquier acción destructiva).
6. Si no supera umbral o no hay intención → devuelve `None` (el sistema avisa "no tengo regla para esto" y NO invoca IA).

### Flujos alternativos / Edge cases
- "borrame to" → fuzzy → "elimina el archivo to" → destrucción → confirma.
- "muestra lo que hay" con score 0.60 → no supera umbral → devuelve `None`, sin IA.
- Varias intenciones con score alto → usa la máxima; si hay empate o muy cerca, preferiblemente la primera conocida (stable).

## Criterios de aceptación

- **AC-01**: Dado "borrame to", cuando no hay regla y el prompt es clasificado como `Action`, entonces el fuzzy matching devuelve el `DirectMatch` de "elimina el archivo to" (destructivo).
- **AC-02**: Dado un prompt sin regla y con similitud < 0.85 respecto a cualquier intención, cuando se ejecuta fuzzy, entonces devuelve `None`.
- **AC-03**: Dado "muestra los archivos" (sin regla declarativa exacta en ese momento hipotético), cuando se ejecuta fuzzy, entonces encuentra la intención "lista los archivos" y devuelve el comando asociado.
- **AC-04**: Dado "haz ls", cuando se ejecuta fuzzy, entonces devuelve el comando `ls` (no destructivo, sin confirmación adicional).

## Restricciones

- Técnicas: fuzzy solo aplica dentro del `DirectMatcher` tras reglas declarativas.
- Seguridad: acciones destructivas → confirmación (especificación de seguridad en SPEC-005).

## Notas

- `similar` se usa con strings normalizados (lowercase, sin stop words opcionales).
- Umbral calibrado en 0.85 para v1.0.0; a calibrado posterior puede variar.
- Diccionario de intenciones: se extrae de las reglas built-in (SPEC-003); no requiere mantenimiento manual si se mantiene sincronizado.
- Este fuzzy es red de seguridad, no sustituto de reglas declarativas.