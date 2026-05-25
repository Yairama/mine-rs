# Backlog de desarrollo — mine-rs

Este documento es el backlog operativo de `mine-rs`.

`mine-rs` es una plataforma Rust-first enfocada en construir:

* infraestructura computacional minera
* primitives reutilizables
* motores deterministas
* tooling AI-native
* interoperabilidad minera moderna
* planeamiento “as code”

El proyecto NO busca inicialmente construir una suite GUI monolitica.

La arquitectura objetivo es:

```text id="bjlwm0"
Rust Core
↓
Python Bindings
↓
Agentic Layer
↓
CLI / Notebooks / Automation
↓
UI futura opcional
```

---

# Filosofia del proyecto

`mine-rs` sigue una filosofia similar a:

* Bevy
* Polars
* DuckDB
* PyTorch

pero aplicada a mineria.

El objetivo es construir:

```text id="d57j1x"
Mining Engineering Infrastructure
```

y no simplemente otro software propietario cerrado.

---

# Vision agentica

El sistema conversacional NO debe calcular mineria directamente.

Los agentes:

* interpretan
* planean
* orquestan
* validan
* explican

Pero:

```text id="20bf9e"
Los calculos mineros reales SIEMPRE deben ejecutarse mediante tools deterministas.
```

Arquitectura objetivo:

```text id="fsh9ah"
User
↓
Mine Agent
↓
Task Tool
↓
Subagents especializados
↓
mine-rs deterministic tools
↓
JSON structured outputs
↓
VFS artifacts
```

Inspiracion:

* deep-agents-from-scratch
* LangGraph
* task tool orchestration
* virtual filesystem memory
* specialist subagents

---

# Leyenda de estado

| Estado | Significado         |
| ------ | ------------------- |
| `[ ]`  | Pendiente           |
| `[~]`  | En progreso         |
| `[x]`  | Completado          |
| `[!]`  | Bloqueado o pausado |

---

# Prioridades

| Prioridad | Significado                                   |
| --------- | --------------------------------------------- |
| P0        | Base obligatoria para que el proyecto exista. |
| P1        | MVP usable por ingenieros.                    |
| P2        | Funcionalidad avanzada importante.            |
| P3        | Vision futura / AI avanzada / enterprise.     |

---

# Objetivos del MVP

El MVP debe permitir:

* cargar block models
* validar modelos
* xyz ↔ ijk
* reblocking
* analytics basicos
* curvas ley-tonelaje
* exportacion visual
* uso desde Python
* conversaciones sobre modelos mediante agentes
* generacion de escenarios basicos

---

# Arquitectura inicial

```text id="58ttji"
mine-rs/
├─ crates/
│
│  ├─ mine-core/
│  ├─ mine-blockmodel/
│  ├─ mine-indexing/
│  ├─ mine-validation/
│  ├─ mine-reblock/
│  ├─ mine-geometry/
│  ├─ mine-economics/
│  ├─ mine-planning/
│  ├─ mine-visualization/
│  ├─ mine-io/
│  ├─ mine-sdk/
│  ├─ mine-tools/
│  ├─ mine-python/
│  ├─ mine-cli/
│  └─ mine-agent-* opcionales para piezas deterministas futuras
│
├─ python/
│  ├─ miners/
│  └─ mine-agents/
│
├─ examples/
├─ benchmarks/
├─ datasets/
├─ docs/
└─ tests/
```

Decision de arquitectura: mantener monorepo y separar por crates/paquetes. `mine-sdk` sera la fachada Rust publica, `mine-python` el binding nativo, `python/miners` la API Python de usuario, `mine-tools` los contratos deterministas y `python/mine-agents` la capa agentica Python-first. Ver `docs/references/repository-strategy.md`.

---

# Checklist maestro

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 1 | MR-001 | `[ ]` | P0 | Repo | Crear workspace Rust | Crear estructura base multi-crate del proyecto. | Workspace compilable | - | `cargo build --workspace` funciona correctamente. |

| 2 | MR-002 | `[ ]` | P0 | Repo | Configurar dependencias base | Integrar `serde`, `thiserror`, `anyhow`, `rayon`, `arrow`, `parquet`, `nalgebra`, `tracing`, `pyo3`. | Tooling base | MR-001 | Proyecto compila sin errores. |

