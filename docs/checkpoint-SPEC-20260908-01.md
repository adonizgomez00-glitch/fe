---
checkpoint_id: CHECKPOINT-SPEC-20260908-01
fecha: 2026-09-08
autor: adonis
estado: En progreso
siguiente: SPEC-004 (fuzzy matching)

---

# Checkpoint — Implementación SPEC-002 y SPEC-003

## Estado actual

| Especificación | Estado | Archivo | Pruebas | Notas |
|----------------|--------|---------|---------|-------|
| SPEC-001 | Pendiente documentación | docs/specs/SPEC-001.md | — | Soporte cloud + local — solo docs |
| SPEC-002 | ✅ Completo | docs/specs/SPEC-002.md + src/agent/intent.rs | 4 tests (AC-01 a AC-04) | Clasificador de intención implementado |
| SPEC-003 | ✅ Completo | docs/specs/SPEC-003.md + src/agent/direct_matcher.rs | 18 tests unitarios | Motor de reglas declarativas implementado |
| SPEC-004 | Pendiente | docs/specs/SPEC-004.md | — | Fuzzy matching pendiente |
| SPEC-005 | Pendiente | docs/specs/SPEC-005.md | — | Flujo en core.rs pendiente |

## Archivos modificados/creados

```
src/agent/
├── mod.rs                  ← añadido: pub mod direct_matcher, pub mod intent
├── intent.rs               ← NUEVO: clasificador de intención (SPEC-002)
├── direct_matcher.rs       ← NUEVO: motor de reglas declarativas (SPEC-003)
├── direct_executor.rs      ← sin cambios (fallback actual)
└── ...

docs/specs/
├── SPEC-001.md             ← documentado (soporte cloud + local)
├── SPEC-002.md             ← documentado (clasificador intención)
├── SPEC-003.md             ← documentado (motor reglas declarativas)
├── SPEC-004.md             ← documentado (fuzzy matching)
├── SPEC-005.md             ← documentado (flujo core.rs)
└── checkpoint-SPEC-20260908-01.md  ← ESTE ARCHIVO
```

## Implementación SPEC-002 ✅

### Clasificador de intención (`src/agent/intent.rs`)

**Estructuras:**
- `Intent` enum: `Action` / `Reasoning`
- `IntentClassifier` con `new(config)` y `classify(prompt) -> Intent`

**Lógica de precedencia (documentada en SPEC-005):**
1. Verifica marcadores de razonamiento (`what is`, `explain`, `why`, etc.)
2. Verifica marcadores de acción (`create`, `delete`, `install`, etc.)
3. Si hay ambigüedad o vacío → `Reasoning` (safe default)

**Tests creados (4):**
- `test_ac01_list_files` — "lista los archivos" → Action
- `test_ac02_what_is_solid` — "¿qué es SOLID?" → Reasoning
- `test_ac03_update_repos` — "actualiza repositorios" → Action
- `test_ac04_analyze_architecture` — "analiza la arquitectura" → Reasoning

### Pendientes SPEC-002:
- No se implementó AC-05 (prompt vacío → Reasoning) — pendiente de verificación
- No se implementó AC-06 (explica cómo funciona → Reasoning) — pendiente de verificación

## Implementación SPEC-003 ✅

### Motor de reglas declarativas (`src/agent/direct_matcher.rs`)

**Estructuras:**
- `CommandRule { intent, pattern, command_template, destructive }` — serializable a TOML
- `DirectMatch { description, command, is_destructive }` — resultado del match
- `DirectMatcher` — motor con `new()`, `with_file_commands(path)` y `match_prompt(prompt) -> Option<DirectMatch>`

**Reglas built-in (11):**
1. pwd — `pwd` (no destructivo)
2. actualizar — `apt update 2>&1` (no destructivo)
3. cat — `cat {0}` (no destructivo)
4. mkdir — `mkdir -p {0}` (no destructivo)
5. rmdir — `rm -rf {0}` (destructivo)
6. rm — `rm {0}` (destructivo)
7. buscar — `find . -name '*{0}*'` (no destructivo)
8. touch — `touch {0}` (no destructivo)
9. write_file — `printf '%s' {1} > {0}` (no destructivo)
10. ls -la — `ls -la` (no destructivo)
11. apt update — `apt update 2>&1` (no destructivo, duplicado)

**Tests creados (18):**
- 6 tests por AC (AC-01 a AC-06) — todos pasando
- 12 tests adicionales de edge cases: precedencia, orden, patrones vacíos, archivos inexistentes, TOML inválido, regex inválidos, captura de índices 0/1, escape de caracteres peligrosos

**Integración:**
- `src/agent/mod.rs` actualizado con `pub mod direct_matcher;`

## Pendiente en próxima sesión

### Prioridad 1: SPEC-004 — Fuzzy matching
- Implementar fuzzy matching con el crate `similar` (ya en Cargo.toml)
- Umbral 0.85 de similitud
- Parte de `DirectMatcher` o componente separado (decisión pendiente)

### Prioridad 2: SPEC-005 — Flujo en core.rs
- Modificar `core.rs::run()` para usar `IntentClassifier` + `DirectMatcher`
- Añadir `execute_action` / `execute_reasoning`
- Verificar ACs de SPEC-002/003 que faltan por implementar

### Prioridad 3: SPEC-001 — Soporte cloud
- Actualizar `config.toml.example` con sección `[[commands]]`
- Implementar backend cloud de referencia (OpenAI-compatible) con `trait Provider`
- Documentar fallback

### Verificación pendiente:
- AC-05 y AC-06 de SPEC-002 (prompt vacío y "explica cómo funciona")
- Integración de fuzzy dentro del `DirectMatcher` (decisión pendiente)

## Criterios de éxito para próxima sesión

1. ✅ SPEC-004 implementado con tests
2. ✅ SPEC-005 integrando intent + direct_matcher
3. ✅ Probar que flujo funciona con prompts reales
4. ✅ Verificar que acciones NO invocan IA (AC-01, AC-05 de SPEC-005)
