# SDD — Spec-Driven Development

> Documento de metodología SDD · v1.0.0 · La especificación (spec) como contrato único de verdad.

## 1. Qué es SDD

**SDD (Spec-Driven Development)** es el protocolo de desarrollo dirigido por especificación usado dentro del marco SAQI. Establece que **la spec es el contrato ejecutable**: anterior al código, fuente de verdad para humanos y agentes IA, y criterio de validación final.

**Principio rector**: *La spec no es documentación, es contrato ejecutable. Si no se puede testear, no está lista.*

Aplica a TODO proyecto SAQI. Nivel **A — Core obligatorio**.

---

## 2. Ciclo SDD integrado en SAQI

| Fase SAQI | Entrada / Salida | Rol de la spec |
|-----------|------------------|----------------|
| **1 Plan** | Spec madura → `ITERATION_PLAN.md` | **Gate**: no inicia sin `SPEC_READY = true` |
| **2 Architect** | ADRs, C4, contratos de puertos | Derivados directamente de la spec |
| **3 Skill Selection** | Skills/versiones aprobadas | Skills alineadas con los ACs de la spec |
| **4 Build** | Código (TDD: Red → Green → Refactor) | Criterios de aceptación = tests |
| **10/11 Validate/Verify** | Validación contra spec | `test:acceptance` = 100% pass |

### Reglas clave del ciclo

| ID | Regla |
|----|-------|
| **A-SDD-001** | Fase 1 (Plan) **no inicia** sin spec madura. Gate: `SPEC_READY = true`. |
| **A-SDD-002** | Spec = **contrato único**. Código, tests, ADRs y docs derivados de spec. Versionada en `docs/specs/SPEC-XXX.md` (SemVer). |
| **A-SDD-003** | Cada criterio de aceptación (Given/When/Then) → test automatizado en `tests/functional/`. **Mapping 1:1**. |
| **A-SDD-004** | Sección **"No incluye"** obligatoria. Sin exclusiones → spec rechazada. |
| **A-SDD-005** | **Spec antes que código** (TDD asistido: test ROJO → código VERDE → refactor). El agente IA usa la spec como contexto primario. |
| **A-SDD-006** | Validación **contra spec, no contra opiniones**. Gate: tests derivados de spec = 100%. |

---

## 3. Estructura canónica de una spec

Una spec válida usa la plantilla canónica (A-SDD-010):

```markdown
# SPEC-XXX — <Título funcional>

## Problema
## Contexto            # usuario real, situación concreta, limitaciones
## Objetivo            # falsable, binario: "El sistema hace X" (pass/fail claro)
## Alcance
### Incluye
### No incluye          # OBLIGATORIO (A-SDD-004)
## Comportamiento
### Flujo principal
### Flujos alternativos / Edge cases
## Criterios de aceptación   # Given/When/Then, binarios, 1:1 con tests
## Restricciones        # técnicas + negocio + seguridad
## Notas
```

### Reglas de calidad de la spec

| ID | Regla |
|----|-------|
| **A-SDD-011** | **Objetivo falsable**: verificable binariamente. No "mejora", "optimiza", "facilita". |
| **A-SDD-012** | **Contexto acotado**: usuario real, situación y límites concretos. No "usuario general". |
| **A-SDD-013** | **Ejemplos ejecutables**: mínimo 1 por flujo principal. Reducen interpretación. |
| **A-SDD-014** | **Restricciones explícitas**: técnicas, negocio y seguridad. Nunca implícitas. |

### Ejemplo de criterios de aceptación (Given/When/Then + test 1:1)

```markdown
- **AC-01**: Dado un email no registrado y una contraseña válida,
  cuando se procesa la solicitud, entonces se crea un nuevo usuario en estado "pending_verification".
```
```typescript
it('AC-01: creates user with pending_verification on valid input', async () => {
  // Given
  const input = { email: 'new@example.com', password: 'ValidPass123!' };
  // When
  const result = await service.register(input);
  // Then
  expect(result.isOk()).toBe(true);
  expect(result.unwrap().status).toBe('pending_verification');
});
```

---

## 4. Integración con agentes IA

| ID | Regla |
|----|-------|
| **A-SDD-020** | Spec = **contexto primario** del agente (mode `builder` → `context_docs: ["specs/SPEC-XXX.md", "ARCHITECTURE.md", "CONTEXT.md"]`). |
| **A-SDD-021** | El agente **NO decide fuera de la spec**. Ante ambigüedad: para y pregunta al humano. No "inventa" comportamiento (no-invention rule). |