| 3 | MR-003 | `[ ]` | P0 | Repo | Configurar CI/CD | Configurar GitHub Actions para build, tests y fmt. | Pipeline CI | MR-001 | Pull requests validan automaticamente. |

| 4 | MR-004 | `[ ]` | P0 | Repo | Configurar linting | Configurar `clippy`, `rustfmt`, reglas de calidad y warnings estrictos. | Linting estable | MR-001 | `cargo clippy` pasa sin warnings criticos. |

| 5 | MR-005 | `[ ]` | P0 | Repo | Configurar benchmarking | Integrar `criterion` para benchmarks reproducibles. | Infra benchmarks | MR-001 | Benchmarks ejecutables correctamente. |

---

# EPIC — Core Block Model Engine

## Objetivo

Construir el motor central de block models.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 10 | MR-010 | `[ ]` | P0 | Core | Crear `Coordinate3D` | Struct base para coordenadas espaciales. | Coordinate3D | MR-001 | Coordenadas serializables y testeadas. |

| 11 | MR-011 | `[ ]` | P0 | Core | Crear `BlockDimensions` | Representar dimensiones espaciales de bloques. | BlockDimensions | MR-010 | Valores invalidos son rechazados. |

| 12 | MR-012 | `[ ]` | P0 | Core | Crear `GridDefinition` | Definir origin, dims y rotacion de grilla. | GridDefinition | MR-011 | GridDefinition valida correctamente. |

| 13 | MR-013 | `[ ]` | P0 | Core | Crear `BlockModel` | Modelo principal del SDK. | BlockModel | MR-012 | Soporta datasets grandes. |

| 14 | MR-014 | `[ ]` | P0 | Core | Diseñar storage columnar | Arquitectura columnar eficiente tipo Arrow. | Column store | MR-013 | Lecturas eficientes benchmarkeadas. |

| 15 | MR-015 | `[ ]` | P0 | Core | Metadata engine | Metadata global y por columnas. | Metadata API | MR-013 | Metadata serializable correctamente. |

---

# EPIC — Indexing Engine

## Objetivo

Sistema determinista xyz ↔ ijk.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 20 | MR-020 | `[ ]` | P0 | Indexing | xyz → ijk | Conversion espacial a indices. | xyz_to_ijk | MR-012 | Tests exactos pasan. |

| 21 | MR-021 | `[ ]` | P0 | Indexing | ijk → xyz | Conversion indices a coordenadas. | ijk_to_xyz | MR-020 | Reversible deterministicamente. |

| 22 | MR-022 | `[ ]` | P0 | Indexing | Linear indexing | Conversion 3D ↔ 1D. | linear indexing | MR-020 | Compatible con modelos grandes. |

| 23 | MR-023 | `[ ]` | P1 | Indexing | Rotated grids | Soporte para modelos rotados. | Rotation engine | MR-021 | Error espacial bajo tolerancia. |

| 24 | MR-024 | `[ ]` | P1 | Indexing | Sparse indexing | Soporte sparse block models. | Sparse engine | MR-022 | Lookups eficientes benchmarkeados. |

---

# EPIC — Validation Engine

## Objetivo

Detectar problemas estructurales del modelo.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 30 | MR-030 | `[ ]` | P0 | Validation | Duplicate detection | Detectar bloques duplicados. | Duplicate validator | MR-013 | Duplicados detectados correctamente. |

| 31 | MR-031 | `[ ]` | P0 | Validation | Regular grid validation | Validar regularidad espacial. | Grid validator | MR-020 | Casos invalidos detectados. |

| 32 | MR-032 | `[ ]` | P1 | Validation | Missing blocks detection | Detectar gaps internos. | Gap validator | MR-031 | Gaps detectados correctamente. |

| 33 | MR-033 | `[ ]` | P1 | Validation | Extents validation | Validar limites espaciales. | Extents validator | MR-031 | Warnings correctos generados. |

| 34 | MR-034 | `[ ]` | P1 | Validation | Validation report | Reporte estructurado JSON. | ValidationReport | MR-030 | Export JSON funcional. |

---

# EPIC — Reblocking Engine

## Objetivo

