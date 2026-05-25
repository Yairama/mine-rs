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
| 133 | MR-133 | `[x]` | P1 | Benchmarks | Comparador de resultados benchmark | Construir comparadores reproducibles entre outputs Marvin externos y outputs generados por `mine-rs` para conteos, membresía de bloques, aristas de precedencia, tonelaje/metal y métricas derivadas. Existe reporte versionado en `datasets/benchmarks/marvin/comparison-report.json` con `prec` edge_jaccard=1.0, economic_objective=1,415,655,436 y upit membership_jaccard=0.6458; quedan pendientes comparaciones equivalentes para `cpit`, `pcpsp` y relajaciones LP. | Benchmark comparator | MR-130, MR-132 | El comparador produce reportes serializables con coincidencias, diferencias y tolerancias explícitas por instancia. |
| 134 | MR-134 | `[x]` | P2 | Planning | Generación determinista de `prec` | Extender `mine-planning` para generar precedencias de bloque desde grilla, índices lineales y una plantilla explícita de talud/vecindad, reutilizando `PrecedenceGraph` como contrato interno. | Generador `prec` | MR-130, MR-093 | La generación produce un DAG válido, serializable y comparable contra referencias externas cuando existan. |
| 135 | MR-135 | `[x]` | P2 | IO | IO abierto para precedencias benchmark | Exponer lectura/escritura de precedencias benchmark desde/hacia un formato abierto y documentado para examples, tests y comparadores; si el `prec` externo no está completamente especificado, definir un formato normalizado propio además del importador. | IO de precedencias | MR-132, MR-134 | `mine-rs` puede exportar e importar precedencias benchmark de forma estable y los examples consumen el mismo contrato. |
| 136 | MR-136 | `[x]` | P2 | Planning | `upit` experimental abierto | Diseñar e implementar una primera ruta abierta para generar `upit` desde un modelo valuado y sus precedencias, dejando explícito si se trata de solver exacto o heurístico y cuáles son sus límites. | Prototipo `upit` | MR-130, MR-134 | El pipeline genera una shell/pit membership reproducible, documenta supuestos y puede compararse contra referencias Marvin cuando existan. |
| 137 | MR-137 | `[x]` | P2 | Benchmarks | Paridad Marvin por olas | Consolidar examples, comparadores y reportes para medir por instancia cuánto cubre `mine-rs` en cada ola: ingestión, outputs externos, `prec` propio y `upit` experimental. La primera matriz versionada ya vive en `datasets/benchmarks/marvin/parity-report.json` y deja explícitos los gaps que dependen de referencias externas reales. | Reporte de paridad Marvin | MR-131, MR-133, MR-135, MR-136 | Existe una matriz de cobertura por instancia/output que diferencia claramente lo ya reproducido, lo comparable y lo que sigue pendiente. |
| 167 | MR-167 | `[x]` | P1 | Benchmarks | Plantilla de talud Marvin 17-offset (45°/8-niveles) | Reverse-engineer y validar la plantilla exacta de precedencias Marvin (45°/8-niveles, 30×30×30m): 5 offsets en dk=1 (cruce cardinal), 4 en dk=3 (esquinas diagonales) y 8 en dk=5 (arco semicircular), totalizando 17 offsets. Aplicar en `marvin-benchmark` para que `edge_jaccard` alcance 1.0. | Plantilla 17-offset validada | MR-134 | `cargo run -p marvin-benchmark` produce `edge_jaccard_index: 1.0` contra `marvin.prec` con los 17 offsets verificados. |
| 168 | MR-168 | `[x]` | P1 | Benchmarks | Valor económico objetivo correcto en benchmark Marvin | Corregir el cálculo de `total_value` del benchmark de `sum(proc_profit)` a `sum(proc_profit × tonnage)`, y agregar `total_economic_objective = sum((max(proc_profit, 0) − mine_cost) × tonnage)` para comparar directamente con el objetivo oficial UPIT (1,415,655,436). | Métricas económicas correctas | MR-133 | El benchmark reporta `reference_total_economic_objective ≈ 1,415,655,436` y el candidato puede compararse en la misma escala. |
| 169 | MR-169 | `[x]` | P1 | IO | Normalizar archivo `marvin.upit` (valores objetivo por bloque) | Agregar parser para `marvin.upit` (formato: `block_id value_objective`) como complemento al `.sol` ya normalizado, exponiendo los valores económicos individuales por bloque para auditar el objetivo UPIT directo. | Parser `marvin.upit` | MR-132 | Existe `read_marvin_upit_block_values()` con tests, fixture de ronda y valores por bloque disponibles para comparar por membresía. |
| 170 | MR-170 | `[ ]` | P1 | IO | Normalizar CPIT y PCPSP Marvin | Implementar parsers para `marvin.cpit`, `marvin_cpit_gmunoz120723.sol`, `marvin.pcpsp`, `marvin_pcpsp_gmunoz120723.sol`, `marvin.LPcpit` y `marvin.LPpcpsp`, normalizando membresías y valores como contratos abiertos con tests reproducibles. | Parsers CPIT y PCPSP | MR-132 | Cada artefacto tiene parser documentado, test de fixture y representación interna que puede alimentar comparadores. |
| 171 | MR-171 | `[ ]` | P1 | Benchmarks | Comparar CPIT Marvin contra scheduling propio | Una vez exista el scheduler agregado (MR-163), conectar el comparador para evaluar la membresía y métricas de CPIT del scheduler mine-rs contra las referencias Marvin normalizadas. | Comparación CPIT vs scheduler | MR-163, MR-170 | El benchmark reporta jaccard de membresía y métricas de valor/tonelaje por periodo comparadas contra las referencias CPIT Marvin. |
| 172 | MR-172 | `[ ]` | P2 | Benchmarks | Comparar PCPSP Marvin contra scheduling con restricciones | Una vez exista scheduling con capacidades y pushbacks (MR-164), conectar el comparador para evaluar PCPSP mine-rs contra referencias Marvin. | Comparación PCPSP vs scheduler | MR-164, MR-170 | El benchmark reporta paridad de membresía y métricas de producción/periodo contra referencias PCPSP Marvin. |

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
| 147 | MR-147 | `[ ]` | P1 | Estimation | Validación del modelo estimado | Construir suite de cross-validation, swath plots, comparación composite-vs-block y reportes de calidad de estimación. | Estimation validation suite | MR-145, MR-146 | Existe un `ValidationReport` específico para estimación con métricas, plots/tablas serializables y ejemplos reproducibles. |
| 148 | MR-148 | `[ ]` | P2 | Estimation | Métricas explícitas de clasificación | Implementar un motor configurable de métricas para clasificación de recursos basado en sample spacing, informedness y continuidad, sin automatizar compliance. | Classification metrics engine | MR-147 | El SDK produce evidencia estructurada y audit trail para clasificación, diferenciando claramente métricas calculadas de decisiones profesionales externas. |

