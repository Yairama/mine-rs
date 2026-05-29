# Backlog final de desarrollo — mine-rs

Este documento es el backlog operativo principal de `mine-rs`.

`mine-rs` es un SDK Rust-first para ingeniería de minas, con una capa Python como superficie principal de uso y una capa agentica futura basada en tools deterministas.

El proyecto busca construir infraestructura computacional minera:

* modelos de bloques reutilizables
* motores deterministas y reproducibles
* interoperabilidad moderna con Python, Arrow y Parquet
* primitives para validación, reblocking, economía y planeamiento
* tools estructuradas para agentes
* automatización minera verificable

El proyecto NO busca inicialmente construir una suite GUI monolítica.

---

# Documentos de referencia

Los documentos estratégicos viven en `docs/references/`:

| Documento | Uso |
| --- | --- |
| `docs/references/vision.md` | Misión, visión y principios. |
| `docs/references/product-scope.md` | Alcance del producto y límites iniciales. |
| `docs/references/domain-capabilities.md` | Mapa de capacidades mineras objetivo. |
| `docs/references/mining-engine-roadmap.md` | Ruta end-to-end basada en literatura para estimación, economía, pit final, pushbacks y LOM. |
| `docs/references/architecture.md` | Arquitectura técnica por capas. |
| `docs/references/repository-strategy.md` | Estrategia de monorepo, crates y paquetes. |
| `docs/references/sparse-blockmodel-design.md` | Diseño experimental de la materialización sparse en `BlockModel`. |
| `docs/references/python-sdk-design.md` | Diseño de experiencia Python. |
| `docs/references/agentic-layer.md` | Diseño conceptual de la capa agentica. |
| `docs/references/roadmap.md` | Roadmap narrativo. |
| `docs/references/temporal-backlog.md` | Backlog temporal original usado como insumo histórico. |

Este backlog final reemplaza a `docs/references/temporal-backlog.md` como fuente operativa.

---

# Arquitectura objetivo

```text
mine-rs/
├─ crates/
│  ├─ mine-core/
│  ├─ mine-blockmodel/
│  ├─ mine-indexing/
│  ├─ mine-validation/
│  ├─ mine-reblock/
│  ├─ mine-io/
│  ├─ mine-economics/
│  ├─ mine-planning/
│  ├─ mine-sdk/
│  ├─ mine-tools/
│  ├─ mine-python/
│  └─ mine-cli/
│
├─ python/
│  ├─ miners/
│  └─ mine-agents/
│
├─ examples/
├─ benchmarks/
├─ datasets/
├─ docs/
├─ tests/
└─ .github/
```

Decision vigente:

```text
Monorepo con separacion interna fuerte.
No separar repositorios hasta que existan APIs estables y ciclos de release independientes.
```

Direccion de dependencias:

```text
mine-core
↓
crates de dominio
↓
mine-sdk
↓
mine-tools
↓
mine-python / mine-cli / python/mine-agents
```

Regla agentica central:

```text
Los agentes razonan y orquestan.
Las tools deterministas calculan y validan.
```

---

# Leyenda de estado

| Estado | Significado |
| --- | --- |
| `[ ]` | Pendiente |
| `[~]` | En progreso |
| `[x]` | Completado |
| `[!]` | Bloqueado o pausado |

---

# Prioridades

| Prioridad | Significado |
| --- | --- |
| P0 | Base obligatoria para que el proyecto exista. |
| P1 | MVP usable por ingenieros desde Python. |
| P2 | Funcionalidad avanzada importante. |
| P3 | Vision futura, agentes avanzados, enterprise o UI. |

---

# Definition of Done global

Todo ticket funcional debe cumplir, salvo que el ticket indique lo contrario:

* compilar en Rust con `cargo build --workspace`
* tener tests automatizados relevantes
* mantener errores explícitos y tipados
* evitar defaults silenciosos en supuestos mineros
* preservar determinismo y reproducibilidad
* documentar APIs públicas o ejemplos si cambia la experiencia de usuario
* no introducir dependencias agenticas en el SDK base
* no romper la direccion de dependencias definida

---

# Objetivos del MVP

El MVP debe permitir:

* crear y cargar block models
* definir grillas y metadata
* validar estructura espacial y schema
* convertir `xyz ↔ ijk`
* leer/escribir CSV y Parquet
* usar el SDK desde Python
* convertir datos con pandas
* calcular estadísticas mineras básicas
* calcular curvas ley-tonelaje
* ejecutar reblocking básico
* exportar resultados estructurados
* exponer tools deterministas iniciales para agentes

---

# Checklist maestro

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 1 | MR-001 | `[x]` | P0 | Repo | Convertir repo a workspace Rust | Reemplazar el crate binario mínimo por un workspace Rust inicial con `crates/`, `examples/`, `tests/`, `benchmarks/`, `datasets/` y estructura preparada para SDK. Mantener el proyecto compilable durante la transición. | Workspace Rust base | - | `cargo metadata` y `cargo build --workspace` funcionan; la estructura coincide con `docs/references/repository-strategy.md`. |
| 2 | MR-002 | `[x]` | P0 | Repo | Crear crates base mínimos | Crear `mine-core`, `mine-sdk`, `mine-tools` y `mine-python` como crates iniciales. `mine-sdk` debe reexportar lo público; `mine-python` no debe contener lógica minera; `mine-tools` debe depender de `mine-sdk`. | Crates base | MR-001 | La direccion de dependencias se valida con `cargo tree`; ningun crate base depende de capas superiores. |
| 3 | MR-003 | `[x]` | P0 | Repo | Configurar dependencias base | Integrar dependencias iniciales: `serde`, `serde_json`, `thiserror`, `anyhow` solo en binarios/tools si aplica, `tracing`, `rayon`, `arrow`, `parquet`, `nalgebra`, `pyo3` y `maturin`. Documentar por qué cada dependencia entra. | Dependencias base | MR-002 | `cargo build --workspace` funciona; no hay dependencias agenticas en crates del SDK. |
| 4 | MR-004 | `[x]` | P0 | Repo | Configurar calidad Rust | Configurar `rustfmt`, `clippy`, warnings estrictos razonables y comandos documentados para formato, lint y test. | Tooling de calidad Rust | MR-001 | `cargo fmt --check`, `cargo clippy --workspace --all-targets` y `cargo test --workspace` ejecutan sin errores en el estado base. |
| 5 | MR-005 | `[x]` | P0 | Repo | Configurar CI inicial | Crear GitHub Actions para build, fmt, clippy y tests del workspace. La CI debe ser simple y expandible para Python después. | Pipeline CI Rust | MR-004 | Un PR ejecuta CI; fallos de formato, lint o tests bloquean merge. |
| 6 | MR-006 | `[x]` | P0 | Repo | Configurar benchmarks | Integrar `criterion` y estructura `benchmarks/` para medir operaciones críticas futuras: indexing, validación, IO y agregación. | Infra benchmarks | MR-004 | Existe un benchmark smoke test ejecutable y documentado. |
| 7 | MR-007 | `[x]` | P0 | Repo | Configurar package Python base | Crear `pyproject.toml`, layout `python/miners`, configuración Maturin y workflow local para construir el módulo Python desde `mine-python`. | Packaging Python inicial | MR-002 | `maturin develop` o comando equivalente genera un paquete importable en entorno local. |
| 8 | MR-008 | `[x]` | P0 | Repo | Crear guía de contribución técnica | Documentar cómo correr build, tests, lint, formato, benchmarks, bindings Python y cómo decidir en qué crate implementar. | Guia contributor | MR-004 | README o docs explican comandos y boundaries; nuevos agentes pueden ubicar el punto correcto de implementación. |

---

# EPIC — Core Domain Foundation

## Objetivo

Definir tipos, errores y contratos base reutilizables por todo el SDK.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 10 | MR-010 | `[x]` | P0 | Core | Definir modelo de errores | Crear jerarquía de errores en `mine-core` para IO, schema, grid, validación, reblocking, economía, planeamiento y parámetros inválidos. Evitar errores genéricos en APIs públicas. | `MineError` y tipos relacionados | MR-002 | Cada error público tiene mensaje claro, variante tipada y tests de conversión/formatting. |
| 11 | MR-011 | `[x]` | P0 | Core | Crear tipos de identificadores | Definir IDs para modelo, bloque, columna, escenario y artefacto. Evitar strings crudos en APIs internas cuando representen conceptos del dominio. | Value objects de IDs | MR-010 | IDs serializan/deserializan correctamente y rechazan valores vacíos o inválidos. |
| 12 | MR-012 | `[x]` | P0 | Core | Crear `Coordinate3D` | Representar coordenadas espaciales `x`, `y`, `z` con tipos numéricos, serialización y helpers mínimos. No asumir unidades globales. | `Coordinate3D` | MR-010 | Tests cubren construcción, serialización y comparación con tolerancia cuando aplique. |
| 13 | MR-013 | `[x]` | P0 | Core | Crear `BlockDimensions` | Representar tamaño de bloque `dx`, `dy`, `dz`; validar que todas las dimensiones sean positivas y finitas. | `BlockDimensions` | MR-012 | Valores cero, negativos, NaN o infinitos son rechazados con error tipado. |
| 14 | MR-014 | `[x]` | P0 | Core | Crear `GridShape` | Representar cantidad de bloques por eje `nx`, `ny`, `nz` con límites seguros para indexación. | `GridShape` | MR-013 | Rechaza ejes cero y detecta overflow potencial en conteo total. |
| 15 | MR-015 | `[x]` | P0 | Core | Crear `GridDefinition` | Definir origen, dimensiones, shape y rotación opcional. Debe ser la fuente de verdad para conversiones espaciales. | `GridDefinition` | MR-014 | Valida configuración completa y serializa/deserializa sin pérdida. |
| 16 | MR-016 | `[x]` | P0 | Core | Crear sistema de metadata | Implementar metadata global y por columna con tipos simples, serializables y preservables en IO. | `Metadata` | MR-011 | Metadata roundtrip en JSON; claves inválidas o duplicadas se manejan explícitamente. |
| 17 | MR-017 | `[x]` | P0 | Core | Definir esquema de columnas | Crear tipos para nombre, tipo lógico, unidad, nullable y rol minero de columnas como ley, tonelaje, densidad, dominio, banco o fase. | `ColumnSchema` | MR-016 | Schema puede validar presencia/tipo de columnas requeridas. |

---

# EPIC — Block Model Engine

## Objetivo

Construir el motor central de modelos de bloques.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 20 | MR-020 | `[x]` | P0 | BlockModel | Crear `BlockModel` mínimo | Implementar entidad principal con `GridDefinition`, schema, metadata y storage columnar inicial. Debe poder representar un modelo regular pequeño en memoria. | `BlockModel` | MR-015, MR-017 | Se puede construir un modelo con columnas básicas y consultar conteo de bloques, schema y metadata. |
| 21 | MR-021 | `[x]` | P0 | BlockModel | Diseñar storage columnar | Implementar almacenamiento columnar basado en Arrow o abstracción compatible. Separar atributos de geometría para evitar copias innecesarias. | Column store | MR-020 | Lectura de columna por nombre funciona; tipos incompatibles producen error claro. |
| 22 | MR-022 | `[x]` | P0 | BlockModel | API de selección de columnas | Permitir seleccionar subconjuntos de columnas preservando schema y metadata relevante. | Column selection API | MR-021 | Selecciones válidas devuelven modelo/vista consistente; columnas inexistentes fallan explícitamente. |
| 23 | MR-023 | `[x]` | P1 | BlockModel | API de filtros básicos | Permitir filtros por rango espacial y por columnas booleanas/numericas simples sin acoplarse a Python. | Filtering API | MR-022 | Tests cubren filtros por coordenada, dominio y ley mínima en datasets pequeños. |
| 24 | MR-024 | `[x]` | P1 | BlockModel | Soporte de modelos sparse | Diseñar representación para modelos con bloques faltantes o no materializados sin romper la API regular. | Sparse block model design | MR-020 | Documento/API experimental define invariantes y tradeoffs; validadores distinguen sparse permitido vs gaps inválidos. |
| 25 | MR-025 | `[x]` | P1 | BlockModel | Resumen de modelo | Crear método para obtener perfil del modelo: dimensiones, extents, columnas, tipos, nulos, memoria aproximada y metadata clave. | `ModelSummary` | MR-020 | Output serializable y usable por `inspect_model`. |

---

# EPIC — Indexing Engine

## Objetivo

Implementar conversiones espaciales deterministas para modelos de bloques.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 30 | MR-030 | `[x]` | P0 | Indexing | Implementar `xyz_to_ijk` | Convertir coordenadas espaciales a índices de grilla considerando origen, tamaño de bloque y límites. | `xyz_to_ijk` | MR-015 | Tests exactos para puntos en centro, borde, fuera de grilla y tolerancias. |
| 31 | MR-031 | `[x]` | P0 | Indexing | Implementar `ijk_to_xyz` | Convertir índices `i,j,k` a coordenada de centro de bloque. Debe ser reversible con `xyz_to_ijk` dentro de tolerancia. | `ijk_to_xyz` | MR-030 | Roundtrip `ijk -> xyz -> ijk` pasa en grillas representativas. |
| 32 | MR-032 | `[x]` | P0 | Indexing | Implementar indexación lineal | Convertir `i,j,k` a índice lineal y viceversa, con detección de overflow. | Linear indexing | MR-031 | Tests cubren orden de indexación documentado y modelos grandes simulados. |
| 33 | MR-033 | `[x]` | P1 | Indexing | Soportar grillas rotadas | Incorporar matriz/ángulo de rotación con tolerancias explícitas para conversiones espaciales. | Rotation engine | MR-031 | Error espacial bajo tolerancia documentada en casos rotados conocidos. |
| 34 | MR-034 | `[x]` | P1 | Indexing | Vecindad de bloques | Exponer funciones para obtener vecinos 6/18/26 conectados respetando límites y sparse opcional. | Neighbor API | MR-032 | Tests cubren bloques esquina, borde, interior y sparse. |