Motor de reblocking reutilizable y determinista.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 40 | MR-040 | `[ ]` | P1 | Reblock | Aggregation rules | Sistema declarativo de agregacion. | AggregationRules | MR-013 | API ergonomica funcional. |

| 41 | MR-041 | `[ ]` | P1 | Reblock | Superblocking | Agrupar bloques. | Superblock engine | MR-040 | Conserva tonelaje y metal. |

| 42 | MR-042 | `[ ]` | P1 | Reblock | Subblocking | Dividir bloques. | Subblock engine | MR-041 | Determinista y reproducible. |

| 43 | MR-043 | `[ ]` | P1 | Reblock | Weighted aggregation | Weighted mean/min/max. | Aggregation engine | MR-040 | Resultados reconciliables. |

| 44 | MR-044 | `[ ]` | P2 | Reblock | Adaptive reblocking | Reblocking variable. | Adaptive engine | MR-041 | Funciona correctamente en grids irregulares. |

| 45 | MR-045 | `[ ]` | P2 | Reblock | Reconciliation report | Comparacion before/after. | Reconciliation report | MR-041 | Diferencias cuantificadas correctamente. |

---

# EPIC — IO Engine

## Objetivo

Interoperabilidad con workflows reales.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 50 | MR-050 | `[ ]` | P0 | IO | CSV IO | Leer/escribir CSV. | CSV engine | MR-013 | Roundtrip funcional. |

| 51 | MR-051 | `[ ]` | P0 | IO | Parquet IO | Soporte parquet columnar. | Parquet engine | MR-014 | Compatible con Python/Arrow. |

| 52 | MR-052 | `[ ]` | P1 | IO | Arrow IPC | Integracion Arrow. | Arrow backend | MR-051 | Zero-copy parcial funcional. |

| 53 | MR-053 | `[ ]` | P1 | IO | Vulcan CSV export | Export compatible Vulcan. | Vulcan exporter | MR-050 | Vulcan importa correctamente. |

| 54 | MR-054 | `[ ]` | P2 | IO | Vulcan BDF support | Generacion BDF. | BDF generator | MR-053 | BDF valido generado. |

| 55 | MR-055 | `[ ]` | P1 | IO | VTK export | Exportacion ParaView. | VTU exporter | MR-013 | Visualizable correctamente. |

---

# EPIC — Planning Primitives

## Objetivo

Primitives reutilizables para planeamiento minero.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 60 | MR-060 | `[ ]` | P1 | Planning | Bench generation | Generacion automatica de benches. | Bench engine | MR-013 | Benching correcto. |

| 61 | MR-061 | `[ ]` | P1 | Planning | Phase tagging | Asignar fases y shells. | Phase engine | MR-060 | Bloques correctamente asignados. |

| 62 | MR-062 | `[ ]` | P2 | Planning | Precedence graph | Grafo de precedencias. | DAG engine | MR-061 | Orden valido generado. |

| 63 | MR-063 | `[ ]` | P2 | Planning | Vertical advance rules | Restricciones verticales. | Constraint engine | MR-062 | Restricciones respetadas. |

| 64 | MR-064 | `[ ]` | P2 | Planning | Schedule primitives | Scheduling base reutilizable. | Schedule API | MR-062 | Schedules reproducibles. |

---

# EPIC — Economics

## Objetivo

Analytics economicos reutilizables.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 70 | MR-070 | `[ ]` | P1 | Economics | Grade-tonnage | Curvas ley-tonelaje. | GT engine | MR-043 | Curvas reconciliables. |

| 71 | MR-071 | `[ ]` | P2 | Economics | Metal calculations | Metal contenido. | Metal engine | MR-070 | Resultados correctos. |

| 72 | MR-072 | `[ ]` | P2 | Economics | NPV calculations | Cashflow y NPV. | Financial engine | MR-071 | Resultados reproducibles. |

| 73 | MR-073 | `[ ]` | P3 | Economics | Scenario analysis | Comparacion multi escenario. | Scenario engine | MR-072 | Comparaciones exportables. |

---

# EPIC — Python Bindings

## Objetivo

Experiencia Python-first.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 80 | MR-080 | `[ ]` | P1 | Python | Configurar pyo3 | Base bindings Python. | Python crate | MR-013 | `import miners` funciona. |