---

# EPIC — Economic block model y valorización multi-destino

## Objetivo

Pasar de un block model validado/estimado a un `EconomicBlockModel` reproducible que pueda alimentar pit final, pushbacks y scheduling, con fórmulas explícitas y destinos mineros auditables.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 149 | MR-149 | `[x]` | P1 | Economics | Contratos de supuestos por destino | Diseñar contratos explícitos para destinos como waste, mill, leach, stockpile y sell, incluyendo recoveries, payabilities, costos y capacidades. | Destination assumptions | MR-017, MR-147 | Los supuestos son serializables, tipados y reutilizables por valuación, scheduling y comparación de escenarios. |
| 150 | MR-150 | `[x]` | P1 | Economics | Fórmulas NSR y equivalent value | Implementar biblioteca explícita de fórmulas para NSR, equivalent value y cutoff-related metrics sin economía implícita. | NSR / EV formulas | MR-149 | El SDK calcula NSR/equivalent value con pruebas de fórmulas, unidades y sensitivities simples. |
| 151 | MR-151 | `[x]` | P1 | Economics | Valorización multi-destino por bloque | Extender la economía actual para calcular revenue, costo, margen y valor por bloque para múltiples destinos y reglas de selección explícitas. | Destination-aware block valuation | MR-149, MR-150 | Cada bloque puede evaluarse contra varios destinos, el artefacto es serializable y la selección de destino queda auditada. |
| 152 | MR-152 | `[ ]` | P2 | Economics | Primitives de stockpile y blending | Definir balances de stockpile, reclaim, degradación futura opcional y reportes mínimos de mezcla/destino sin entrar todavía en optimización completa. | Stockpile primitives | MR-151 | Existen contratos y cálculos base de balance por periodo/destino con errores explícitos y sin defaults silenciosos. |
| 153 | MR-153 | `[x]` | P1 | Economics | `EconomicBlockModel` integrado | Crear un artefacto estable que combine block model, supuestos, destinos y valores derivados como input estándar para pit y scheduling. | `EconomicBlockModel` | MR-151 | El artefacto preserva lineage, metadata, unidades y columnas económicas derivadas, y puede persistirse en formatos abiertos. |