---

# EPIC — IO e interoperabilidad

## Objetivo

Permitir entrada/salida confiable con formatos reales y abiertos.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 40 | MR-040 | `[x]` | P0 | IO | CSV read/write | Leer y escribir modelos desde CSV con schema explícito para columnas de coordenadas, índices y atributos. | CSV IO | MR-020 | Roundtrip CSV preserva datos básicos; errores de columnas faltantes son claros. |
| 41 | MR-041 | `[x]` | P0 | IO | Parquet read/write | Leer y escribir Parquet preservando tipos, metadata y compatibilidad con Python/Arrow. | Parquet IO | MR-021 | Roundtrip Parquet preserva schema y metadata; archivo puede leerse desde Python. |
| 42 | MR-042 | `[x]` | P1 | IO | Arrow IPC | Exponer intercambio Arrow IPC o RecordBatch para integración eficiente con Python. | Arrow backend | MR-041 | Conversión Arrow evita copias innecesarias cuando sea viable y documenta cuándo copia. |
| 43 | MR-043 | `[x]` | P1 | IO | Inferencia controlada de schema | Inferir schema desde archivos cuando sea razonable, pero exigir confirmación o parámetros para columnas críticas como ley, tonelaje y coordenadas. | Schema inference | MR-040 | No se asumen columnas críticas silenciosamente; warnings/errores son estructurados. |
| 44 | MR-044 | `[x]` | P1 | IO | Export VTK/VTU | Exportar `BlockModel` regulares a VTU ASCII con geometría hexaédrica y atributos seleccionados compatibles con VTK para visualización en ParaView. | VTU exporter | MR-020 | El archivo abre en ParaView con los atributos esperados; errores claros cubren columnas `text` no soportadas y grillas rotadas. |
| 45 | MR-045 | `[x]` | P2 | IO | Export compatible Vulcan CSV | Generar CSV con convenciones configurables para importación en workflows Vulcan. | Vulcan CSV exporter | MR-040 | Export produce columnas y unidades documentadas; ejemplo de importación queda documentado. |
| 46 | MR-046 | `[!]` | P2 | IO | Soporte BDF experimental | Investigar y prototipar soporte BDF si existen especificaciones o ejemplos válidos. La documentación pública encontrada solo describe el comando propietario `breverse` para extraer `.bdf` desde `.bmf`, sin especificar el formato. | BDF prototype | MR-045 | Se documentan limitaciones; no se promete compatibilidad completa sin validación externa. |

---

# EPIC — Validation Engine

## Objetivo

Detectar problemas estructurales, espaciales y de schema antes de cálculos mineros.

Nota de dependencia actual:

Tras introducir la layout sparse experimental, la detección de duplicados ya vive sobre artefactos con índices explícitos o coordenadas normalizadas antes de construir `BlockModel`. En `mine-io`, los exportes CSV ya respetan filas materializadas sparse, mientras que Arrow/VTU rechazan explícitamente sparse hasta definir una representación columnar/geometría equivalente. En `mine-validation`, los extents observados ya se validan contra el envelope nominal de grilla con soporte de rotación XY y warnings explícitos para coberturas parciales.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 50 | MR-050 | `[x]` | P0 | Validation | Diseñar `ValidationReport` | Crear contrato estructurado con severidad, código, mensaje, ubicación, conteo, recomendación y metadata. | `ValidationReport` | MR-010 | Report serializa a JSON y puede combinar múltiples issues. |
| 51 | MR-051 | `[x]` | P0 | Validation | Duplicate detection | Detectar bloques duplicados por `ijk` o coordenada normalizada según grilla. | Duplicate validator | MR-030, MR-050 | Casos duplicados se reportan con conteo y ejemplos de bloques afectados. |
| 52 | MR-052 | `[x]` | P0 | Validation | Regular grid validation | Validar que los bloques esperados correspondan a la grilla definida y que sus índices sean consistentes. | Grid validator | MR-032, MR-050 | Detecta índices fuera de rango y coordenadas inconsistentes. |
| 53 | MR-053 | `[x]` | P1 | Validation | Missing blocks detection | Detectar gaps internos en modelos regulares y diferenciarlos de sparse permitido. | Gap validator | MR-052 | Reporta gaps por rango/índice y permite configurar tolerancia o modo sparse. |
| 54 | MR-054 | `[x]` | P1 | Validation | Extents validation | Validar extents esperados vs reales, incluyendo tolerancia y rotación cuando aplique. | Extents validator | MR-052 | Warnings/errores distinguen extents incompletos, desplazados o sobredimensionados. |
| 55 | MR-055 | `[x]` | P1 | Validation | Schema validation | Validar columnas requeridas, tipos, unidades y roles mineros. | Schema validator | MR-017, MR-050 | Reporta columnas faltantes, tipos incompatibles y unidades ambiguas. |
| 56 | MR-056 | `[x]` | P1 | Validation | Value validation | Validar rangos de valores críticos: tonelaje no negativo, densidad positiva, leyes finitas, recuperaciones 0-1. | Value validator | MR-055 | Valores inválidos se reportan por columna y severidad configurable. |
| 57 | MR-057 | `[x]` | P1 | Validation | API de validación completa | Exponer `model.validate()` que ejecute suite configurable y devuelva `ValidationReport`. | Validation API | MR-052, MR-055, MR-056 | `model.validate()` produce reporte completo y permite seleccionar validadores. |

---

# EPIC — Analytics y economía base

## Objetivo

Entregar cálculos mineros útiles, reproducibles y auditables.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 60 | MR-060 | `[x]` | P1 | Analytics | Estadísticas básicas | Calcular conteos, tonelaje, ley media ponderada, metal contenido y nulos por columna. | Basic stats | MR-020, MR-055 | Tests con dataset pequeño dan resultados manualmente verificables. |
| 61 | MR-061 | `[x]` | P1 | Analytics | Agregación por grupo | Agrupar por dominio, banco, fase u otra columna categórica con reglas explícitas. | Grouped stats | MR-060 | Tonelaje/metal se suman y leyes se ponderan correctamente. |
| 62 | MR-062 | `[x]` | P1 | Analytics | Curva ley-tonelaje | Calcular tabla por cutoff con tonelaje, ley media, metal y porcentaje acumulado. | Grade-tonnage engine | MR-061 | Curva coincide con cálculo manual en fixture; cutoffs quedan ordenados y documentados. |
| 63 | MR-063 | `[x]` | P2 | Economics | Modelo de supuestos económicos | Definir precio, recuperación, costos y unidades con validación explícita. | Economic assumptions | MR-060 | Supuestos serializan y rechazan valores inválidos. |
| 64 | MR-064 | `[x]` | P2 | Economics | Revenue y margen por bloque | Calcular revenue, costo y margen por bloque usando supuestos explícitos. | Block economics | MR-063 | Resultados reproducibles con tests; fórmulas quedan documentadas. |
| 65 | MR-065 | `[x]` | P2 | Economics | Cashflow y NPV por escenario | Calcular cashflow por periodo y NPV para escenarios de secuencia. | NPV engine | MR-064, MR-092 | NPV coincide con fixture financiero y expone supuestos. |

---

# EPIC — Reblocking Engine

## Objetivo

Transformar resolución de modelos preservando masa, metal y trazabilidad.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 70 | MR-070 | `[x]` | P1 | Reblock | Diseñar reglas de agregación | Crear API declarativa para suma, promedio ponderado, min, max, first, majority y reglas custom limitadas. | `AggregationRules` | MR-020 | Reglas validan columnas requeridas y rechazan agregaciones inseguras. |
| 71 | MR-071 | `[x]` | P1 | Reblock | Superblocking | Agrupar bloques finos en bloques mayores usando reglas explícitas y grilla destino. | Superblock engine | MR-070, MR-032 | Conserva tonelaje y metal dentro de tolerancia documentada. |
| 72 | MR-072 | `[x]` | P1 | Reblock | Weighted aggregation | Implementar agregación ponderada reusable para leyes, densidades y variables continuas. | Weighted aggregation | MR-070 | Tests cubren pesos cero, nulos y columnas faltantes. |
| 73 | MR-073 | `[x]` | P2 | Reblock | Subblocking | Dividir bloques grandes en bloques menores con distribución configurable de atributos. | Subblock engine | MR-071 | Resultado es determinista y conserva variables conservativas. |
| 74 | MR-074 | `[x]` | P2 | Reblock | Reconciliation report | Comparar before/after con métricas de masa, metal, ley media, bloques y diferencias absolutas/relativas. | Reconciliation report | MR-071, MR-073 | Reporte serializable cuantifica diferencias y marca tolerancias excedidas. |
| 75 | MR-075 | `[x]` | P2 | Reblock | Adaptive reblocking experimental | Investigar reblocking variable por zonas, densidad de información o dominios. | Adaptive prototype | MR-074 | Queda marcado experimental; incluye limitaciones y tests mínimos. |

---

# EPIC — Python SDK

## Objetivo

Crear una experiencia Python-first para ingenieros de minas.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 80 | MR-080 | `[x]` | P0 | Python | Configurar `mine-python` con PyO3 | Crear módulo nativo mínimo compilable y conectado con `python/miners`. | Binding base | MR-007 | `import miners` funciona en entorno local y expone versión. |
| 81 | MR-081 | `[x]` | P1 | Python | Exponer tipos core | Exponer `Coordinate3D`, `BlockDimensions`, `GridDefinition` y errores específicos a Python. | Core Python API | MR-015, MR-080 | Python puede construir tipos y recibir excepciones específicas. |
| 82 | MR-082 | `[x]` | P1 | Python | Exponer `BlockModel` | Crear wrapper Python para cargar/construir modelos, consultar schema, metadata y summary. | Python `BlockModel` | MR-025, MR-081 | Ejemplo Python construye modelo pequeño y muestra summary. |
| 83 | MR-083 | `[x]` | P1 | Python | Interoperabilidad pandas | Implementar `from_pandas` y `to_pandas` preservando tipos y metadata externa cuando aplique. | Pandas bridge | MR-082 | Roundtrip pandas funciona en tests; columnas críticas no se infieren silenciosamente. |
| 84 | MR-084 | `[x]` | P2 | Python | Interoperabilidad numpy | Exponer columnas numéricas y booleanas como arrays y aceptar arrays `numpy` para construir columnas nuevas cuando sea seguro. | Numpy bridge | MR-083 | Tests validan tipos, shape y comportamiento de copias entre Rust y Python. |
| 85 | MR-085 | `[x]` | P1 | Python | API de validación Python | Exponer `model.validate()`, `ValidationReport`, `to_json()` y `to_pandas()`. | Python validation API | MR-057, MR-082 | Notebook/script puede validar modelo y tabular issues. |
| 86 | MR-086 | `[x]` | P1 | Python | API analytics Python | Exponer estadísticas básicas y curva ley-tonelaje con nombres mineros claros. | Python analytics API | MR-062, MR-085 | Ejemplo Python calcula curva ley-tonelaje con fixture. |
| 87 | MR-087 | `[x]` | P2 | Python | API fluida experimental | Diseñar una API estilo dataframe/mining workflow sin ocultar supuestos críticos. | Fluent API prototype | MR-086 | API queda marcada experimental y documentada con límites. |

---

# EPIC — Planning primitives

## Objetivo

Construir primitives reutilizables para planeamiento, pushbacks y secuencias.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 90 | MR-090 | `[x]` | P1 | Planning | Bench generation | Generar bancos desde grilla/modelo usando altura de banco, origen y tolerancias. | Bench engine | MR-020, MR-032 | Bloques se asignan a banco correcto en fixtures. |
| 91 | MR-091 | `[x]` | P1 | Planning | Phase tagging | Asignar fase, pushback o shell desde columna existente o reglas geométricas simples. | Phase engine | MR-090 | Bloques reciben fase válida y reporte de no asignados. |
| 92 | MR-092 | `[x]` | P2 | Planning | Scenario model | Definir `MiningScenario` con periodos, reglas, restricciones, supuestos y referencias a modelo. | Scenario model | MR-091 | Escenario serializa a JSON y valida campos obligatorios. |
| 93 | MR-093 | `[x]` | P2 | Planning | Precedence graph | Crear grafo de precedencias entre bloques/bancos/fases para restricciones de minado. | DAG engine | MR-092, MR-034 | Grafo es acíclico o reporta ciclos; tests cubren precedencias simples. |
| 94 | MR-094 | `[x]` | P2 | Planning | Reglas de avance vertical | Implementar restricciones de avance vertical máximo entre bancos/fases. | Vertical constraints | MR-093 | Secuencias inválidas reportan violaciones con ubicación. |
| 95 | MR-095 | `[x]` | P2 | Planning | Schedule primitives | Crear primitives para asignar bloques/tonelaje a periodos con restricciones básicas. | Schedule API | MR-094 | Schedule simple reproduce tonelajes por periodo y reporta violaciones. |
| 96 | MR-096 | `[x]` | P3 | Planning | Pushback design experimental | Investigar representación de pushbacks y reglas de generación/evaluación sin prometer optimización completa. | Pushback prototype | MR-095 | Documento/prototipo define alcance, limitaciones y siguientes pasos. |