| 81 | MR-081 | `[ ]` | P1 | Python | Exponer BlockModel | Bindings principales. | Python BlockModel | MR-080 | Operaciones basicas funcionales. |

| 82 | MR-082 | `[ ]` | P1 | Python | Pandas interoperability | Conversion pandas ↔ Rust. | Pandas bridge | MR-081 | Roundtrip correcto. |

| 83 | MR-083 | `[ ]` | P2 | Python | Numpy interoperability | Soporte numpy arrays. | Numpy bridge | MR-082 | Zero-copy parcial funcional. |

| 84 | MR-084 | `[ ]` | P2 | Python | Fluent dataframe API | API estilo dataframe. | Fluent API | MR-081 | UX limpia desde notebooks. |

---

# EPIC — Agentic Layer

## Objetivo

Construir capa agentica basada en tools deterministas.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 90 | MR-090 | `[ ]` | P1 | Agent | Diseñar task tool | Tool principal de delegacion agentica. | Task tool | MR-081 | Delegacion funcional. |

| 91 | MR-091 | `[ ]` | P1 | Agent | Virtual filesystem | Memoria persistente tipo VFS. | VFS engine | MR-090 | Artefactos persistentes funcionales. |

| 92 | MR-092 | `[ ]` | P1 | Agent | Tool contracts | Contratos JSON estructurados. | Tool schema | MR-090 | Schemas validables. |

| 93 | MR-093 | `[ ]` | P1 | Agent | Model inspector agent | Subagente de inspeccion. | Inspector agent | MR-091 | Genera perfiles correctamente. |

| 94 | MR-094 | `[ ]` | P1 | Agent | Validation agent | Subagente de validacion. | Validation agent | MR-093 | Reportes correctos. |

| 95 | MR-095 | `[ ]` | P2 | Agent | Economics agent | Analytics economicos conversacionales. | Economics agent | MR-070 | Explica resultados correctamente. |

| 96 | MR-096 | `[ ]` | P2 | Agent | Planning agent | Planeamiento conversacional. | Planning agent | MR-064 | Genera escenarios validos. |

| 97 | MR-097 | `[ ]` | P2 | Agent | Verifier agent | Verificador de outputs y supuestos. | Verifier agent | MR-092 | Detecta inconsistencias. |

| 98 | MR-098 | `[ ]` | P2 | Agent | Scenario comparison agent | Comparacion multi escenario. | Comparison agent | MR-073 | Comparaciones coherentes. |

| 99 | MR-099 | `[ ]` | P3 | Agent | Natural language planning | Conversaciones complejas de planeamiento. | NL planner | MR-096 | Traduce correctamente a tools. |

---

# EPIC — Deterministic Tools

## Objetivo

Tools estructuradas para agentes.

---

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |

| 110 | MR-110 | `[ ]` | P1 | Tools | inspect_model | Schema y metadata. | Tool funcional | MR-081 | Output JSON correcto. |

| 111 | MR-111 | `[ ]` | P1 | Tools | query_blocks | Query estructurado. | Query tool | MR-081 | Filtros funcionan correctamente. |

| 112 | MR-112 | `[ ]` | P1 | Tools | aggregate_blocks | Aggregations estructuradas. | Aggregation tool | MR-111 | Resultados correctos. |

| 113 | MR-113 | `[ ]` | P1 | Tools | validate_model | Validacion estructurada. | Validation tool | MR-034 | Warnings correctos. |

| 114 | MR-114 | `[ ]` | P1 | Tools | grade_tonnage | Curvas ley-tonelaje. | GT tool | MR-070 | Curvas correctas. |

| 115 | MR-115 | `[ ]` | P2 | Tools | create_scenario | Crear escenarios de planeamiento. | Scenario tool | MR-064 | Escenario valido generado. |

| 116 | MR-116 | `[ ]` | P2 | Tools | evaluate_scenario | Evaluar escenarios. | Evaluation tool | MR-115 | Resultados reproducibles. |

| 117 | MR-117 | `[ ]` | P2 | Tools | compare_scenarios | Comparacion multi escenario. | Comparison tool | MR-116 | Comparaciones correctas. |