---

## 5. Anti-patrones prohibidos

1. Objetivos vagos: "mejorar", "optimizar", "facilitar" sin criterio medible.
2. Contexto genérico sin usuario/escenario concretos.
3. Sin sección "No incluye".
4. Criterios de aceptación no binarios o sin test derivado.
5. Cambiar la spec en Fase 4 sin actualizar tests y backend.
6. Spec que contradice ADRs existentes.
7. ACs duplicados entre specs sin trazabilidad.
8. Agente implementando comportamiento no especificado.
9. Spec sin versionado ni changelog.
10. Validar por opinión en vez de contra tests de spec.
---

## 6. Procedimientos

### 6.1 Crear / actualizar spec (Fase 1)
1. Definir `SPEC-XXX` con la plantilla canónica (frontmatter con `spec_id`).
2. Checklist de revisión completado.
3. Validar estructura: `specmark validate SPEC-XXX.md` → 0 errors.
4. Actualizar `CONTEXT.md` → spec activa.
5. Crear `ITERATION_PLAN.md` con backlog derivado de los ACs.

### 6.2 Derivar tests de la spec (Fase 4 → 5)
1. Por cada AC → test `tests/functional/SPEC-XXX-AC-YY.test.ts`.
2. Implementar test **ROJO** (falla) → TDD.
3. Implementar código → test **VERDE**.
4. Refactor manteniendo tests verdes.
5. Registrar trazabilidad `SPEC-XXX-AC-YY → test → src/file.rs:line`.

### 6.3 Cambio de spec (post-Fase 1)
1. Detectar la necesidad (nuevo requisito / bug / cambio de negocio).
2. Crear `SPEC-XXX vN+1` con frontmatter actualizado y `impact_analysis.md`.
3. Actualizar tests afectados (ROJO → VERDE), código y ADRs afectados.
4. Actualizar `ITERATION_PLAN.md` + `CHANGELOG.md`.
5. Gate: `validateSpec()` + tests 100% pass.

### 6.4 Spec Review Checklist

```
[ ] Problema entendido sin contexto adicional
[ ] Objetivo binario falsable (pass/fail claro)
[ ] Contexto: usuario real + escenario + límites
[ ] Alcance: Incluye + No incluye (ambos no vacíos)
[ ] Comportamiento: flujo principal + alternativos + edge cases
[ ] ACs: todos Given/When/Then, binarios, verificables
[ ] Ejemplos: ≥1 por flujo crítico
[ ] Restricciones: técnicas + negocio + seguridad
[ ] Trazabilidad: spec_id + related_specs/adrs
[ ] validateSpec() → 0 errors
[ ] Firmas: PM + Tech Lead + QA Lead
```

---

## 7. Criterios de verificación

| Check | Herramienta | Gate |
|-------|-------------|------|
| Spec válida | `specmark validate SPEC-XXX.md` | 0 errors |
| Objetivo falsable | Manual + `validateSpec()` | Pass |
| Exclusiones explícitas | `validateSpec()` | Pass |
| ACs Given/When/Then | Parser BDD | 100% compliant |
| ACs binarios | `validateSpec()` | Pass |
| Trazabilidad spec→test→código | Script `check-traceability` | 100% mapped |
| `SPEC_READY` gate | Checklist firmado | Fase 1 entry |

---

## 8. Aplicación de SDD al proyecto `fe`

- **Uso directo**: `fe "crea una app python que sume"` genera proyectos a partir de la descripción; ese flujo puede formalizarse como spec para validar contra criterios verificables.
- **Spec-first del código del agente**: el desarrollo de `fe` (p. ej. la futura capa `direct_matcher`) debe seguir SDD: definir los criterios de aceptación de "entender un comando en lenguaje natural" como ACs Given/When/Then y mapearlos a tests.
- **Trazabilidad**: cada nuevo comando de lenguaje natural → su regla de matching → su test unitario (1:1).
- Consulta `docs/SAQI.md` para el marco general y `AGENT.md` para el comportamiento del agente.

---

## 9. Changelog

### v1.0.0 — 2026-09-08
- Definición de SDD, principio rector y ciclo SDD integrado en SAQI.
- Reglas MUST, estructura canónica de spec, anti-patrones y procedimientos.
- Criterios de verificación y sección de aplicación al proyecto `fe`.