---

# EPIC — Deterministic Tools

## Objetivo

Exponer capacidades del SDK como tools serializables para automatización y agentes.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 110 | MR-110 | `[x]` | P1 | Tools | Definir contrato común de tools | Crear estructura común para input, output, errores, metadata de ejecución, versión y referencias a artefactos. | Tool contract base | MR-025, MR-050 | Contrato serializa a JSON y puede validarse en tests. |
| 111 | MR-111 | `[x]` | P1 | Tools | `inspect_model` | Tool para perfilar modelo: shape, extents, columnas, tipos, nulos, metadata y advertencias iniciales. | `inspect_model` | MR-025, MR-110 | Output JSON estable y validado contra fixture. |
| 112 | MR-112 | `[x]` | P1 | Tools | `validate_model` | Tool para ejecutar validaciones y emitir `ValidationReport` estructurado. | `validate_model` | MR-057, MR-110 | Reporte JSON contiene severidades, códigos y conteos. |
| 113 | MR-113 | `[x]` | P1 | Tools | `query_blocks` | Tool para filtrar y seleccionar bloques/columnas con límites para evitar outputs gigantes. | `query_blocks` | MR-023, MR-110 | Filtros funcionan y la tool reporta truncamiento/paginación. |
| 114 | MR-114 | `[x]` | P1 | Tools | `aggregate_blocks` | Tool para agregaciones por dominio, banco, fase o filtros con reglas explícitas. | `aggregate_blocks` | MR-061, MR-113 | Resultados coinciden con analytics core y se serializan. |
| 115 | MR-115 | `[x]` | P1 | Tools | `grade_tonnage` | Tool para curva ley-tonelaje con columnas, cutoffs y unidades explícitas. | `grade_tonnage` | MR-062, MR-110 | Output contiene tabla, resumen y supuestos usados. |
| 116 | MR-116 | `[x]` | P2 | Tools | `create_scenario` | Tool para crear escenario básico desde reglas de planeamiento y referencias a modelo. | `create_scenario` | MR-092, MR-110 | Escenario válido se guarda/serializa; errores explican campos faltantes. |
| 117 | MR-117 | `[x]` | P2 | Tools | `evaluate_scenario` | Tool para evaluar cashflow y NPV de un escenario usando inputs financieros explícitos por periodo, sin inferir granularidad no disponible en `Schedule`. | `evaluate_scenario` | MR-065, MR-116 | Resultados reproducibles, serializables y trazables a los inputs financieros suministrados. |
| 118 | MR-118 | `[x]` | P2 | Tools | `compare_scenarios` | Tool para comparar dos evaluaciones estructuradas de escenario y resumir diferencias de cashflow y NPV por periodo y en total. | `compare_scenarios` | MR-117 | Comparación JSON incluye diferencias totales, preferencia por NPV y detalle por periodo cuando existe en ambos reportes. |

---

# EPIC — Refactorización técnica y orden del proyecto

## Objetivo

Reducir deuda estructural antes de que el crecimiento del SDK vuelva costoso navegar, testear y mantener el workspace.

Hallazgos principales de la revisión:

* crates con `src/lib.rs` monolíticos (`mine-io` 2484 líneas, `mine-reblock` 2405, `mine-validation` 1574, `mine-planning` 1414, `mine-tools` 1366, `mine-blockmodel` 1255 y `mine-python` 1253);
* tests de integración mezclados dentro de `src/` en casi todos los crates;
* `cargo nextest` ya está disponible localmente, pero la CI sigue ejecutando `cargo test --workspace`;
* `mine-sdk` reexporta la superficie completa en un namespace plano sin agrupación por dominio.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 119 | MR-119 | `[x]` | P1 | Repo | Adoptar `cargo-nextest` | Migrar verificación principal de tests a `cargo nextest run --workspace`, actualizar CI y documentar el flujo recomendado. | Baseline `nextest` | MR-004 | La CI instala `cargo-nextest`, ejecuta `cargo nextest run --workspace` y README documenta el comando como verificación principal. |
| 120 | MR-120 | `[x]` | P1 | Repo | Definir layout de tests por crate | Establecer convención explícita: tests unitarios mínimos en `src/`, tests de integración en `crates/*/tests/`, helpers compartidos en `tests/common`. | Convención de testing | MR-119 | Existe documentación breve de la convención y cada crate tiene directorio `tests/` listo para migración incremental. |
| 121 | MR-121 | `[x]` | P1 | IO | Modularizar `mine-io` | Dividir `mine-io/src/lib.rs` en módulos por formato (`csv`, `parquet`, `vtu`, `vulcan`) y mover tests de roundtrip/IO completo a `tests/`. | `mine-io` modular | MR-120 | `lib.rs` queda como fachada pequeña, la API pública no cambia y las pruebas pasan con `cargo nextest run -p mine-io`. |
| 122 | MR-122 | `[x]` | P1 | Reblock | Modularizar `mine-reblock` | Separar agregación, distribución, reconciliación y prototipos experimentales en módulos internos, con tests end-to-end fuera de `src/`. | `mine-reblock` modular | MR-120 | `lib.rs` deja de concentrar toda la implementación, la API pública se preserva y las pruebas pasan con `cargo nextest run -p mine-reblock`. |
| 123 | MR-123 | `[x]` | P1 | Validation | Modularizar `mine-validation` | Separar reportes, opciones y validadores en módulos internos y mover los tests de flujo público a `tests/`. | `mine-validation` modular | MR-120 | La crate queda navegable por dominio interno, mantiene compatibilidad pública y las pruebas pasan con `cargo nextest run -p mine-validation`. |
| 124 | MR-124 | `[x]` | P1 | Planning | Modularizar `mine-planning` | Separar bancos, fases, precedencias, scheduling y prototipo de pushbacks en módulos coherentes con tests de integración por feature. | `mine-planning` modular | MR-120 | Cada subdominio vive en su módulo, `lib.rs` queda como fachada y las pruebas pasan con `cargo nextest run -p mine-planning`. |
| 125 | MR-125 | `[x]` | P1 | Tools | Modularizar `mine-tools` | Separar contratos, wrappers de validación, analytics y planeamiento en módulos internos para reducir acoplamiento y facilitar crecimiento. | `mine-tools` modular | MR-120 | Los contratos y tools quedan organizados por dominio, la API no se rompe y las pruebas pasan con `cargo nextest run -p mine-tools`. |
| 126 | MR-126 | `[x]` | P2 | BlockModel | Modularizar `mine-blockmodel` | Separar layout, selección, filtros, summaries y analytics base en módulos internos, dejando `lib.rs` como punto de entrada reducido. | `mine-blockmodel` modular | MR-120 | Se reducen responsabilidades por archivo, la API pública se mantiene y las pruebas pasan con `cargo nextest run -p mine-blockmodel`. |
| 127 | MR-127 | `[x]` | P2 | Python | Modularizar `mine-python` | Separar bindings por dominio (`core`, `blockmodel`, `validation`, `analytics`, `io`) y mantener la ergonomía experimental en `python/miners`, no en el binding Rust. | `mine-python` modular | MR-120 | `mine-python/src/lib.rs` deja de ser archivo monolítico, los bindings siguen importables y los tests Python continúan pasando. |
| 128 | MR-128 | `[x]` | P2 | SDK | Ordenar reexports de `mine-sdk` | Agrupar la fachada pública por dominios internos y reducir el namespace plano sin romper compatibilidad. | Fachada SDK ordenada | MR-121, MR-122, MR-123, MR-124, MR-125, MR-126 | `mine-sdk` expone módulos por dominio, conserva reexports de conveniencia y README muestra la organización recomendada. |

---

# EPIC — Benchmarks Marvin y outputs abiertos

## Objetivo

Usar instancias Marvin como benchmark reproducible para validar ingestión, comparación y, por olas, avanzar hacia generación propia de `prec` y `upit` sin depender de formatos propietarios no documentados.

Estado actual del SDK frente a este frente:

* ya existe staging local reproducible en `datasets/benchmarks/marvin/` con `marvin.blocks` y `manifest.json`;
* `mine-io` ya carga `blocks`, `mine-planning` ya genera un `prec` determinista y expone IO JSON abierto para precedencias, y el workspace ya tiene examples `marvin-inspect` y `marvin-prec`;
* la URL pública indicada de Marvin no fue verificable automáticamente porque hoy responde detrás de un WAF, así que los outputs externos adicionales siguen dependiendo de artefactos abiertos/provistos localmente antes de asumir formatos exactos.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 129 | MR-129 | `[x]` | P1 | Benchmarks | Inventario de artefactos Marvin | Definir un manifiesto reproducible para instancias Marvin y sus artefactos abiertos/provistos localmente (`blocks`, `prec`, `upit` y otros outputs publicados cuando existan), incluyendo origen, licencia, checksum, formato detectado y ubicación canónica en `datasets/benchmarks/marvin/`. | Manifiesto Marvin | MR-001 | Existe inventario versionado con nombres de instancia, artefactos disponibles, notas de acceso y límites de uso; no se asumen formatos no verificados. |
| 130 | MR-130 | `[x]` | P1 | IO | Ingesta de `blocks` Marvin a `BlockModel` | Crear la ruta reproducible para cargar los `blocks` de Marvin al storage actual de `BlockModel`, con schema explícito, metadata de benchmark y validación estructurada. | Loader Marvin blocks | MR-129 | Una o más instancias Marvin cargan a `BlockModel`, pasan validación y documentan el mapeo de columnas mineras. |
| 131 | MR-131 | `[x]` | P1 | Examples | Ejemplos reproducibles Marvin | Crear examples ejecutables para cargar instancias, inspeccionarlas, validar estructura y producir salidas base comparables. Los ejemplos deben usar artefactos locales/manifiesto y no depender de scraping online. | Examples Marvin | MR-130 | Existe al menos un flujo reproducible documentado desde `blocks` hacia summary, validación y analytics básicos. |
| 132 | MR-132 | `[x]` | P1 | IO | Normalización de outputs externos Marvin | Implementar importadores/normalizadores para outputs de referencia disponibles (`prec`, `upit` y otros artefactos benchmark presentes en `datasets/benchmarks/marvin/`), llevándolos a contratos abiertos y serializables dentro del repo. Ya existen normalizadores reproducibles para `marvin.prec` y `marvin_upit.sol`; quedan pendientes `cpit`, `pcpsp` y las relajaciones LP. | Contratos benchmark normalizados | MR-129 | Cada output soportado tiene parser explícito, validación, fixture y representación interna documentada. |
| 133 | MR-133 | `[x]` | P1 | Benchmarks | Comparador de resultados benchmark | Construir comparadores reproducibles entre outputs Marvin externos y outputs generados por `mine-rs` para conteos, membresía de bloques, aristas de precedencia, tonelaje/metal y métricas derivadas. Existe reporte versionado en `datasets/benchmarks/marvin/outputs/comparison-report.json` con `prec` edge_jaccard=1.0, comparación exacta `upit`, auditorías reproducibles de `cpit` / `pcpsp` / LP sobre las referencias locales y comparaciones CPIT/PCPSP contra candidatos propios de scheduling. | Benchmark comparator | MR-130, MR-132 | El comparador produce reportes serializables con coincidencias, diferencias y tolerancias explícitas por instancia. |
| 134 | MR-134 | `[x]` | P2 | Planning | Generación determinista de `prec` | Extender `mine-planning` para generar precedencias de bloque desde grilla, índices lineales y una plantilla explícita de talud/vecindad, reutilizando `PrecedenceGraph` como contrato interno. | Generador `prec` | MR-130, MR-093 | La generación produce un DAG válido, serializable y comparable contra referencias externas cuando existan. |
| 135 | MR-135 | `[x]` | P2 | IO | IO abierto para precedencias benchmark | Exponer lectura/escritura de precedencias benchmark desde/hacia un formato abierto y documentado para examples, tests y comparadores; si el `prec` externo no está completamente especificado, definir un formato normalizado propio además del importador. | IO de precedencias | MR-132, MR-134 | `mine-rs` puede exportar e importar precedencias benchmark de forma estable y los examples consumen el mismo contrato. |
| 136 | MR-136 | `[x]` | P2 | Planning | `upit` experimental abierto | Diseñar e implementar una primera ruta abierta para generar `upit` desde un modelo valuado y sus precedencias, dejando explícito si se trata de solver exacto o heurístico y cuáles son sus límites. | Prototipo `upit` | MR-130, MR-134 | El pipeline genera una shell/pit membership reproducible, documenta supuestos y puede compararse contra referencias Marvin cuando existan. |
| 137 | MR-137 | `[x]` | P2 | Benchmarks | Paridad Marvin por olas | Consolidar examples, comparadores y reportes para medir por instancia cuánto cubre `mine-rs` en cada ola: ingestión, outputs externos, `prec` propio y `upit` experimental. La matriz versionada vive en `datasets/benchmarks/marvin/outputs/parity-report.json` y deja explícitos los gaps que dependen de referencias externas reales y del scheduler propio aún pendiente. | Reporte de paridad Marvin | MR-131, MR-133, MR-135, MR-136 | Existe una matriz de cobertura por instancia/output que diferencia claramente lo ya reproducido, lo comparable y lo que sigue pendiente. |
| 167 | MR-167 | `[x]` | P1 | Benchmarks | Plantilla de talud Marvin 17-offset (45°/8-niveles) | Reverse-engineer y validar la plantilla exacta de precedencias Marvin (45°/8-niveles, 30×30×30m): 5 offsets en dk=1 (cruce cardinal), 4 en dk=3 (esquinas diagonales) y 8 en dk=5 (arco semicircular), totalizando 17 offsets. Aplicar en `marvin-benchmark` para que `edge_jaccard` alcance 1.0. | Plantilla 17-offset validada | MR-134 | `cargo run -p marvin-benchmark` produce `edge_jaccard_index: 1.0` contra `marvin.prec` con los 17 offsets verificados. |
| 168 | MR-168 | `[x]` | P1 | Benchmarks | Valor económico objetivo correcto en benchmark Marvin | Corregir el cálculo de `total_value` del benchmark de `sum(proc_profit)` a `sum(proc_profit × tonnage)`, y agregar `total_economic_objective = sum((max(proc_profit, 0) − mine_cost) × tonnage)` para comparar directamente con el objetivo oficial UPIT (1,415,655,436). | Métricas económicas correctas | MR-133 | El benchmark reporta `reference_total_economic_objective ≈ 1,415,655,436` y el candidato puede compararse en la misma escala. |
| 169 | MR-169 | `[x]` | P1 | IO | Normalizar archivo `marvin.upit` (valores objetivo por bloque) | Agregar parser para `marvin.upit` (formato: `block_id value_objective`) como complemento al `.sol` ya normalizado, exponiendo los valores económicos individuales por bloque para auditar el objetivo UPIT directo. | Parser `marvin.upit` | MR-132 | Existe `read_marvin_upit_block_values()` con tests, fixture de ronda y valores por bloque disponibles para comparar por membresía. |
| 170 | MR-170 | `[x]` | P1 | IO | Normalizar CPIT y PCPSP Marvin | Implementar parsers para `marvin.cpit`, `marvin_cpit_gmunoz120723.sol`, `marvin.pcpsp`, `marvin_pcpsp_gmunoz120723.sol`, `marvin.LPcpit` y `marvin.LPpcpsp`, normalizando membresías y valores como contratos abiertos con tests reproducibles. Los artefactos externos quedan aislados en `datasets/benchmarks/marvin/references/` y el benchmark audita además los objetivos oficiales descontados de CPIT/PCPSP/LP. | Parsers CPIT y PCPSP | MR-132 | Cada artefacto tiene parser documentado, test de fixture y representación interna que puede alimentar comparadores. |
| 171 | MR-171 | `[x]` | P1 | Benchmarks | Comparar CPIT Marvin contra scheduling propio | Una vez exista el scheduler agregado (MR-163), conectar el comparador para evaluar la membresía y métricas de CPIT del scheduler mine-rs contra las referencias Marvin normalizadas. | Comparación CPIT vs scheduler | MR-163, MR-170 | El benchmark reporta jaccard de membresía y métricas de valor/tonelaje por periodo comparadas contra las referencias CPIT Marvin. |
| 172 | MR-172 | `[x]` | P2 | Benchmarks | Comparar PCPSP Marvin contra scheduling con restricciones | Una vez exista scheduling con capacidades y pushbacks (MR-164), conectar el comparador para evaluar PCPSP mine-rs contra referencias Marvin. | Comparación PCPSP vs scheduler | MR-164, MR-170 | `datasets/benchmarks/marvin/outputs/comparison-report.json` incluye una comparación `mine-rs` vs PCPSP Marvin con proxy de routing por destino, membresía por periodo+destino y métricas agregadas por periodo. |
| 173 | MR-173 | `[x]` | P1 | Planning | Backend exacto UPL escalable | Reemplazar o complementar el backend Edmonds-Karp actual por un algoritmo escalable (push-relabel, pseudoflow o equivalente) para que el solver exacto de UPL pueda correr sobre instancias tipo Marvin sin quedar restringido a fixtures pequeños. | Exact UPL scalable backend | MR-156 | `solve_upl_exact()` usa ahora un backend exacto tipo Dinic, mantiene paridad con los fixtures previos y corre la validación exacta sobre Marvin sin cambiar la API pública del solver. |
| 174 | MR-174 | `[x]` | P1 | Benchmarks | Comparar UPL exacto contra UPIT Marvin | Conectar el backend exacto escalable al benchmark Marvin usando el objetivo económico correcto por bloque para comparar memberships, tonelaje y valor contra `marvin_upit.sol`. | Exact UPL vs Marvin report | MR-170, MR-173 | `datasets/benchmarks/marvin/outputs/comparison-report.json` incluye la comparación exacta `mine-rs` vs UPIT Marvin usando `marvin.upit` + `marvin.prec`, complementando la ruta heurística actual. |