---

# EPIC — Pit final exacto, shells anidados y métricas de pit

## Objetivo

Reemplazar el `upit` heurístico actual por una ruta exacta y benchmarkeable basada en max-closure / max-flow, con soporte para taludes variables y shells anidados.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 154 | MR-154 | `[ ]` | P1 | Planning | Plantillas de talud variables | Extender la generación de precedencias para soportar plantillas geotécnicas y slope templates más generales que offsets fijos mínimos. | Variable slope templates | MR-134, MR-153 | El SDK puede construir DAGs de precedencia desde reglas de talud explícitas y compararlas contra fixtures abiertos. |
| 155 | MR-155 | `[ ]` | P1 | Planning | Transformación max-closure del pit | Implementar la transformación desde `EconomicBlockModel` + `PrecedenceGraph` a un problema exacto de max-closure / max-flow serializable y reusable. | Max-closure transform | MR-153, MR-154 | El problema transformado conserva audit trail, pesos y precedencias, y puede verificarse con instancias pequeñas conocidas. |
| 156 | MR-156 | `[ ]` | P1 | Planning | Backend exacto de ultimate pit | Incorporar un backend exacto inicial para pit final basado en max-flow / pseudoflow o equivalente, detrás de una API estable de solver. | Exact UPL solver | MR-155 | El SDK resuelve instancias de prueba, produce memberships reproducibles y mejora explícitamente sobre el prototipo heurístico actual. |
| 157 | MR-157 | `[ ]` | P1 | Planning | Generación de shells anidados | Implementar revenue-factor sweeps o parametric pit limits para producir familias anidadas de shells desde el solver exacto. | `PitShellSet` | MR-156 | El SDK genera shells anidados con metadata del método y métricas por shell, y puede exportarlos a contratos abiertos. |
| 158 | MR-158 | `[ ]` | P2 | Planning | Métricas y reportes de pit shells | Añadir tonelaje, metal, strip ratio, valor y deltas shell-to-shell para cada `PitShellSet`. | Pit shell metrics | MR-157 | Cada shell viene con métricas serializables, comparables y listas para reports, examples y benchmark harnesses. |
| 159 | MR-159 | `[ ]` | P2 | Benchmarks | IO y benchmarks abiertos de shells | Exponer IO abierto para shells/pit memberships y agregar fixtures/benchmarks sobre MineLib y Marvin cuando haya artefactos verificables. | Shell IO + benchmarks | MR-157, MR-158 | `mine-rs` exporta/importa shells en formatos abiertos y el benchmark harness compara memberships y métricas de pit. |

---

# EPIC — Pushbacks, fases y scheduling de largo plazo

## Objetivo

