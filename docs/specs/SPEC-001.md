---
spec_id: SPEC-001
title: Soporte múltiples proveedores de modelos (local + cloud)
version: 1.1.0
status: draft
author: adonis
created: 2026-09-08
updated: 2026-09-08
related_specs: [SPEC-002, SPEC-005]
related_adrs: []
tags: [config, models, provider, ollama, cloud, provider]
---

# SPEC-001 — Soporte múltiples proveedores de modelos (local + cloud)

## Problema

`fe` opera solo con Ollama local y no puede elegir entre proveedores de diferentes
tipos (local vs cloud) cuando el usuario desea eso. Además, la config por defecto
apunta a modelos que no existen localmente (`qwen2.5:3b`, `gemma3:4b`) creando
una experiencia de fallo. El usuario necesita poder:
1. Usar modelo local (Ollama) como opción por defecto y preferida.
2. Configurar backends cloud (OpenAI, Anthropic, etc.) como alternativa o fallback.
3. Seleccionar qué proveedor usar por slot de modelo (complex, simple, reasoning, summarizer).

## Contexto

- `fe` usa un `trait Provider` (src/agent/provider.rs) con implementaciones:
  - `OllamaProvider`: cliente HTTP hacia Ollama local (`http://localhost:11434`).
- La arquitectura del trait `Provider` ya está preparada para extenderse a otros backends.
- Config se resuelve por precedencia: defaults < `config.toml` < env vars < flags.
- Modelos locales instalados actualmente: `MobiusDevelopment/Bonsai-27B`, `qwen2.5-coder:7b`, `qwen2.5-coder:3b`, `qwen2.5-coder:1.5b`.
- Cargo existente: `reqwest`, `serde`, `anyhow`, `tokio` — suficientes para añadir clientes cloud.

## Objetivo

El sistema hace X de forma binaria: **permite al usuario configurar múltiples proveedores (local Ollama + cloud providers) y selecciona cuál usar por medio de la configuración por slot**, de modo que:
- Por defecto usa Ollama local (qwen2.5-coder:3b recomendado).
- Cuando el usuario configura un cloud provider, puede indicar qué modelo usar y `fe` lo usa.
- Si un proveedor falla, puede configurarse un fallback al siguiente.

## Alcance

### Incluye
- Extender la configuración de modelos para incluir selección de proveedor por slot.
- Soportar al menos: Ollama (local) y un backend cloud (ej. OpenAI-compatible o Anthropic).
- Permitir fallback entre proveedores configurados.
- Actualizar `config.toml.example` para documentar la nueva sección de proveedores.
- Mantener `qwen2.5-coder:3b` como modelo por defecto local recomendado.

### No incluye
- Añadir todos los proveedores cloud existentes (solo al menos uno de referencia más allá de Ollama).
- Cambiar el comportamiento de razonamiento (sección SPEC-002).
- UI dedicada para selector de proveedores (solo config + doc).

## Comportamiento

### Flujo principal (selección de modelo)
1. `fe` carga configuración (config.toml + env vars).
2. Para cada slot de modelo (complex, simple, reasoning, summarizer):
   - Se lee la configuración del proveedor asignado a ese slot.
   - Si el proveedor configurado no está disponible → fallback al siguiente configurado o error.
3. Se usa el modelo del proveedor seleccionado para la petición de razonamiento.

### Flujo alternativo (proveedor cloud)
- Ejemplo: usuario configura `complex = { provider = "openai", model = "gpt-4o-mini" }` en config.toml.
- `fe` detecta el proveedor cloud y usa el cliente correspondiente.
- Si OpenAI devuelve error, puede configurarse fallback a Ollama local.

### Flujos alternativos / Edge cases
- Ollama no está corriendo → `fe doctor` reporta; si no hay fallback, razonamiento falla.
- Cloud provider sin API key → error claro en inicialización ("falta API key para proveedor X").
- Ambos proveedores configurados pero ninguno disponible → error claro con opciones indicadas.

## Criterios de aceptación

- **AC-01**: Dado un `config.toml` con solo Ollama local, cuando se ejecuta `fe`, entonces usa el modelo Ollama configurado (por defecto qwen2.5-coder:3b).
- **AC-02**: Dado `config.toml` configurando un proveedor cloud (ej. OpenAI) como fallback del slot complex, cuando Ollama local falla o no está disponible, entonces `fe` usa el proveedor cloud configurado.
- **AC-03**: Dado que `qwen2.5-coder:3b` está instalado en Ollama local y es el modelo configurado, cuando `fe` ejecuta razonamiento, entonces usa ese modelo local.
- **AC-04**: Dado que no hay API key configurada para el proveedor cloud, cuando `fe` intenta inicializar ese proveedor, entonces falla con mensaje claro de error indicando la clave que falta.
- **AC-05**: Dado el setup actual sin config real, cuando `fe` carga config, entonces detecta que el modelo por defecto no existe y usa `qwen2.5-coder:3b` (o falla con mensaje claro si no está instalado).
- **AC-06**: Dado `router.supports_tools("qwen2.5-coder:3b")`, entonces devuelve `true`.

## Restricciones

- Técnicas:
  - Solo un backend cloud implementado en esta versión (ir a N proveedores es backlog).
  - La implementación cloud usa el mismo `trait Provider` para mantener consistencia.
- Negocio: sin backend externo obligatorio; Ollama por defecto es suficiente.
- Seguridad:
  - Las API keys de cloud se manejan como secretos (env vars o fichero config con permisos adecuados).
  - No se loguean las API keys en trazas.
  - Fallos de autenticación se comunican de forma segura (sin exponer credenciales).

## Notas

- `config.toml.example` documentará la sección `[providers]` con ejemplos de uso de Ollama + cloud.
- El modelo por defecto local sigue siendo `qwen2.5-coder:3b` (ya instalado).
- Fallback entre proveedores es configurable, no automático (el usuario decide qué fallback usar).
- Si en el futuro se quieren más backends cloud, se añaden como nuevas implementaciones de `Provider`.
- `qwen2.5-coder:3b` es el modelo local recomendado por defecto.