---

# EPIC — Preparación y estimación de resource model

## Objetivo

Extender `mine-rs` desde un motor de block models existentes hacia una ruta reproducible de preparación y estimación de modelos útiles para economía y planeamiento, siguiendo la secuencia recomendada en `docs/references/mining-engine-roadmap.md`.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 138 | MR-138 | `[x]` | P1 | Estimation | Engine de compositing determinista | Implementar compositing de intervalos con políticas explícitas para longitud objetivo, residuales y splits por dominio, preservando trazabilidad desde muestras fuente. | Composite engine | MR-020, MR-017 | El SDK genera composites reproducibles, serializables y auditables, con tests de residuals, weighting y domain splits. |
| 139 | MR-139 | `[x]` | P1 | Estimation | Domaining duro y auditoría de límites | Incorporar máscaras de dominio, filtros por hard boundaries y reportes de consistencia entre dominios, composites y bloques objetivo. | Domain masking | MR-138 | Los workflows de compositing y estimación pueden restringirse por dominio y reportan explícitamente muestras fuera de dominio o mezclas inválidas. |
| 140 | MR-140 | `[x]` | P1 | Estimation | Declustering y estadísticas ponderadas | Agregar cell declustering con múltiples orígenes, pesos serializables y estadísticas ponderadas por dominio para evitar sesgos de muestreo. | Declustered statistics | MR-139 | El SDK calcula pesos reproducibles, histogramas ponderados y resúmenes comparables contra estadísticas no declusterizadas. |
| 141 | MR-141 | `[x]` | P1 | Estimation | Variografía experimental | Construir variogramas experimentales omni y direccionales con lagging explícito, tolerancias y artefactos serializables por dominio/variable. | Experimental variograms | MR-140 | Se pueden generar semivariogramas reproducibles con tests de binning, anisotropía básica y outputs JSON/Parquet. |
| 142 | MR-142 | `[x]` | P1 | Estimation | Librería de modelos variográficos | Implementar modelos autorizados básicos (spherical, exponential, gaussian, nugget) y fitting con restricciones explícitas. | Variogram model library | MR-141 | El SDK ajusta modelos válidos, reporta parámetros y rechaza configuraciones no autorizadas o no físicas. |
| 143 | MR-143 | `[x]` | P1 | Estimation | Neighborhoods y estimation passes | Definir contratos para vecindades de búsqueda, máximos/mínimos de muestras, anisotropía y passes de estimación reproducibles. | Search neighborhood API | MR-142 | Las reglas de búsqueda son serializables, reutilizables entre estimadores y tienen tests de selección espacial y prioridades por pass. |
| 144 | MR-144 | `[x]` | P1 | Estimation | Estimadores deterministas base | Implementar nearest neighbour e inverse distance weighting sobre la nueva API de neighborhoods y dominios. | NN + IDW estimators | MR-143 | El SDK estima bloques con NN/IDW, documenta supuestos y valida resultados en fixtures pequeños conocidos. |
| 145 | MR-145 | `[x]` | P1 | Estimation | Ordinary y simple kriging | Añadir ordinary kriging como ruta principal y simple kriging opcional cuando el mean sea explícito, usando modelos variográficos ajustados. | Kriging estimators | MR-142, MR-143 | El SDK resuelve OK/SK en casos pequeños, reporta pesos/varianzas y pasa cross-validation básica. |
| 146 | MR-146 | `[x]` | P1 | Estimation | Regularización de soporte de bloque | Incorporar point-to-block y block-to-block covariance para block kriging y regularización lineal del soporte. | Block support regularization | MR-145 | Las estimaciones de bloque usan soporte explícito, documentan discretización y muestran diferencias verificables contra estimación puntual. |
| 147 | MR-147 | `[x]` | P1 | Estimation | Validación del modelo estimado | Construir suite de cross-validation, swath plots, comparación composite-vs-block y reportes de calidad de estimación. | Estimation validation suite | MR-145, MR-146 | Existe un `ValidationReport` específico para estimación con métricas, plots/tablas serializables y ejemplos reproducibles. |
| 148 | MR-148 | `[x]` | P2 | Estimation | Métricas explícitas de clasificación | Implementar un motor configurable de métricas para clasificación de recursos basado en sample spacing, informedness y continuidad, sin automatizar compliance. | Classification metrics engine | MR-147 | El SDK produce evidencia estructurada y audit trail para clasificación, diferenciando claramente métricas calculadas de decisiones profesionales externas. |

---

# EPIC — Economic block model y valorización multi-destino

## Objetivo

Pasar de un block model validado/estimado a un `EconomicBlockModel` reproducible que pueda alimentar pit final, pushbacks y scheduling, con fórmulas explícitas y destinos mineros auditables.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 149 | MR-149 | `[x]` | P1 | Economics | Contratos de supuestos por destino | Diseñar contratos explícitos para destinos como waste, mill, leach, stockpile y sell, incluyendo recoveries, payabilities, costos y capacidades. | Destination assumptions | MR-017, MR-147 | Los supuestos son serializables, tipados y reutilizables por valuación, scheduling y comparación de escenarios. |
| 150 | MR-150 | `[x]` | P1 | Economics | Fórmulas NSR y equivalent value | Implementar biblioteca explícita de fórmulas para NSR, equivalent value y cutoff-related metrics sin economía implícita. | NSR / EV formulas | MR-149 | El SDK calcula NSR/equivalent value con pruebas de fórmulas, unidades y sensitivities simples. |
| 151 | MR-151 | `[x]` | P1 | Economics | Valorización multi-destino por bloque | Extender la economía actual para calcular revenue, costo, margen y valor por bloque para múltiples destinos y reglas de selección explícitas. | Destination-aware block valuation | MR-149, MR-150 | Cada bloque puede evaluarse contra varios destinos, el artefacto es serializable y la selección de destino queda auditada. |
| 152 | MR-152 | `[x]` | P2 | Economics | Primitives de stockpile y blending | Definir balances de stockpile, reclaim, degradación futura opcional y reportes mínimos de mezcla/destino sin entrar todavía en optimización completa. | Stockpile primitives | MR-151 | Existen contratos y cálculos base de balance por periodo/destino con errores explícitos y sin defaults silenciosos. |
| 153 | MR-153 | `[x]` | P1 | Economics | `EconomicBlockModel` integrado | Crear un artefacto estable que combine block model, supuestos, destinos y valores derivados como input estándar para pit y scheduling. | `EconomicBlockModel` | MR-151 | El artefacto preserva lineage, metadata, unidades y columnas económicas derivadas, y puede persistirse en formatos abiertos. |

---

# EPIC — Pit final exacto, shells anidados y métricas de pit

## Objetivo

Reemplazar el `upit` heurístico actual por una ruta exacta y benchmarkeable basada en max-closure / max-flow, con soporte para taludes variables y shells anidados.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 154 | MR-154 | `[x]` | P1 | Planning | Plantillas de talud variables | Extender la generación de precedencias para soportar plantillas geotécnicas y slope templates más generales que offsets fijos mínimos. | Variable slope templates | MR-134, MR-153 | El SDK puede construir DAGs de precedencia desde reglas de talud explícitas y compararlas contra fixtures abiertos. |
| 155 | MR-155 | `[x]` | P1 | Planning | Transformación max-closure del pit | Implementar la transformación desde `EconomicBlockModel` + `PrecedenceGraph` a un problema exacto de max-closure / max-flow serializable y reusable. | Max-closure transform | MR-153, MR-154 | El problema transformado conserva audit trail, pesos y precedencias, y puede verificarse con instancias pequeñas conocidas. |
| 156 | MR-156 | `[x]` | P1 | Planning | Backend exacto de ultimate pit | Incorporar un backend exacto inicial para pit final basado en max-flow / pseudoflow o equivalente, detrás de una API estable de solver. | Exact UPL solver | MR-155 | El SDK resuelve instancias de prueba, produce memberships reproducibles y mejora explícitamente sobre el prototipo heurístico actual. |
| 157 | MR-157 | `[x]` | P1 | Planning | Generación de shells anidados | Implementar revenue-factor sweeps o parametric pit limits para producir familias anidadas de shells desde el solver exacto. | `PitShellSet` | MR-156 | El SDK genera shells anidados con metadata del método y métricas por shell, y puede exportarlos a contratos abiertos. |
| 158 | MR-158 | `[x]` | P2 | Planning | Métricas y reportes de pit shells | Añadir tonelaje, metal, strip ratio, valor y deltas shell-to-shell para cada `PitShellSet`. | Pit shell metrics | MR-157 | Cada shell viene con métricas serializables, comparables y listas para reports, examples y benchmark harnesses. |
| 159 | MR-159 | `[x]` | P2 | Benchmarks | IO y benchmarks abiertos de shells | Exponer IO abierto para shells/pit memberships y agregar fixtures/benchmarks sobre MineLib y Marvin cuando haya artefactos verificables. | Shell IO + benchmarks | MR-157, MR-158 | `PitShellSet` tiene roundtrip JSON abierto (`read_pit_shell_set_json` / `write_pit_shell_set_json`) y el benchmark harness Marvin compara memberships y métricas de pit tanto en la ruta heurística como en la exacta. |