Pasar de shells y artefactos económicos a una ruta reproducible de pushbacks, fases y scheduling agregado de largo plazo con restricciones explícitas.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 160 | MR-160 | `[ ]` | P1 | Planning | Contratos de pushbacks y fases | Diseñar artefactos estables para `PushbackPlan` y `PhaseDesign`, separando shells, fases operativas y reglas de nesting/acceso. | Pushback / phase contracts | MR-157 | Los contratos diferencian shell, pushback y fase, son serializables y no dependen de heurísticas ocultas. |
| 161 | MR-161 | `[ ]` | P1 | Planning | Diseño de fases desde shells | Implementar una primera ruta explícita para derivar fases desde shells anidados, benches y reglas de continuidad/precedencia. | Phase design engine | MR-160 | El SDK genera fases auditables desde shells y documenta claramente qué reglas usa y qué limitaciones tiene. |
| 162 | MR-162 | `[ ]` | P1 | Planning | Contratos de scheduler de largo plazo | Definir el artefacto `LongTermSchedule` con capacidades mina/planta, precedencias, destinos, stockpiles y violaciones estructuradas. | Long-term schedule contract | MR-160, MR-153 | Existe un contrato serializable que puede representar asignaciones por periodo, destino y fase sin ambigüedad. |
| 163 | MR-163 | `[ ]` | P1 | Planning | Scheduler agregado determinista | Implementar una primera ruta determinista de scheduling agregado por bench/phase/shell con restricciones de tonelaje, precedencia y avance vertical. | Aggregated LOM scheduler | MR-161, MR-162 | El scheduler genera periodos reproducibles, reporta violaciones y puede validarse en instancias pequeñas y benchmarks abiertos. |
| 164 | MR-164 | `[ ]` | P2 | Planning | Scheduling con destinos y stockpiles | Extender el scheduler para considerar ruteo a destinos, balances de stockpile y reclaim básico sin romper la reproducibilidad. | Destination-aware schedule | MR-152, MR-163 | El schedule puede separar flujos por destino y stockpile, con balances explícitos y reportes serializables por periodo. |
| 165 | MR-165 | `[ ]` | P1 | Economics | Evaluación económica de schedule | Conectar `LongTermSchedule` con cashflow, NPV, metal y métricas de negocio por periodo usando el `EconomicBlockModel`. | LOM economic evaluator | MR-153, MR-163 | Un usuario puede evaluar un schedule completo y obtener KPIs técnicos/económicos reproducibles por periodo y escenario. |
| 166 | MR-166 | `[ ]` | P1 | Economics | Packs de sensibilidad y escenarios | Agregar packs de sensibilidad para precio, recovery, costos, capacidades y reglas de scheduling, preservando comparación serializable entre escenarios. | Scenario sensitivity packs | MR-165 | El SDK ejecuta escenarios parametrizados y produce `ScenarioComparisonReport` con deltas claros de NPV, cashflow y producción. |
| 167 | MR-167 | `[ ]` | P2 | Benchmarks | Harness end-to-end de MineLib | Construir un harness end-to-end desde `EconomicBlockModel` hasta `LongTermSchedule` sobre instancias abiertas de MineLib/Newman y paridad Marvin cuando sea posible. | End-to-end benchmark harness | MR-159, MR-163, MR-165 | Existen ejemplos y comparadores por etapa para pit, shells, pushbacks, schedule y reportes económicos. |

---

# EPIC — Incertidumbre y métodos avanzados

## Objetivo

Preparar la ruta P2/P3 para incertidumbre geológica y planeamiento robusto sin acoplar métodos estocásticos al core determinista inicial.

| Orden | Id ticket | Estado | Prioridad | Area | Titulo | Descripcion | Entregable principal | Depende de | Criterio de aceptacion |
| ----: | --------- | ------ | --------- | ---- | ------ | ----------- | -------------------- | ---------- | ---------------------- |
| 168 | MR-168 | `[ ]` | P2 | Estimation | Contratos para realizaciones condicionales | Diseñar artefactos y metadatos para manejar múltiples realizaciones geológicas sin romper los contratos base del SDK. | Conditional realization contracts | MR-145 | Las realizaciones pueden almacenarse y evaluarse de forma consistente, con lineage explícito y sin mezclar determinismo con sampling implícito. |
| 169 | MR-169 | `[ ]` | P3 | Estimation | Prototipos SGS y SIS | Implementar prototipos experimentales de sequential Gaussian simulation y sequential indicator simulation como capa avanzada sobre el engine de estimación. | SGS / SIS prototypes | MR-168, MR-142 | El SDK puede producir ensembles pequeños reproducibles con seeds explícitos y validación mínima de estadísticos globales. |
| 170 | MR-170 | `[ ]` | P2 | Economics | Métricas de riesgo y valuación robusta | Añadir métricas de riesgo tipo P10/P50/P90, downside, CVaR u otras sobre escenarios y realizaciones. | Risk-aware valuation | MR-166, MR-168 | Los reports económicos pueden resumir distribución y riesgo de valor sin esconder supuestos de sampling. |
| 171 | MR-171 | `[ ]` | P3 | Planning | Pit y scheduling estocásticos experimentales | Explorar prototipos de pit final y scheduling bajo incertidumbre usando realizaciones y métricas de riesgo, sin prometer todavía un solver industrial completo. | Stochastic planning prototypes | MR-169, MR-170, MR-156, MR-163 | Existen prototipos documentados, ejemplos pequeños y criterios explícitos para decidir si conviene seguir hacia una ruta exacta/decomposed posterior. |