---

# EPIC — Pushbacks, fases y scheduling de largo plazo

## Objetivo

Pasar de shells y artefactos económicos a una ruta reproducible de pushbacks, fases y scheduling agregado de largo plazo con restricciones explícitas.

## Diagnóstico actual del gap de scheduling

- La base de pit ya quedó validada contra Marvin: `prec` alcanza paridad exacta y el backend exacto de `upit` reproduce la referencia.
- El gap fuerte aparece aguas abajo, en scheduling: hoy `build_aggregated_long_term_schedule()` llena periodos de forma greedy por fase y capacidad mina, sin optimizar explícitamente descuento, capacidad de planta, selección fina de unidades listas ni ruteo por destino durante la construcción.
- En el benchmark Marvin, el candidato end-to-end todavía depende de fases sintéticas por bandas de bench y el proxy PCPSP asigna destinos después de fijar la membresía por periodo. Eso sirve para medir el gap, pero no debe confundirse con un scheduler mine-agnostic reusable.
- La siguiente ola no es tuning de parámetros Marvin-específicos, sino rediseño del motor de scheduling sobre contratos genéricos, heurísticas con frontera lista y subproblemas explícitos de destino/stockpile.

## Referencias guía para esta ola

- `docs/references/mining-engine-roadmap.md` ya concentra la base bibliográfica relevante para scheduling y stockpiles.
- Referencias clave para esta ola: [R20] Ramazan y Dagdelen (pushbacks), [R21] Tolwinski (heurística de scheduling open pit), [R22] Caccetta y Hill (branch-and-cut), [R23] Lambert et al. (tutorial de formulaciones CPIT), [R24] Moreno et al. (modelos lineales con stockpiles), [R25] Rezakhah y Newman (degradación por stockpiling), [R29] Espinoza et al. / MineLib (benchmarks abiertos), [R30] Cullenbine et al. (sliding window), [R31] Meagher et al. (gap problem), [R32] Fathollahzadeh et al. (review metodológico), [R33] Jélvez et al. (mejoras MineLib), [R34] Muñoz et al. (implementación BZ), [R35] Chicoisne et al. (LP relaxado), [R36] Boland et al. (disaggregación PCPSP) y [R37] Rivera Letelier et al. (MIP con bench-phases geométricos y B&B).
- Además de papers revisados por pares, esta ola puede apoyarse en preprints de arXiv, tutoriales técnicos y bibliografía de práctica actual, siempre distinguiendo claramente el tipo de fuente y su nivel de madurez.
- Cuando se implemente código basado en estas fuentes, dejar referencia explícita a paper, arXiv o fuente técnica en comentarios o docstrings del módulo/función correspondiente.
- Hallazgo actual tras `MR-181`: la validación multi-mine ya demuestra que el contrato core corre sobre Marvin y McLaughlin, y el adapter benchmark dejó de hardcodear una sola agregación: Marvin ya promueve una ruta primaria `nested-shell × bench` acotada y reproducible, mientras McLaughlin mantiene `reference-period × bench` como fallback explícito. Aun así el benchmark sigue siendo **exploratorio** y no **paper-comparable** porque la familia de shells/cuts todavía no reproduce el pipeline bibliográfico completo y la heurística temporal sigue siendo demasiado débil para cerrar el gap con la literatura.
- Criterio para la siguiente ola: antes de seguir tuneando la heurística `ready frontier`, alinear la instancia/objetivo con MineLib y pasar de fases sintéticas a unidades geométricas o agregadas según la bibliografía (pushbacks desde shells anidados, bench-pushback pairs, LP/BZ warm starts, TopoSort/sliding-window/disaggregation).

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 160 | MR-160 | `[x]` | P1 | Planning | Contratos de pushbacks y fases | Diseñar artefactos estables para `PushbackPlan` y `PhaseDesign`, separando shells, fases operativas y reglas de nesting/acceso. | Pushback / phase contracts | MR-157 | Los contratos diferencian shell, pushback y fase, son serializables y no dependen de heurísticas ocultas. |
| 161 | MR-161 | `[x]` | P1 | Planning | Diseño de fases desde shells | Implementar una primera ruta explícita para derivar fases desde shells anidados, benches y reglas de continuidad/precedencia. | Phase design engine | MR-160 | El SDK genera fases auditables desde shells y documenta claramente qué reglas usa y qué limitaciones tiene. |
| 162 | MR-162 | `[x]` | P1 | Planning | Contratos de scheduler de largo plazo | Definir el artefacto `LongTermSchedule` con capacidades mina/planta, precedencias, destinos, stockpiles y violaciones estructuradas. | Long-term schedule contract | MR-160, MR-153 | Existe un contrato serializable que puede representar asignaciones por periodo, destino y fase sin ambigüedad. |
| 163 | MR-163 | `[x]` | P1 | Planning | Scheduler agregado determinista | Implementar una primera ruta determinista de scheduling agregado por bench/phase/shell con restricciones de tonelaje, precedencia y avance vertical. | Aggregated LOM scheduler | MR-161, MR-162 | El scheduler genera periodos reproducibles, reporta violaciones y puede validarse en instancias pequeñas y benchmarks abiertos. |
| 164 | MR-164 | `[x]` | P2 | Planning | Scheduling con destinos y stockpiles | Extender el scheduler para considerar ruteo a destinos, balances de stockpile y reclaim básico sin romper la reproducibilidad. | Destination-aware schedule | MR-152, MR-163 | `LongTermScheduleEntry` soporta reclaim explícito desde stockpile a destino y `evaluate_long_term_schedule_material_flows()` produce balances/violaciones serializables por periodo para destinos, depósitos y reclaim. |
| 165 | MR-165 | `[x]` | P1 | Economics | Evaluación económica de schedule | Conectar `LongTermSchedule` con cashflow, NPV, metal y métricas de negocio por periodo usando el `EconomicBlockModel`. | LOM economic evaluator | MR-153, MR-163 | Un usuario puede evaluar un schedule completo y obtener KPIs técnicos/económicos reproducibles por periodo y escenario. |
| 166 | MR-166 | `[x]` | P1 | Economics | Packs de sensibilidad y escenarios | Agregar packs de sensibilidad para precio, recovery, costos, capacidades y reglas de scheduling, preservando comparación serializable entre escenarios. | Scenario sensitivity packs | MR-165 | El SDK ejecuta escenarios parametrizados y produce `ScenarioComparisonReport` con deltas claros de NPV, cashflow y producción. |
| 167 | MR-167 | `[x]` | P2 | Benchmarks | Harness end-to-end de MineLib | Construir un harness end-to-end desde `EconomicBlockModel` hasta `LongTermSchedule` sobre instancias abiertas de MineLib/Newman y paridad Marvin cuando sea posible. | End-to-end benchmark harness | MR-159, MR-163, MR-165 | Existen ejemplos y comparadores por etapa para pit, shells, pushbacks, schedule y reportes económicos. |
| 175 | MR-175 | `[x]` | P1 | Planning | Contrato genérico de `SchedulingProblem` | Reemplazar supuestos implícitos del scheduler agregado actual por un contrato explícito para unidades programables, precedencias temporales, recursos, destinos, stockpiles, descuento y objetivo. Ya existe `SchedulingProblem` en `mine-planning`/`mine-sdk`, con `SchedulingPeriod`, `SchedulingUnit`, términos de objetivo, requerimientos de recursos y adapter desde `PushbackPlan` + capacidades, dejando explícito el `gap problem` entre shells/pushbacks y schedule práctico descrito en [R31]. | Scheduling problem contract | MR-162, MR-165 | Existe un contrato serializable y reusable que puede poblarse desde Marvin y otra mina/instancia abierta sin ramas por dataset ni bandas sintéticas codificadas en el core. |
| 176 | MR-176 | `[x]` | P1 | Planning | Baseline exacta/LP para scheduling pequeño | Implementar una baseline exacta o LP/MILP para instancias pequeñas de scheduling con precedencias y capacidades, usada para validar heurísticas y medir gaps. Ya existe `solve_small_scheduling_problem()` sobre `SchedulingProblem`, con precedencias, cotas por recurso, objetivo descontado y destino opcional, documentado con referencias a [R22] y [R23]. | Small-instance scheduling baseline | MR-175, MR-133 | El repo resuelve fixtures pequeños con objetivo y restricciones auditables y reporta gap entre baseline exacta y heurísticas sobre casos sintéticos o benchmarks reducidos. |
| 177 | MR-177 | `[x]` | P1 | Planning | Heurística de frontera lista con valor descontado | Sustituir la política greedy por fase/periodo por una heurística determinista que seleccione unidades listas (`ready frontier`) usando valor descontado, precedencias temporales, límites mina/planta y desempates explícitos. Ya existe `build_ready_frontier_schedule()` sobre `SchedulingProblem`, con valoración descontada, recursos explícitos y soporte de destino opcional, dejando abierta una variante rolling-horizon/sliding-window para periodos consecutivos. | Frontier-aware long-term scheduler | MR-175 | El scheduler decide con objetivo y recursos durante la construcción, mejora el baseline actual en benchmarks abiertos y no asume una mina específica. |
| 178 | MR-178 | `[x]` | P1 | Planning | Ruteo por destino integrado al motor | Integrar `max_plant_tonnage`, capacidades por destino y objetivo multi-destino dentro del motor de scheduling; hoy esos límites aparecen sobre todo como evaluación o proxy externo. La decisión de destino por periodo debe modelarse como subproblema explícito siguiendo [R23] y [R24]. Ya existe `build_ready_frontier_long_term_schedule()` sobre `SchedulingProblem`, que deriva requerimientos mínimos de mina/planta/destino cuando faltan, materializa `LongTermSchedule` con `destination_id` durante la construcción y quedó conectado al benchmark Marvin mediante un adapter que normaliza CPIT/PCPSP a un problema ruteable por fases sintéticas chunked. | Destination-aware scheduling kernel | MR-175, MR-177, MR-164 | Las decisiones del scheduler incluyen destino factible durante la construcción y el benchmark mejora valor/uso de capacidades sin depender de asignación post-hoc. |
| 179 | MR-179 | `[x]` | P2 | Planning | Políticas explícitas de stockpile y reclaim | Extender el core con inventario carryover, reclaim, límites por stockpile y políticas configurables que no dependan de un caso particular. La ruta debe apoyarse en [R24] y [R25], dejando abierta la incorporación posterior de degradación o ageing sin romper contratos. Ya existe `stockpile_policy.rs` con `LongTermStockpilePolicy`, reglas explícitas de depósito/reclaim por periodo y `apply_long_term_stockpile_policy()` sobre `LongTermSchedule`, reutilizando balances/violaciones serializables del contrato existente sin acoplar la lógica a Marvin ni a una mina específica. | Stockpile policy engine | MR-178 | El scheduler soporta depósito/reclaim con balances por periodo y reglas explícitas; activar o desactivar stockpile cambia inputs y no la forma del algoritmo base. |
| 180 | MR-180 | `[x]` | P2 | Planning | Arquitectura decomposed para CPIT/PCPSP | Introducir una capa de descomposición que separe selección temporal, ruteo por destino y stockpiles, permitiendo bounds o warm starts desde relajaciones antes de aspirar a un solver industrial. Ya existe `decomposed_scheduling.rs` con `DecomposedSchedulingConfig`, `DecomposedSchedulingArtifacts` y `solve_decomposed_scheduling_problem()`, que separa candidato temporal, materialización/ruteo y política opcional de stockpile, además de exponer un bound de referencia opcional mediante la baseline exacta pequeña para MVPs y tests; las relajaciones LP/sliding-window/disaggregation de [R34], [R35] y [R36] quedan como la siguiente etapa, no como supuestos implícitos. | Decomposed scheduling architecture | MR-176, MR-178, MR-179 | Existe una ruta reproducible que entrega candidato + bound/relajación y escala mejor que la baseline exacta sin perder auditabilidad ni acoplarse a Marvin. |
| 181 | MR-181 | `[x]` | P1 | Benchmarks | Validación multi-mine del scheduler | Consolidar una matriz de verificación sobre Marvin y al menos otra instancia abierta (MineLib/Newman u otra equivalente) para asegurar que el scheduler usa el mismo contrato genérico y no supuestos Marvin-específicos. [R29] debe ser la referencia mínima para el set abierto. Ya existe el binario `cargo run -p marvin-benchmark --bin multi_mine_scheduler`, que usa `SchedulingProblem` + `solve_decomposed_scheduling_problem()` sobre Marvin y McLaughlin con configuración explícita de columnas/recursos, y versiona la salida consolidada en `datasets/benchmarks/outputs/multi-mine-scheduling-report.json`. | Multi-mine scheduling benchmark | MR-167, MR-175, MR-177 | El benchmark corre sobre más de una mina/instancia con la misma API del core y reporta diferencias por dataset sin ramas especiales por nombre de mina. |
| 182 | MR-182 | `[x]` | P1 | Docs | Trazabilidad bibliográfica en algoritmos | Formalizar que cada implementación basada en papers, preprints arXiv, tutoriales o bibliografía técnica de práctica actual deje DOI/URL y cita corta en comentarios o docstrings del módulo/función correspondiente, además de referencias visibles en docs y benchmarks cuando aplique. Ya quedó documentado en `AGENTS.md`, reforzado en `docs/backlog.md` y aplicado en módulos nuevos como `scheduling_problem.rs` y `small_scheduling.rs`. | Algorithm reference policy | MR-001 | Los nuevos módulos algorítmicos del repo preservan referencias técnicas visibles en código y documentación, facilitando auditoría, mantenimiento y revisión crítica de la madurez de cada fuente. |
| 183 | MR-183 | `[x]` | P1 | Benchmarks | Taxonomía exacta de instancias MineLib comparables | Separar explícitamente en manifests, harnesses y reportes qué instancia se está ejecutando (`marvin`, `mclaughlin-limit`, `mclaughlin-full`, u otra) para no comparar contra papers que reportan otra variante. La literatura suele reportar `McLaughlin Limit` (~112k bloques, 15 periodos), mientras que el repo hoy corre `mclaughlin-full` (~2.14M bloques) para smoke pesado. Ya existe diferenciación explícita en `datasets/benchmarks/*/manifest.json`, un manifiesto reservado para `datasets/benchmarks/mclaughlin-limit/manifest.json`, y el reporte `datasets/benchmarks/outputs/multi-mine-scheduling-report.json` expone `instance_id`, `instance_variant` y `literature_reference_instance`. | Benchmark target matrix | MR-181 | Los reportes distinguen sin ambigüedad la instancia ejecutada, los tamaños/periodos coinciden con la fuente y no se mezclan resultados del benchmark local con tablas de papers sobre otra variante. |
| 184 | MR-184 | `[x]` | P1 | Benchmarks | Inputs de scheduling alineados a CPIT/PCPSP | Reemplazar el uso de `upit.sol` como proxy de bloques seleccionados cuando se compare contra CPIT/PCPSP, especialmente en McLaughlin. La entrada al scheduler debe venir de una familia de shells/pushbacks o de un conjunto de bloques consistente con la misma economía/capacidades del problema de scheduling, siguiendo [R23], [R29] y el `gap problem` de [R31]. El harness `multi_mine_scheduler` ya usa `cpit-solution` como `selected_block_source` para Marvin y McLaughlin Full, y versiona esa trazabilidad directamente en `datasets/benchmarks/outputs/multi-mine-scheduling-report.json`. | Scheduling-aligned benchmark inputs | MR-174, MR-181, MR-183 | El benchmark ya no alimenta PCPSP desde un shell incompatible; cada comparación declara exactamente de dónde viene la selección de bloques y por qué es comparable con la referencia. |
| 185 | MR-185 | `[ ]` | P1 | Planning | Pushbacks data-driven desde shells anidados | Pasar de bandas sintéticas de bancos a pushbacks derivados de shells anidados por revenue factor o parametrización equivalente. La ruta debe seguir la práctica descrita en [R20], [R29], [R31] y [R37], de modo que el número de pushbacks no sea una constante hardcodeada sino una consecuencia del yacimiento y la familia de shells. Ya existe una base sparse-safe en el core (`generate_nested_shells_from_weight_map(...)` y `generate_nested_shells_from_weight_scenarios(...)`), un helper benchmark-side para MineLib/Marvin sin asumir índices densos y ahora también una selección explícita de ruta preferida para scheduling MineLib: `multi_mine_scheduler` ya promueve en Marvin una familia `nested-shell × bench` revenue/cost-aware con acceso `strict sequential`, dejando `reference-period × bench` solo como fallback/reporting donde la ruta de shells todavía no está habilitada. El aprendizaje reciente ya no es solo “sí aparecen múltiples shells”, sino también **cuántos** convienen bajo la política actual: en Marvin, un sweep estricto por factor-count mostró que 7 revenue factors (6 shells / 73 fases) mejoran el candidato hasta `664,161,466.99`, muy por encima del setup de 5 factors (`563,922,451.84`) y también por encima de 9 (`566,738,916.45`). Eso confirma que la calibración de la familia de shells ya es una palanca real de desempeño, no solo un detalle de presentación. | Nested-shell pushback generator | MR-157, MR-184 | El SDK puede generar una familia de shells/pushbacks reproducible, con metadata del factor económico usado y sin depender de `phase_bench_span = 4` ni cadenas secuenciales artificiales. |
| 186 | MR-186 | `[ ]` | P1 | Planning | Unidades `bench × pushback` o mining cuts | Evolucionar el adapter actual basado en `reference-period × bench` hacia una agregación paper-like derivada desde geometría real: pares `(bench, pushback)` o mining cuts/aggregation units guiadas por shells, geología y precedencias auditables. Esto debe poblar `SchedulingProblem` con una estructura más fiel que la proxy actual basada en membresías CPIT staged, siguiendo [R33], [R36] y [R37]. Ya existe una ruta sparse-safe para derivar `bench × pushback` desde shells anidados (`derive_phase_design_from_nested_shells_from_map(...)`) y ahora el core ya respeta reglas cross-shell alineadas por bench más `min_bench_lag`; además, el benchmark Marvin ya dejó atrás el split **uniforme** de chunks y ahora preserva bloques/sumas reales por chunk usando cuantiles de tonelaje, y `multi_mine_scheduler` ya consume una ruta preferida explícita donde Marvin promueve `nested-shell × bench` como candidato principal y deja `reference-period × bench` como baseline/fallback reportado en vez de hardcodearlo como único adapter. Aun así, veintinueve atajos fáciles ya quedaron descartados o acotados: (1) sobre-segmentar fases por periodos LP con la familia actual de cuts solo llega a `617,541,049.66` aun usando el mejor ancho probado, (2) reemplazar esos cuts por una variante LP-sorted + tonnage-balanced quantiles tampoco ayuda (`605,287,253.62`), (3) abrir por completo el acceso cross-shell bench-aligned en Marvin, (4) introducir lags bench-aligned simples `1/2` sobre esa misma política abierta, (5) esperar que el ajuste fino de la proxy LP-cut por sí solo cierre el gap, (6) partir la fase por componentes geométricas planas sin una ley de acceso más rica —aunque esa variante sube a `661,967,591.88`, todavía no supera el shell×bench base—, (7) **localizar** los predecesores de esas componentes por solape/vecindad en planta, que baja levemente a `661,655,409.66`, (8) **subdividir cada componente en stripes fijas sobre el eje dominante**, que cae mucho más a `634,746,714.87`, (9) **partir la fase completa en front bands direccionales** sobre el eje dominante, que recupera algo frente a las stripes rígidas pero sigue lejos del mejor geométrico (`645,831,287.60`), (10) **localizar** los predecesores de esos front bands globales por solape/vecindad, que no mueve el resultado en absoluto (`645,831,287.60`), (11) **split selectivo de componentes grandes** dentro de fases ya fragmentadas, que sí recupera bastante valor (`658,651,263.66`) pero todavía queda por debajo de la baseline geométrica simple por componentes, (12) el **sweep de umbrales** sobre ese split adaptativo mostró que `35%` empeora (`658,031,543.83`), `50/65/80%` empatan en `658,651,263.66`, y `95%` simplemente colapsa al baseline geométrico por componentes (`661,967,591.88`) porque casi no activa el split, (13) un **sweep refinado de gates por forma/elongación** (`aspect ratio` en `{1.25,1.5,1.75,2.0,2.5,3.0}` y `dominant-span` en `{1,2,3,4}`) mostró una meseta de mejor valor (`662,040,687.48`) en varias combinaciones, y permitió promover una regla más estricta (`aspect ratio >= 2.0`, `dominant-span >= 2`) que conserva ese valor con menos fases (`1174` vs `1196`), (14) un **sweep de cap de fronts** bajo esa gate promotora confirmó `max_front_count = 3` como mejor ajuste local (`662,040,687.48`), por encima de `2` (`662,027,181.12`) y `4` (`662,021,878.01`), (15) una variante **shape-gated con precedencias localizadas por solape/vecindad** (`shape-gated-local-front-target-seeded`) elevó el candidato a `662,115,935.74` manteniendo `1174` fases y `10` periodos usados, (16) un **sweep de filtro local** confirmó que `overlap-plus-adjacency` mantiene ese mejor valor (`662,115,935.74`) mientras `overlap-only` cae a `662,059,771.89`, (17) un **sweep de progresión de frentes** mostró que los perfiles front-loaded (`45-80-100`, `55-85-100`) recortan fases (`1021/1014`) pero degradan el objetivo (`661,834,930.10` y `661,680,969.80`) frente al perfil uniforme (`662,115,935.74`), (18) un **sweep condicional por forma** (activar `45-80-100` solo desde `aspect_ratio >= 2.5/3.0/3.5`) tampoco mejoró: el mejor punto quedó en `661,750,864.14`, ~`365k` por debajo del shape-gated-local uniforme, (19) un **sweep de ventana local de predecesores** (closest-N) solo mejoró marginalmente: `N=1` cae a `657,027,789.69`, `N=2` sube a `661,516,032.35`, `N=3` llega a `662,133,368.49`, `N=4` marca el mejor punto nuevo en `662,139,586.73`, y `N=5/6` vuelven al baseline local (`662,115,935.74`), (20) un **sweep de front-count localizado** con ventana fija `N=4` (`shape_gated_local_front_count_sweep`) confirmó que `max_front_count = 3` sigue siendo el mejor cap (`662,139,586.73`), mientras `2/4/5` quedan por debajo (`661,976,808.44`, `662,087,463.40`, `662,033,999.99`), (21) un **sweep de filtro local con ventana fija** (`shape_gated_local_access_window_sweep`, `N=4`) confirmó que `overlap-plus-adjacency` sigue siendo mejor (`662,139,586.73`) que `overlap-only` (`662,059,771.89`) sin mover el techo, (22) un **sweep de progresión condicional con ventana fija** (`shape_gated_conditional_window_progression_sweep`, `N=4`) tampoco ayudó: el mejor punto quedó en `661,750,864.14` (`aspect_ratio >= 2.5`), por debajo del mejor local-window actual, (23) un **sweep de progresión de frentes con ventana fija** (`shape_gated_front_progression_window_sweep`, `N=4`) confirmó de nuevo el perfil uniforme `33-67-100` como mejor (`662,139,586.73`), mientras `45-80-100`/`55-85-100` siguen degradando (`661,775,296.31` y `661,680,969.80`) aunque recorten fases, (24) un **sweep de front-count localizado en modo overlap-only** (`shape_gated_local_overlap_front_count_sweep`, `N=4`) confirmó que incluso su mejor punto (`max_front_count=3`, `662,059,771.89`) queda por debajo del mejor modo con adjacency (`662,139,586.73`), (25) un **sweep conjunto de gates geométricos + ventana local fija** (`shape_gated_local_rule_window_sweep`, `N=4`) sí movió el techo local: `aspect_ratio >= 3.0` y `dominant-span >= 4` elevan el candidato a `662,950,385.30` con `798` fases y `10` periodos usados, +`810,798.57` frente al mejor local-window previo, (26) un **sweep de front-count sobre esa regla local promovida** (`shape_gated_local_rule_front_count_sweep`) confirmó que `max_front_count = 3` ya era el mejor cap bajo (`aspect_ratio >= 3.0`, `dominant-span >= 4`, `N=4`) sin mejora adicional (`662,950,385.30`; `2/4/5` quedan en `661,729,972.50`, `662,070,053.09`, `662,162,252.72`), (27) un **sweep dinámico de ventana local por aspecto** (`shape_gated_dynamic_local_window_sweep`) tampoco rompió el techo: promover `closest-N` desde base `N=4` hacia `N=5/6/8` para componentes elongados (`aspect_ratio >= 2.5/3.0`) empata exactamente en `662,950,385.30` con `798` fases, (28) el builder benchmark-side de **pushback bench-localized mining cuts** sobre la misma base shell×bench + acceso local sí logró refinar al menos una fase single-component y exponer una alternativa más paper-like (`1176` fases / `1180` unidades), pero el primer punto quedó en `662,139,586.73`, ~`810,798.57` por debajo del mejor v8 y con gap bound→candidate de `60.57%`, y (29) un **sweep focalizado de calibration knobs del mismo builder** confirmó que ese primer punto ya era el mejor ajuste local: `closest-N=3` cae a `662,133,368.49` (−`6,218.25`), `N=5/6` empatan en `662,115,935.74` (−`23,650.99`), `max_front_count=4` baja a `662,087,463.40` (−`52,123.33`) y `max_front_count=2` cae a `661,976,808.44` (−`162,778.29`). Es decir, la familia ya tiene una calibración benchmark-side creíble, pero su óptimo local sigue siendo exactamente el primer punto y todavía no justifica promover esa ruta como nueva baseline frente a v8. Con la familia de 7 factors, el sweep de acceso mostró que `strict-shell-sequential` sigue claramente mejor (`664,161,466.99`) que `open-bench-lag-0/1/2` (`531.27M / 452.35M / 367.68M`), y el sweep de anchos LP-cut dejó a `period_band_width = 3` como el menos malo de `1/2/3/4`, pero todavía por debajo del shell×bench base. Eso deja más claro que la agregación todavía necesita una geometría/pushback design más fiel a papers y una política de acceso calibrada, pero también que la dirección más prometedora ya no es el split global rígido, sino cerrar una ley de acceso/progresión geométrica que capture el remanente (~`1.21M`) frente al baseline estricto sin romper factibilidad. | Literature-grounded scheduling aggregation | MR-175, MR-185 | El benchmark alimenta el scheduler con unidades derivadas de shells/pushbacks o cortes auditables; ya no depende de una proxy `reference-period × bench` cuando la bibliografía exige una descomposición geométrica más fuerte. |
| 187 | MR-187 | `[ ]` | P1 | Planning | Baseline LP/BZ + rounding para MineLib scheduling | Incorporar una baseline fuerte para benchmarking basada en relajación LP/BZ o equivalente y rounding factible tipo TopoSort/sliding-window, de forma que `ready frontier` deje de ser la única referencia sobre instancias medianas/grandes. Hoy el reporte multi-mine ya expone una baseline intermedia `cpit-period-routed` para aislar el gap de ruteo/destino usando los periodos staged de CPIT, y además versiona las relajaciones abiertas `LPcpit`/`LPpcpsp` cuando el dataset las trae; aun así falta el salto bibliográfico principal: un bound LP/BZ propio y un candidato entero derivado de esa relajación dentro del workflow del repo. Ya se probaron y descartaron varios atajos benchmark-side: (1) rounding bloque-a-bloque guiado por LP + `*.prec`, (2) guidance por fases agregadas sobre la proxy `reference-period × bench`, (3) una baseline `lp-shell-seeded` sobre las fases shell-driven de Marvin que agrega los periodos LPpcpsp por fase y repara precedencias, (4) una baseline `lp-target-period-seeded` que usa esos mismos targets LP para guiar directamente un ready-frontier sobre el `SchedulingProblem` chunked, (5) una variante `lp-staggered-target-seeded` que escalona esos targets por chunk, (6) una baseline `lp-windowed-exact` que resuelve un packing exacto local sobre una ventana LP-guided de hasta 18 unidades ready por iteración, y ahora (7) una baseline `lp-cut-target-seeded` que primero parte las fases `shell × bench` en cuts guiados por bandas de periodo LP. Además, el benchmark ya corrigió otro sesgo importante: los chunks dejaron de repartir objetivo/recursos por promedio y ahora preservan bloques reales y sumas reales por cuantiles de tonelaje, el core ya soporta acceso cross-shell alineado por bench con `min_bench_lag`, y el reporte ahora versiona sweeps de factor-count, acceso y ancho LP-cut, además de variantes LP-cut tonnage-balanced y geométricas. El primer sweep sí movió materialmente el techo de Marvin: con 7 revenue factors y acceso shell-a-shell explícito, el candidato sube a `664,161,466.99` (vs `563,922,451.84` con 5 factors). Pero la conclusión de MR-187 no cambió: las variantes `lp-target-period-seeded` y `lp-windowed-exact` solo empatan ese techo nuevo, la familia `lp-cut-target-seeded` mejora apenas al calibrar el ancho y queda en `617,541,049.66` con `period_band_width = 3`, la variante `lp-quantile-cut-target-seeded` es aún peor (`605,287,253.62`), la familia geométrica por componentes queda apenas debajo del baseline en `661,967,591.88`, la refinación con predecesores localizados por solape/vecindad tampoco rompe el techo (`661,655,409.66`), la variante con stripes fijas por eje dominante se aleja todavía más (`634,746,714.87`), las front bands direccionales globales mejoran algo frente a esas stripes pero tampoco destraban el techo (`645,831,287.60`), localizarlas por solape/vecindad no cambia nada (`645,831,287.60`), y el split adaptativo/selectivo por tonelaje tampoco rompe el techo. El hallazgo nuevo relevante es que la familia shape-gated sigue mejorando cuando se localiza el acceso: `shape-gated-local-front-target-seeded` llega a `662,115,935.74`, el sweep closest-`N` mejora levemente hasta `N=4` (`662,139,586.73`), el sweep de front-count localizado con ese `N=4` confirma que `max_front_count=3` sigue siendo el mejor cap sin mejora adicional, el sweep de filtro local con ventana fija (`N=4`) también confirma `overlap-plus-adjacency` como mejor modo sin mover el techo, los sweeps de progresión condicionada/ventaneada tampoco mejoran (mejor punto `661,750,864.14`), el sweep de progresión frontal con ventana fija revalida `uniform-33-67-100` como mejor perfil pero al mismo techo actual, el sweep de front-count localizado en modo overlap-only queda por debajo de ese techo (`662,059,771.89`), el nuevo sweep conjunto de gates geométricos + ventana local fija (`shape_gated_local_rule_window_sweep`, `N=4`) eleva ese techo local a `662,950,385.30` con `aspect_ratio >= 3.0` y `dominant-span >= 4`, el sweep de front-count sobre esa regla local (`shape_gated_local_rule_front_count_sweep`) revalida `max_front_count=3` sin mejora adicional (`662,950,385.30`), y el nuevo sweep dinámico por aspecto (`shape_gated_dynamic_local_window_sweep`) también empata en `662,950,385.30`. En resumen, los perfiles front-loaded y la supresión de adjacency siguen degradando el objetivo, y la promoción dinámica simple de closest-N tampoco rompe el techo local. Eso todavía deja un gap de ~`1.21M` frente a `strict-shell-sequential` (`664,161,466.99`) y refuerza que, con la agregación/unidades actuales, el cuello de botella ya no está en otro tweak local del guidance LP sino en cerrar una ley de acceso/progresión geométrica más rica que capture ese remanente sin perder factibilidad. El refresh focalizado más reciente ya versiona además un `calibration_sweep` específico para `pushback-bench-localized-cut-phase`, y ese sweep confirma que el primer builder point (`front3-ar2.0-span2-n4`, `662,139,586.73`) sigue siendo el mejor de los knobs plausibles benchmark-side probados: `N=3` queda `6,218.25` abajo, `N=5/6` pierden `23,650.99`, `max_front_count=4` pierde `52,123.33` y `max_front_count=2` pierde `162,778.29`. El mismo refresh focalizado ya prueba además una ola v9 `shape-gated-local-front-period-band-phase` sobre los localized fronts v8, y también falla en promover una nueva baseline: `period_band_width = 3` es el menos malo (`618,109,648.19`), mientras `4/1/2` quedan en `616,905,733.77` / `614,903,239.76` / `614,832,519.92`; al comparar el cableado de precedencia, `predecessor-last-cut` y `all-predecessor-cuts` empatan exactamente en ese mismo techo, mientras `predecessor-first-cut` cae a `607,477,911.35`. Eso entrega evidencia más creíble sobre la familia paper-like, pero no cambia el diagnóstico: el óptimo localized-cut sigue ~`810,798.57` por debajo de v8 (`662,950,385.30`), la ruta v9 refinada por bandas de periodo LP queda muchísimo más abajo, y el bloqueo principal sigue siendo de comparabilidad/modelado bibliográfico, no de refresh. Por eso MR-187 sigue bloqueado hasta consolidar una base de unidades y accesos más fiel a la literatura; recién ahí valdrá la pena volver a intentar un rounder/repair más fuerte sobre unidades comparables con la bibliografía. La etapa debe apoyarse en [R30], [R34], [R35], [R36] y [R37], y exponer bound + candidato entero comparable. | LP-guided scheduling baseline | MR-176, MR-180, MR-186 | Run 2026-05-28 (focused MR-187 refresh v8 + localized-cut/v9 sweeps): `cargo run -p marvin-benchmark --bin marvin-benchmark -- --mode focused-mr187` completado y `datasets\benchmarks\marvin\outputs\mr187-focused-refresh-report.json` regenerado. El refresh focalizado confirma `report_mode=focused-mr187`, `lp_bz_inputs` con 20 periodos y 802 unidades `shape-gated-local-front-phase`, `lp_bz_bound_artifact` (`bound_label=lp-bz-native-resource-envelope`, `discounted_objective_bound=1,679,430,935.69`), `lp_bz_lp_kernel_artifact` (`kernel_label=lp-bz-lp-kernel-v8-local-front-access-progression-scaffold`) y `lp_bz_integer_candidate_artifact` (`baseline_name=lp-bz-round-repair-local-front-seeded`, `discounted_objective=662,950,385.30`, `phase_count=798`, `fractional_assignment_count=0`). `lp_bz_gap_metrics` deja un gap bound→candidate de `1,016,480,550.39` (`60.53%`) y un gap contra la referencia PCPSP de `223,017,676.19`; además deja explícito que este refresh es focalizado, no el benchmark exhaustivo: el solve LP nativo queda `skipped-focused-refresh`, `effective_bound_source=native-resource-envelope` y `candidate_vs_ready_frontier_objective_gap=0.0` porque `ready frontier` no se reejecuta. La evidencia nueva del mismo run también deja trazado el sweep `lp_bz_pushback_bench_localized_cut_experiment.calibration_sweep`, donde `front3-ar2.0-span2-n4` permanece como mejor punto local (`662,139,586.73`, `1176` fases / `1180` unidades, `repaired_phase_target_count=625`) y ninguno de los ajustes benchmark-side de cut-count/closest-N logra acercarlo a v8. El mismo artefacto reporta además `lp_bz_v9_local_front_band_width_sweep`, donde `period_band_width = 3` es el mejor ancho focalizado pero solo llega a `618,109,648.19`, y `lp_bz_v9_local_front_band_link_policy_sweep`, donde `predecessor-last-cut` y `all-predecessor-cuts` empatan mientras `predecessor-first-cut` degrada a `607,477,911.35`. Evidencia v8 refrescada, familia localized-cut calibrada y v9 period-band descartado como promoción inmediata; el bloqueo pendiente sigue siendo de comparabilidad/modelado bibliográfico, no de refresh. |
| 188 | MR-188 | `[x]` | P1 | Benchmarks | Matriz de comparabilidad contra papers | Versionar para cada benchmark el workflow exacto usado (instancia, horizonte, discount rate, capacidades, shell source, tipo de agregación y solver/heurística) y marcar cuándo una corrida es solo exploratoria versus comparable con la literatura. Debe dejar explícito que “seguir el paper” implica también copiar el pipeline de parametrización y no solo el nombre del solver. El reporte `datasets/benchmarks/outputs/multi-mine-scheduling-report.json` ya versiona `instance_id`, `instance_variant`, `literature_reference_instance`, `selected_block_source`, `comparison_classification` y `comparability_gaps` por dataset. | Literature comparability report | MR-183, MR-184, MR-187 | Los reportes separan corridas paper-comparable de corridas exploratorias locales, documentan el método de referencia esperado y evitan interpretar como “gap al óptimo” un experimento que todavía no reproduce el pipeline de la bibliografía. |
| 189 | MR-189 | `[ ]` | P1 | Benchmarks | Pipeline bibliográfico de Marvin | Construir un pipeline de reproducción específico para `marvin` que siga la secuencia usada por MineLib y los trabajos de LP/PCPSP sobre esta instancia: problema PCPSP con su horizonte/capacidades/destinos reales, shell/pushbacks consistentes con scheduling y baseline LP/BZ + rounding o equivalente. Debe apoyarse en [R29], [R35] y [R37], además de la especificación pública MineLib enlazada por `marving-info.txt` (`https://mansci-web.uai.cl/minelib/minelib_format.pdf`). El benchmark específico de Marvin ya sustituyó las bandas sintéticas de 4 benches por una ruta reproducible `field_4/field_5/field_6 -> revenue/cost-aware factor scenarios -> nested-shell × bench -> ready-frontier`, esa corrección ya produce múltiples shells, el chunking interno ya preserva bloques y sumas reales por cuantiles de tonelaje, y el reporte ahora versiona cinco baselines LP-guided explícitas (`lp-shell-seeded`, `lp-target-period-seeded`, `lp-staggered-target-seeded`, `lp-windowed-exact`, `lp-cut-target-seeded`) más tres probes de calibración (`strict_shell_factor_sweep`, `shell_access_sweep`, `lp_cut_band_width_sweep`) y dos baselines exploratorias adicionales (`lp-quantile-cut-target-seeded`, `geometric-component-target-seeded`). Esos artefactos ya dejaron seis criterios prácticos: (1) con acceso shell-a-shell estricto, 7 revenue factors (6 shells / 73 fases) superan claramente al setup previo de 5 y al de 9; (2) con esa misma familia, los accesos abiertos bench-aligned con lags simples 0/1/2 quedan muy por debajo del acceso estricto; (3) dentro de la familia actual de LP-cuts, `period_band_width = 3` es el mejor ancho probado, pero igual queda bastante por debajo del shell×bench base; (4) la alternativa LP-sorted + tonnage-balanced quantiles también queda por debajo de la mejor variante banded; (5) una descomposición geométrica básica por componentes conectadas sí es la alternativa más prometedora hasta ahora, pero todavía no supera el baseline shell×bench; (6) incluso la nueva ola focalizada v9 `shape-gated-local-front-period-band-phase` sobre los localized fronts v8 sigue siendo exploratoria y claramente insuficiente: su mejor ancho (`period_band_width = 3`) solo llega a `618,109,648.19`, y cambiar la política de enlace entre cuts predecesores no mejora ese techo. Sigue pendiente calibrar mejor los pushbacks y cuts más allá del simple factor-count/band-width/componente plana, la política de acceso entre shells y completar la comparación contra la bibliografía LP/BZ/TopoSort. | Marvin paper-reproduction pipeline | MR-184, MR-185, MR-186, MR-187, MR-188 | Existe un comando/reporte específico para Marvin que reproduce un workflow comparable con la bibliografía, declara cada etapa usada y deja explícita la diferencia entre bound, solución factible y referencia publicada. |
| 190 | MR-190 | `[x]` | P1 | Benchmarks | Pipeline bibliográfico de McLaughlin Limit | Incorporar el pipeline específico para la variante `mclaughlin-limit` que aparece en la literatura MineLib (≈112,687 bloques, 15 periodos), separándola de `mclaughlin-full`. Ya existe integración explícita en `datasets/benchmarks/mclaughlin-limit/manifest.json`, un test dedicado de carga en `examples/marvin-benchmark/tests/benchmark_blocks.rs` y el harness `cargo run -p marvin-benchmark --bin multi_mine_scheduler --quiet` reporta `mclaughlin-limit-local` con 15 periodos, `selected_block_source = "cpit-solution"` y referencia PCPSP separada de la variante full. | McLaughlin Limit paper-reproduction pipeline | MR-183, MR-184, MR-185, MR-186, MR-187, MR-188 | El repo puede correr un pipeline dedicado sobre `mclaughlin-limit`, los tamaños/horizonte coinciden con la literatura y el reporte deja de mezclar la variante full local con los resultados publicados. |
| 191 | MR-191 | `[x]` | P2 | Benchmarks | Pipeline exploratorio de McLaughlin Full | Mantener `mclaughlin-full` como pipeline pesado de stress/scalability sobre el mismo core, pero separado explícitamente de la reproducción bibliográfica. Ya existe en `multi_mine_scheduler` como `mclaughlin-full-local`, reutilizando `SchedulingProblem` + `solve_decomposed_scheduling_problem()` y quedando etiquetado en `datasets/benchmarks/outputs/multi-mine-scheduling-report.json` como `comparison_classification = "exploratory-local"` con gaps explícitos frente a la literatura `mclaughlin-limit`. | McLaughlin Full stress pipeline | MR-183, MR-187, MR-188 | Existe un pipeline reproducible para la instancia full que conserva el valor ingenieril del benchmark pesado, pero sus reportes/README lo etiquetan como exploratorio y no como reproducción directa de la bibliografía. |

### MR-187 — contrato LP/BZ + rounding (benchmark-side)

Esta especificación fija el contrato de MR-187 sobre la arquitectura actual de `examples/marvin-benchmark/src/main.rs` y el reporte `datasets/benchmarks/marvin/outputs/comparison-report.json`. Es una definición de planning/benchmark; no cambia lógica de `mine-sdk` en esta etapa.

Para refreshes operativos del backlog sin recorrer todo el benchmark exhaustivo, el harness ahora expone además `cargo run -p marvin-benchmark --bin marvin-benchmark -- --mode focused-mr187` (o `MARVIN_BENCHMARK_MODE=focused-mr187`), que por defecto escribe `datasets/benchmarks/marvin/outputs/mr187-focused-refresh-report.json`. Ese modo mantiene el modo `full` intacto y conserva los artefactos LP/BZ relevantes para MR-187, pero deja explícito cuando neutraliza comparaciones no recalculadas (`ready frontier`) o marca el solve LP nativo como `skipped` para mantener la corrida acotada en este entorno.

La misma corrida focalizada ahora deja un sweep experimental side-by-side `shape-gated-local-front-period-band-phase` para `period_band_width ∈ {1,2,3,4}` y evaluar si conviene componer la normalización v8 con bandas de periodo LP. La evidencia nueva confirma que la composición **sí** refina la granularidad, pero por ahora **no** conviene promoverla en ninguno de esos anchos: `width=1` llega a `614,903,239.76` (+358 fases, `641` reparaciones), `width=2` a `614,832,519.92` (+207 fases, `526` reparaciones), `width=3` es el menos malo con `618,109,648.19` (+162 fases, `499` reparaciones) y `width=4` queda en `616,905,733.77` (+128 fases, `464` reparaciones), todos muy por debajo del v8 focalizado base (`662,950,385.30`). El refresh v10 agregó además un sweep focalizado de políticas de enlace entre cuts predecesores para ese `width=3`: `predecessor-first-cut` cae todavía más a `607,477,911.35` (`465` reparaciones; gap `63.83%`), mientras que `all-predecessor-cuts` empata exactamente al baseline actual `predecessor-last-cut` (`618,109,648.19`, gap `63.20%`) aun subiendo la densidad de precedencias directas de `115,502` a `152,000`. La lectura práctica es que, dentro de esta composición benchmark-side, la degradación ya no parece explicarse por “anclar al último cut” sino por un problema más profundo en la geometría/unidad refinada y/o en la ley de acceso/progresión que gobierna esas bandas sobre localized fronts.

Implementación inicial ya integrada (benchmark-side, primera pasada):

- `lp_bz_bound_artifact`: bound proxy basado en `marvin.LPpcpsp` (objetivo descontado leído del artefacto LP de referencia).
- `lp_bz_lp_kernel_artifact`: kernel LP auditable versionado en formato compacto para el reporte (conteos, labels, muestras representativas y diagnósticos de acceso) en vez de serializar todas las variables/filas/términos de instancias grandes.
- `lp_bz_lp_solve_artifact`: solve LP nativo in-harness (`minilp`) sobre kernel relajado (capacidad+activación, precedencia removida).
- `lp_bz_integer_candidate_artifact`: candidato entero inicial usando la ruta `lp-windowed-exact` (target-period LP-guided + packing exacto local/repair sobre ready frontier).
- `lp_bz_gap_metrics`: cálculo explícito de gap absoluto/relativo bound↔candidato y gaps del candidato frente a `pcpsp_reference` y `ready frontier`.

#### 1) Inputs requeridos del workflow (`lp_bz_inputs`)

- `problem_normalization` (obligatorio): normalización explícita del problema de scheduling sobre el que se calcula el bound y se redondea.
  - Campos mínimos: `period_count`, `resource_constraint_count`, `destination_count`, `discount_rate`.
  - Debe reutilizar la semántica de `ScheduleReferenceArtifactSummary` ya usada en `cpit_reference`, `pcpsp_reference`, `lp_cpit_reference` y `lp_pcpsp_reference`.
- `precedence_units` (obligatorio): unidades agregadas para LP/BZ + rounding.
  - Campos mínimos: `unit_count`, `edge_count`, `unit_granularity_label`.
  - `unit_count` debe ser consistente con `phase_count` de la baseline usada para redondeo.
- `lp_relaxation_source` (obligatorio): artefacto de relajación base para el bound.
  - Campos mínimos: `source_label`, `reference_artifact_path`, `objective_kind`.
  - `source_label` debe distinguir al menos `lp_bz` vs otras relajaciones (`LPcpit`, `LPpcpsp`) para evitar mezclar gaps heterogéneos.

#### 2) Outputs requeridos del workflow

- `lp_bz_bound_artifact` (obligatorio): bound LP/BZ auditable.
  - Campos mínimos: `bound_label`, `discounted_objective_bound`, `period_count`, `resource_constraint_count`, `destination_count`, `unit_count`.
- `lp_bz_lp_kernel_artifact` (obligatorio): kernel LP/BZ serializable.
  - Campos mínimos: `kernel_label`, `period_count`, `unit_count`, `destination_count`, `variable_index`, `objective`, `constraints`.
  - En `comparison-report.json` puede usarse una representación compacta siempre que preserve conteos, labels, ejemplos deterministas y diagnósticos suficientes para auditar comparabilidad/gaps sin serializar todas las filas o términos.
- `lp_bz_lp_solve_artifact` (obligatorio): resultado del solve LP nativo sobre el kernel in-harness.
  - Campos mínimos: `solver_label`, `solve_status`, `discounted_objective_bound`, `variable_count`, `active_variable_count`.
- `lp_bz_integer_candidate_artifact` (obligatorio): candidato entero obtenido por rounding/repair desde la relajación.
  - Campos mínimos: `baseline_name`, `phase_count`, `candidate_pcpsp_summary`, `candidate_vs_reference_metrics`, `candidate_vs_reference_membership_comparison`.
  - `candidate_pcpsp_summary.fractional_assignment_count` debe ser `0`.
- `lp_bz_rounder_v6_local_optimizer_diagnostics` (obligatorio): diagnósticos serializables del rounder/optimizer local benchmark-side.
  - Campos mínimos: `rounder_strategy_label`, `local_optimizer_strategy_label`, `local_optimizer_max_iteration_count`, `local_optimizer_executed_iteration_count`, `local_optimizer_improving_move_count`, `local_optimizer_termination_reason`, `repaired_phase_target_count`, `repaired_unit_target_count`, `horizon_clamp_count`, `phase_target_count`, `unit_target_count`.
- `lp_bz_gap_metrics` (obligatorio): métricas de gap en el mismo reporte.
  - Campos mínimos:
    - `bound_to_candidate_absolute_gap`
    - `bound_to_candidate_relative_gap`
    - `effective_discounted_objective_bound`
    - `effective_bound_source`
    - `native_lp_kernel_discounted_objective_bound`
    - `candidate_vs_pcpsp_reference_objective_gap`
    - `candidate_vs_ready_frontier_objective_gap`

#### 3) Requisitos de comparabilidad

- Etiquetado obligatorio con `comparison_classification`:
  - `paper-comparable`: configuración alineada al pipeline bibliográfico declarado.
  - `exploratory-local`: experimento útil de ingeniería pero no comparable 1:1 con paper.
- `comparability_gaps` (lista) obligatorio:
  - Vacía solo cuando `comparison_classification = "paper-comparable"`.
  - Si hay cualquier diferencia de normalización/instancia/pipeline, debe listarse en texto explícito.
- Reusar naming de comparabilidad ya presente en benchmark multi-mine: `instance_variant`, `literature_reference_instance`, `selected_block_source`, `comparison_classification`, `comparability_gaps`.

#### 4) Criterios de aceptación MR-187 (medibles en reporte)

1. El reporte incluye `lp_bz_bound_artifact`, `lp_bz_lp_kernel_artifact`, `lp_bz_lp_solve_artifact`, `lp_bz_integer_candidate_artifact`, `lp_bz_rounder_v6_local_optimizer_diagnostics` y `lp_bz_gap_metrics`.
2. Se verifica en el JSON que:
   - `lp_bz_integer_candidate_artifact.candidate_pcpsp_summary.fractional_assignment_count == 0`.
   - `lp_bz_gap_metrics.bound_to_candidate_absolute_gap = lp_bz_gap_metrics.effective_discounted_objective_bound - lp_bz_integer_candidate_artifact.candidate_pcpsp_summary.discounted_objective`.
   - `lp_bz_gap_metrics.bound_to_candidate_relative_gap >= 0`.
3. El reporte incluye `comparison_classification` + `comparability_gaps`:
   - `paper-comparable` ⇒ `comparability_gaps = []`.
   - `exploratory-local` ⇒ `comparability_gaps` documenta por qué no hay comparabilidad bibliográfica.
4. El candidato LP/BZ puede compararse contra referencias existentes del mismo reporte mediante campos ya vigentes (`candidate_vs_reference_metrics`, `candidate_vs_reference_membership_comparison`), sin introducir nomenclatura inconsistente.

---

# EPIC — Incertidumbre y métodos avanzados

## Objetivo

Preparar la ruta P2/P3 para incertidumbre geológica y planeamiento robusto sin acoplar métodos estocásticos al core determinista inicial.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 168 | MR-168 | `[x]` | P2 | Estimation | Contratos para realizaciones condicionales | Diseñar artefactos y metadatos para manejar múltiples realizaciones geológicas sin romper los contratos base del SDK. | Conditional realization contracts | MR-145 | Las realizaciones pueden almacenarse y evaluarse de forma consistente, con lineage explícito y sin mezclar determinismo con sampling implícito. |
| 169 | MR-169 | `[x]` | P3 | Estimation | Prototipos SGS y SIS | Implementar prototipos experimentales de sequential Gaussian simulation y sequential indicator simulation como capa avanzada sobre el engine de estimación. | SGS / SIS prototypes | MR-168, MR-142 | Existen prototipos reproducibles de SGS/SIS con seeds explícitos, `ConditionalRealizationSet` y validación mínima de estadísticos globales en tests del crate. |
| 170 | MR-170 | `[x]` | P2 | Economics | Métricas de riesgo y valuación robusta | Añadir métricas de riesgo tipo P10/P50/P90, downside, CVaR u otras sobre escenarios y realizaciones. | Risk-aware valuation | MR-166, MR-168 | Los reports económicos pueden resumir distribución y riesgo de valor sin esconder supuestos de sampling. |
| 171 | MR-171 | `[x]` | P3 | Planning | Pit y scheduling estocásticos experimentales | Explorar prototipos de pit final y scheduling bajo incertidumbre usando realizaciones y métricas de riesgo, sin prometer todavía un solver industrial completo. | Stochastic planning prototypes | MR-169, MR-170, MR-156, MR-163 | Existe el ejemplo `stochastic-planning` con un ensemble SGS sintético, comparación reproducible entre schedules candidatos, criterios explícitos de selección y limitaciones declaradas para decidir si vale la pena una ruta exacta/decomposed posterior. |
