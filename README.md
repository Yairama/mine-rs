# mine-rs

`mine-rs` es un proyecto open source para construir infraestructura computacional minera: un SDK reproducible, programable y auditable para trabajar con modelos de bloques, validación, reblocking, analytics, economía y planeamiento.

La apuesta no es empezar como otra suite minera cerrada, sino como una base reusable sobre la que puedan construirse librerías, notebooks, automatizaciones, tools deterministas y, más adelante, experiencias agenticas o interfaces de usuario.

> Estado actual: `mine-rs` ya puede tratarse como un SDK `alpha` con base funcional en Rust y Python. Aun así, sigue en una etapa temprana: la API pública todavía está consolidándose y la documentación mezcla capacidades ya implementadas con dirección objetivo y roadmap; cuando un ejemplo aún no existe, se marca como conceptual.

> Alcance recomendado del producto actual: ver [`docs/references/sdk-alpha-scope.md`](docs/references/sdk-alpha-scope.md).

> Garantías exactas de versionado y releases `0.x`: ver [`docs/references/alpha-release-policy.md`](docs/references/alpha-release-policy.md).

> Lectura transversal de madurez: ver [`docs/references/maturity-matrix.md`](docs/references/maturity-matrix.md) para distinguir qué superficies ya deben leerse como SDK usable, cuáles siguen experimentales y cuáles son benchmark-side o de investigación.

> Guardrail público de performance: ver [`docs/references/public-performance-baseline.md`](docs/references/public-performance-baseline.md) para la baseline de workflows públicos del SDK `alpha`, separada del material benchmark-side de diagnóstico y comparabilidad.

## Qué estamos construyendo

`mine-rs` busca convertirse en la capa base abierta para workflows mineros modernos:

- Un **Rust core** rápido, seguro y determinista para cómputo crítico.
- Un **Python SDK** ergonómico para ingenieros de minas, geólogos y analistas.
- Un set de **tools deterministas** para automatización y agentes.
- Contratos y formatos abiertos para integrar datos mineros con ecosistemas modernos.
- Una base reusable para productos internos, investigación aplicada y software minero futuro.

## Por qué existe

En minería, mucha lógica crítica todavía vive:

- atrapada dentro de suites propietarias;
- repartida entre planillas, macros y scripts difíciles de auditar;
- desconectada de ecosistemas modernos como Arrow, Parquet, pandas o pipelines reproducibles;
- poco preparada para automatización confiable y uso por agentes.

`mine-rs` existe para ofrecer una alternativa abierta y componible: que la lógica minera pueda versionarse, inspeccionarse, testearse y reutilizarse fuera de una GUI monolítica.

## Qué no es

`mine-rs` no busca, al menos en esta etapa:

- reemplazar de inmediato suites comerciales completas como Vulcan, Surpac, Deswik, Datamine, MinePlan o Micromine;
- priorizar una GUI como producto principal;
- prometer optimización minera avanzada completa desde el día uno;
- delegar la verdad técnica a agentes o prompts.

El foco inicial es mucho más concreto: **ser el mejor core open source para block models y workflows mineros reproducibles**.

## Principios

- **Open source primero:** el valor central es transparencia, extensibilidad y auditabilidad.
- **Rust-first core:** el cómputo crítico debe vivir en módulos Rust testeables, rápidos y mantenibles.
- **Python-first UX:** la experiencia principal para usuarios finales debe ser una librería Python clara y usable desde notebooks.
- **Determinismo:** los cálculos mineros deben ser reproducibles y verificables.
- **SDK antes que aplicación:** el proyecto prioriza primitives, APIs y tools reutilizables.
- **Interoperabilidad:** los datos deben moverse con facilidad entre workflows reales.
- **AI-native, no AI-dependent:** los agentes pueden orquestar y explicar, pero no reemplazar los motores deterministas.

## Dónde queremos ganar

La oportunidad de `mine-rs` no está en copiar una suite cerrada pantalla por pantalla. Está en ofrecer una base abierta mejor para:

- block models programables y serializables;
- validación estructurada y reportes auditables;
- interoperabilidad con CSV, Parquet, Arrow, pandas y numpy;
- workflows reproducibles en notebooks, scripts y pipelines;
- tools deterministas listas para automatización y sistemas agenticos;
- extensibilidad comunitaria sin encierro tecnológico.

## Arquitectura objetivo

```text
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

La capa agentica futura estará inspirada en `github.com/langchain-ai/deep-agents-from-scratch/`, con VFS, task tools, subagents especializados y contratos estructurados. Aun así, los agentes no deberían ejecutar lógica minera de forma implícita: deben llamar tools del SDK y producir artefactos verificables.

## Capacidades objetivo

El SDK apunta a cubrir progresivamente dominios clave para ingeniería de minas:

- Modelos de bloques y metadata.
- Conversión espacial `xyz ↔ ijk`.
- Indexación regular, rotada y sparse.
- Validación estructural de modelos.
- Reblocking, agregaciones ponderadas y reconciliación.
- IO con CSV, Parquet, Arrow y exportaciones visuales.
- Curvas ley-tonelaje y cálculo de metal.
- Primitives de planeamiento, benches, fases y precedencias.
- Simulación de secuencias de minado y escenarios.
- Pushbacks y evaluación económica.
- Tools deterministas para automatización y agentes.

## Foco actual

La prioridad del proyecto hoy es consolidar una columna vertebral útil y abierta para ingeniería de minas:

1. `BlockModel` y tipos de dominio claros.
2. Indexación `xyz ↔ ijk` y operaciones espaciales deterministas.
3. Validación estructural con reportes serializables.
4. IO abierto e interoperabilidad con formatos de uso real.
5. Analytics mineros base, economía y primitives iniciales de planeamiento.
6. Python SDK usable antes de expandir la capa agentica.

Ese orden es intencional: primero cálculo confiable y contratos sólidos; después más automatización, más tooling y una capa agentica más rica.

## Workflow Python recomendado hoy

Hoy el camino público soportado para usuarios Python debe entenderse como:

1. `load_from_pandas(...)` o `load_from_numpy(...)`
2. `validate()`
3. `summary()` / `basic_statistics()` / `grouped_statistics()` / `grade_tonnage()`
4. `export_to_pandas(...)` o `export_to_numpy(...)`

Ejemplo ejecutable del workflow actual:

```python
from miners import export_to_pandas, load_from_pandas

model = load_from_pandas(
    dataframe=df,
    grid=grid,
    schema=schema,
    metadata={"source": "notebook"},
)

report = model.validate()
summary = model.summary()
stats = model.basic_statistics("tonnes")
by_domain = model.grouped_statistics("domain", "tonnes")
curve = model.grade_tonnage("cu", "tonnes", [0.3, 0.5, 0.7])
exported = export_to_pandas(model, columns=["cu", "tonnes", "domain"])
```

Este es el flujo actual que la documentación debe promover. La API fluent o wrappers encadenables viven en `miners.experimental` y siguen siendo opt-in, no el camino por defecto para usuarios generales.

## Contrato actual de errores en Python

La superficie pública actual expone una sola excepción Python: `miners.MineError`.

Ese error único cubre hoy las mismas categorías base que existen en Rust:

- `Io`
- `Schema`
- `Grid`
- `Validation`
- `Reblock`
- `Economics`
- `Planning`
- `InvalidParameter`
- `Numeric`

La raíz `miners` no publica todavía una jerarquía de excepciones más fina (`SchemaError`, `GridError`, etc.). Si más adelante aparece, deberá documentarse como una ampliación del contrato; hoy el contrato público correcto es capturar `miners.MineError`.

Las validaciones ordinarias del modelo no siguen ese camino por defecto: `validate()` y los helpers relacionados devuelven un `ValidationReport` estructurado para inspección, serialización y tabulación. En otras palabras, los hallazgos esperables de calidad o consistencia del modelo viven en el reporte; `MineError` queda para fallas de operación, contrato o parámetros que impiden ejecutar correctamente el workflow solicitado.

## Dónde empezar con ejemplos Python ejecutables

Si quieres descubrir el SDK Python alpha por casos de uso reales, empieza por `examples/python/`.

Hoy ese pack ejecutable cubre:

- [`examples/python/pandas_load_validate_analyze_export.py`](examples/python/pandas_load_validate_analyze_export.py): flujo público `load -> validate -> analyze -> export` con `pandas`.
- [`examples/python/numpy_load_validate_export.py`](examples/python/numpy_load_validate_export.py): carga y exportación desde `numpy` usando la raíz `miners`.
- [`examples/python/tools_workflow.py`](examples/python/tools_workflow.py): uso básico de `miners.tools` sobre un `BlockModel` pequeño.

La regla de lectura es simple: `README.md` explica qué flujo público conviene usar, `examples/python/` muestra cómo correrlo y los ejemplos conceptuales quedan reservados para documentar APIs futuras. Si un snippet depende de wrappers fluent o rutas todavía opt-in, debe quedar separado y marcado como experimental en `miners.experimental`.

## Documentación

| Documento | Propósito |
| --- | --- |
| [`AGENTS.md`](AGENTS.md) | Guía operativa para agentes que trabajen en este repositorio. |
| [`docs/backlog.md`](docs/backlog.md) | Backlog final operativo con epics, tareas, dependencias y criterios de aceptación. |
| [`docs/references/vision.md`](docs/references/vision.md) | Misión, visión y principios del proyecto. |
| [`docs/references/product-scope.md`](docs/references/product-scope.md) | Alcance funcional, límites y etapas del producto. |
| [`docs/references/domain-capabilities.md`](docs/references/domain-capabilities.md) | Mapa de capacidades mineras objetivo. |
| [`docs/references/mining-engine-roadmap.md`](docs/references/mining-engine-roadmap.md) | Ruta end-to-end basada en literatura para economía, pit final, pushbacks y LOM. |
| [`docs/references/benchmark-diagnosis.md`](docs/references/benchmark-diagnosis.md) | Diagnóstico benchmark-side del estado MineLib/Marvin/McLaughlin; para la tabla canónica de paridad, ver `literature-parity.md`. |
| [`docs/references/public-performance-baseline.md`](docs/references/public-performance-baseline.md) | Baseline pública de performance para workflows del SDK `alpha`, usada como guardrail de producto y lectura de regresiones, no como benchmark-side de investigación. |
| [`docs/references/literature-parity.md`](docs/references/literature-parity.md) | Fuente canónica de la paridad mine-rs vs resultados publicados MineLib, validada por test contra los reportes JSON. |
| [`docs/references/architecture.md`](docs/references/architecture.md) | Arquitectura técnica por capas y módulos. |
| [`docs/references/repository-strategy.md`](docs/references/repository-strategy.md) | Decisión sobre monorepo, crates Rust, paquetes Python y capa agentica. |
| [`docs/references/sparse-blockmodel-design.md`](docs/references/sparse-blockmodel-design.md) | Diseño experimental de materialización sparse en `BlockModel`. |
| [`docs/references/python-sdk-design.md`](docs/references/python-sdk-design.md) | Diseño de experiencia Python y criterios de API. |
| [`docs/references/sdk-alpha-scope.md`](docs/references/sdk-alpha-scope.md) | Alcance oficial del SDK `alpha`, incluyendo superficies recomendadas, experimentales e internas. |
| [`docs/references/alpha-release-policy.md`](docs/references/alpha-release-policy.md) | Garantías de versionado, compatibilidad y gates mínimos para releases `0.x`. |
| [`docs/references/maturity-matrix.md`](docs/references/maturity-matrix.md) | Matriz canónica de madurez para distinguir superficies estables para SDK, experimentales y benchmark-side. |
| [`docs/references/agentic-layer.md`](docs/references/agentic-layer.md) | Diseño conceptual de la capa agentica futura. |
| [`docs/references/roadmap.md`](docs/references/roadmap.md) | Roadmap narrativo por fases. |
| [`docs/references/temporal-backlog.md`](docs/references/temporal-backlog.md) | Backlog temporal original conservado como referencia histórica. |

## Dependencias base del workspace

La fundación del workspace ya reserva el set base de dependencias para las siguientes fases:

| Dependencia | Rol previsto |
| --- | --- |
| `serde` | Serialización de contratos y tipos públicos. |
| `serde_json` | Intercambio JSON para reports y tools deterministas. |
| `thiserror` | Errores públicos tipados en crates del SDK. |
| `anyhow` | Propagación ergonómica de errores en binarios y herramientas auxiliares. |
| `tracing` | Telemetría y diagnósticos estructurados. |
| `rayon` | Paralelismo de datos para operaciones sobre modelos grandes. |
| `arrow` | Base columnar en memoria para atributos de block models. |
| `parquet` | IO columnar abierto para datasets y artefactos reproducibles. |
| `nalgebra` | Geometría, rotaciones y primitivas matemáticas espaciales. |
| `pyo3` | Bindings Rust ↔ Python sin duplicar lógica minera. |
| `maturin` | Tooling externo para compilar y empaquetar `mine-python` cuando se habilite la superficie Python real. |

## Uso esperado a futuro

La experiencia objetivo de mediano plazo desde Python será similar a trabajar con una librería técnica más amplia que el workflow público actual:

```python
# Ejemplo conceptual: no representa el camino público recomendado hoy.
from miners import BlockModel

model = BlockModel.read_parquet("block_model.parquet")
report = model.validate()
curve = model.grade_tonnage(grade="cu", tonnage="tonnes")

scenario = model.plan.sequence(
    bench_height=10,
    max_vertical_advance=30,
)
```

Este ejemplo representa la intención de diseño: APIs legibles, reproducibles y conectadas con objetos mineros reales.

## Relación con agentes

Sobre el SDK se planea construir un sistema agentico capaz de recibir un modelo de bloques, inspeccionarlo, crear tareas, delegar a subagents y ejecutar tools del SDK para producir reportes, escenarios y artefactos persistentes.

La regla central es:

```text
Los agentes razonan y orquestan.
El SDK calcula y valida.
```

## Estado del repositorio

Actualmente el repositorio ya cuenta con:

- workspace Rust inicial;
- crates base `mine-core`, `mine-blockmodel`, `mine-indexing`, `mine-io`, `mine-economics`, `mine-validation`, `mine-reblock`, `mine-planning`, `mine-sdk`, `mine-tools` y `mine-python`;
- packaging Python base con `maturin`;
- bindings Python ejecutables para `Coordinate3D`, `BlockDimensions`, `GridDefinition`, `ColumnSchema`, `BlockModel`, `ModelSummary`, `ValidationReport` y analytics base;
- value objects fundacionales de core (`MineError`, IDs, coordenadas, grilla, metadata y schema);
- un `BlockModel` en memoria con storage columnar inicial, selección de columnas, filtros básicos y una layout experimental sparse basada en índices lineales materializados;
- un crate `mine-indexing` con conversiones `xyz ↔ ijk`, indexación lineal y vecindad 6/18/26 con filtro sparse opcional, incluyendo soporte de rotación en XY para grillas regulares.
- un crate `mine-io` con lectura y escritura CSV para `BlockModel` usando schema, grilla e índices `i/j/k` explícitos, con errores claros para columnas faltantes, duplicados y gaps, además de un exporter Vulcan CSV configurable con columnas `x/y/z`, aliases y formato booleano.
- un crate `mine-io` con lectura y escritura Parquet preservando grilla, schema y metadata del modelo mediante metadata Arrow/Parquet estándar, con archivos legibles por readers Arrow compatibles.
- un backend Arrow inicial en `mine-io` con conversión `BlockModel <-> RecordBatch`; por ahora esta ruta **copia** datos entre el storage columnar actual y buffers Arrow tipados, dejando una integración de menor copia para una etapa posterior y rechazando explícitamente layouts sparse hasta definir una representación columnar equivalente.
- una inferencia controlada de schema en `mine-io` para CSV y Parquet, con tipos inferidos cuando es razonable pero **sin asumir silenciosamente** columnas críticas como índices espaciales, leyes o tonelaje; esos casos quedan resueltos por hints explícitos o warnings estructurados.
- un exporter VTU ASCII en `mine-io` para visualizar `BlockModel` regulares en ParaView, con geometría hexaédrica por bloque y columnas seleccionadas compatibles con VTK (`float`, `integer`, `boolean`); por ahora no soporta columnas de texto, grillas rotadas ni layouts sparse.
- un crate `mine-economics` con `EconomicAssumptions` validados y evaluación determinista de **revenue, costo y margen por bloque** con chequeo explícito de unidades de ley y tonelaje.
- un engine financiero inicial en `mine-economics` para **cashflow por periodo** y **NPV** sobre `MiningScenario`, usando inputs explícitos por periodo y una convención documentada de descuento.
- un crate `mine-validation` con `ValidationReport` serializable, opciones configurables de validación y una suite actual de schema, consistencia de grilla regular, detección de duplicados pre-materialización, bloques faltantes, extents observados rotación-aware y valores críticos para `BlockModel`, reutilizada también desde `mine-tools` y la capa Python.
- un crate `mine-reblock` con `AggregationRules` declarativas para suma, promedio ponderado, min, max, first, majority y operaciones custom numéricas limitadas, además de una agregación ponderada reusable sobre slices opcionales y columnas de `BlockModel` para leyes, densidades y variables continuas, un `superblock(...)` determinista para grillas alineadas que conserva tonelaje y metal mediante reglas explícitas, un `subblock(...)` inicial con `DistributionRules` explícitas (`split_equally` y `replicate`) para subdividir modelos alineados preservando variables conservativas, y un `reconcile_models(...)` serializable para cuantificar masa, metal, ley media y cambios de bloques before/after con tolerancias explícitas.
- un prototipo experimental `build_adaptive_reblock_prototype(...)` en `mine-reblock` para planificar estrategias variables por zona/dominio con reglas explícitas, resúmenes por zona y limitaciones documentadas, sin ejecutar todavía reblocking mixto automático.
- analytics base en Rust para `BlockModel`: estadísticas básicas, agregación por grupos y curva ley-tonelaje con cutoffs explícitos, ya expuestos también en Python.
- un frente inicial de estimación en `mine-blockmodel` con compositing determinista, auditoría de dominios, declustering, histogramas ponderados, variografía experimental, fitting de modelos variográficos autorizados (`nugget`, `spherical`, `exponential`, `gaussian`), una API de neighborhoods/passes con anisotropía explícita, estimadores puntuales base (`nearest neighbour` e `inverse distance weighting`) y kriging puntual (`ordinary` y `simple`) con pesos y varianzas serializables.
- interoperabilidad `pandas` y `numpy` inicial en Python para cargar modelos con `load_from_pandas(...)` / `load_from_numpy(...)` y exportarlos con `export_to_pandas(...)` / `export_to_numpy(...)`; hoy estos puentes priorizan seguridad y simplicidad, por lo que pueden copiar datos entre Rust y Python.
- una API Python experimental `miners.experimental.experimental_workflow(...)` que encadena validación, summary, estadísticas, grouped statistics, curva ley-tonelaje y export a pandas sin ocultar columnas críticas; queda marcada como wrapper experimental separado de la superficie recomendada en la raíz `miners`.
- un crate `mine-planning` con generación determinista de bancos, phase tagging, `MiningScenario` serializable, `PrecedenceGraph` acíclico y `Schedule` mínimo con restricciones básicas de tonelaje y avance vertical.
- un prototipo experimental de pushbacks en `mine-planning` derivado desde `Schedule` mediante `build_pushback_prototype(...)`, agrupando por fase y explicitando limitaciones y siguientes pasos sin prometer optimización.
- un crate `mine-tools` con contrato común serializable y tools iniciales `inspect_model`, `validate_model`, `query_blocks`, `aggregate_blocks`, `grade_tonnage`, `create_scenario`, `evaluate_scenario` y `compare_scenarios`, con evaluación financiera explícita por periodo y comparación estructurada de reportes.

Las siguientes etapas deben profundizar Arrow IPC cuando haga falta un artefacto binario estable, soporte BDF experimental, evaluación técnico-económica más granular conectada a `Schedule` y una extensión del reblocking hacia distribuciones/adaptaciones más flexibles.

### Ejemplo de export Vulcan CSV

```rust
use std::collections::BTreeMap;

use mine_sdk::{
    core::ColumnId,
    io::{
        CsvIndexColumns, VulcanBooleanFormat, VulcanCoordinateColumns, VulcanCsvWriteOptions,
        write_block_model_vulcan_csv,
    },
};

let options = VulcanCsvWriteOptions::new(
    VulcanCoordinateColumns::new("xworld", "yworld", "zworld")?,
    Some(CsvIndexColumns::new("ix", "iy", "iz")?),
    Some(vec![ColumnId::new("bench")?, ColumnId::new("cu")?]),
    BTreeMap::from([(ColumnId::new("bench")?, "bench_rl".to_owned())]),
    VulcanBooleanFormat::ZeroOne,
)?;

write_block_model_vulcan_csv(&model, "model_vulcan.csv", &options)?;
```

## Desarrollo local

Comandos base del workspace:

```powershell
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --workspace
cargo bench -p mine-benchmarks
```

Convención de tests Rust del workspace: dejar tests unitarios mínimos junto al módulo en `src/` y mover pruebas de flujo público, roundtrips y cobertura de integración a `crates/*/tests/`, con helpers compartidos en `tests/common` cuando aparezca duplicación real.

La suite criterion vive en `benchmarks/` (`mine-benchmarks`, MR-216) y cubre microbenchmarks de las operaciones críticas actuales: indexación `xyz↔ijk`, precedencias con la plantilla Marvin de 17 offsets, solver exacto de UPL (Dinic), shells anidados, scheduler `ready frontier` y heurística CPIT TopoSort. Los microbenchmarks miden operaciones aisladas sobre fixtures sintéticos; los pipelines completos sobre instancias MineLib reales se miden con los harnesses de runtime descritos abajo (telemetría MR-215).

Como complemento product-side, la baseline pública de performance vive en [`docs/references/public-performance-baseline.md`](docs/references/public-performance-baseline.md). Ese documento fija guardrails para workflows públicos del SDK `alpha` como carga, validación y analytics base; no reemplaza ni resume por sí solo el material benchmark-side de runtime, comparabilidad o diagnóstico descrito en esta sección.

Para el benchmark local de Marvin ya existen ejemplos ejecutables para inspeccionar `datasets/benchmarks/marvin/marvin.blocks`, generar un `prec` abierto y comparar referencias Marvin locales contra las salidas actuales de `mine-rs`:

```powershell
cargo run -p marvin-inspect
cargo run -p marvin-prec
cargo run -p marvin-planning
cargo run -p marvin-benchmark
cargo run -p marvin-benchmark --bin multi_mine_scheduler
cargo run -p stochastic-planning
cargo run --release -p marvin-benchmark --bin upit_runtime -- [--include-full]
cargo run --release -p marvin-benchmark --bin cpit_toposort -- [--include-full]
cargo run --release -p marvin-benchmark --bin pcpsp_toposort -- [--include-full]
cargo run --release -p marvin-benchmark --bin pcpsp_bound -- [--include-full]
```

Los cuatro últimos bins versionan reportes con telemetría de runtime por etapa (MR-215):

- `upit_runtime` (MR-209) mide el solver exacto de UPIT (Dinic) sobre `marvin`, `mclaughlin-limit` y, con `--include-full`, sobre `mclaughlin` completo (2.14M bloques / 73.1M aristas), comparando el valor del pit contra el objetivo oficial MineLib. Reporte: `datasets/benchmarks/outputs/upit-runtime-report.json`. Estado actual: paridad exacta en las tres instancias.
- `cpit_toposort` (MR-211) construye un candidato CPIT propio con la heurística TopoSort core (`solve_cpit_with_toposort`, Chicoisne et al. 2012, doi 10.1287/opre.1120.1072) ordenada por el tiempo esperado de extracción de la relajación LP staged (`*.LPcpit`), audita recursos y precedencias, y compara contra el best-known staged 2012. Reporte: `datasets/benchmarks/outputs/cpit-toposort-report.json`. Estado actual: la variante con retraso de estéril supera el incumbent staged en las tres instancias y queda a 0.2–3.7% del bound LP publicado; el bound LP propio sigue pendiente (MR-213).
- `pcpsp_toposort` (MR-212) extiende la heurística al caso multi-destino PCPSP con decisión de destino durante la construcción (`solve_pcpsp_with_toposort`: valor descontado máximo entre pares destino/periodo factibles, de modo que el mineral espera capacidad de planta en vez de perder valor en botadero). Reporte: `datasets/benchmarks/outputs/pcpsp-toposort-report.json`. Estado actual: Marvin 829.5M (gap 6.37% vs 886.0M oficial, cumpliendo los dos primeros hitos de MR-212; el candidato exploratorio anterior quedaba en 664.2M / 25%), McLaughlin Limit 1,072.5M (gap 18.85% con ordering proxy `LPcpit` documentado).
- `pcpsp_bound` (MR-213) calcula bounds superiores propios con la relajación Lagrangiana de capacidades del core (`compute_pcpsp_lagrangian_bound`): el subproblema interno es un max-closure exacto tiempo-expandido con el 100% de las precedencias en cada iteración (sin checkpoints parciales; Geoffrion 1974, Dagdelen & Johnson 1986), y deriva además candidatos TopoSort **self-contained** ordenados por la propia relajación, sin consumir las relajaciones LP staged de MineLib. Reporte: `datasets/benchmarks/outputs/pcpsp-bound-report.json`. Estado actual: bounds Marvin +16.9%/+15.2% (120 iteraciones, ~4.7 s/iter con el Dinic denso interno) y McLaughlin Limit +4.0%/+6.8% (12 iteraciones) sobre los LP oficiales (gap dual restante por presupuesto finito de subgradiente, declarado en el artefacto; el resultado expone `best_multipliers` para warm-start); el candidato CPIT self-contained de Marvin (841.6M, LP gap 2.59%) supera al candidato con ordering LP staged.

La fuente canónica única del estado de paridad contra la literatura vive en `docs/references/literature-parity.md` (MR-217); README y diagnóstico solo la apuntan, y la tabla se valida automáticamente contra los reportes JSON con `cargo test -p marvin-benchmark --test literature_parity`.

Para estos bins benchmark-side, la política de rutas es explícita: los datasets/references/outputs por defecto salen desde la raíz del repo, las rutas absolutas CLI se respetan tal cual y cualquier ruta relativa provista por CLI se rebasea también contra la raíz del repo para que los comandos documentados funcionen igual desde el workspace root.

El estado actual de paridad del benchmark Marvin queda versionado en `datasets/benchmarks/marvin/outputs/parity-report.json`, y la comparación reproducible hoy disponible queda registrada en `datasets/benchmarks/marvin/outputs/comparison-report.json`.

Cuando solo haga falta refrescar la evidencia LP/BZ de MR-187 sin ejecutar todas las baselines/sweeps pesadas, `marvin-benchmark` también acepta `--mode focused-mr187` (o `MARVIN_BENCHMARK_MODE=focused-mr187`) y escribe por defecto `datasets/benchmarks/marvin/outputs/mr187-focused-refresh-report.json`. Ese modo deja intacto el modo `full`, conserva los artefactos LP/BZ relevantes para backlog refresh, explicita cuándo neutraliza comparaciones no recalculadas para mantener la corrida acotada y ahora serializa las mismas superficies raíz de protocolo (identidad explícita + `benchmark_contract_audit`, `benchmark_contract_roles`, `diagnostics_schema`, `diagnostic_groups_present`) que usa la lectura comparativa del benchmark-side. Además, la procedencia paper-like de Marvin ya no se deja implícita como `cpit-solution`: los artefactos exponen un contrato benchmark-side explícito `shells -> pushbacks -> mining-cuts -> scheduling`, junto con el resumen de procedencia y los gaps de comparabilidad que todavía bloquean una clasificación `paper-comparable`.

La validación multi-mine actual del scheduler queda versionada en `datasets/benchmarks/outputs/multi-mine-scheduling-report.json` y ejecuta la misma ruta core (`SchedulingProblem` + `solve_decomposed_scheduling_problem`) sobre Marvin, `mclaughlin-limit` y la instancia local `mclaughlin-full`, con solo configuración explícita de columnas/recursos cuando MineLib cambia semánticas de dataset. Hoy la agregación intermedia ya no usa bandas fijas de 4 bancos: Marvin promueve como ruta primaria una familia acotada `nested-shell × bench` derivada de escenarios revenue/cost-aware con acceso `strict sequential`, y `mclaughlin-limit` ya promueve una ruta `nested-shell × bench` reconstruida desde `*.upit` + precedencias MineLib, expuesta explícitamente como proxy benchmark-side de shells -> fases shell×bench pushback-equivalent -> scheduling. `mclaughlin-full` queda separado como variante local de stress con fallback `reference-period × bench`, no como comparación literaria directa. El reporte deja explícito cuándo la corrida usa la variante bibliográfica `mclaughlin-limit` y cuándo usa la variante full local, declara la fuente exacta del conjunto de bloques que alimenta al scheduler, publica `selected_block_provenance_summary` + `selected_block_provenance_chain` para distinguir la cadena Marvin `shells -> pushbacks -> mining-cuts -> scheduling` del contrato equivalente benchmark-side que ahora declara `mclaughlin-limit`, y ahora además versiona `primary_unit_family_traceability` para cuantificar el puente `selected blocks -> phase-plan proxy -> scheduling units` junto con evidencia estructurada `benchmark_side_evidence` para dejar explícito si la ruta activa ya tiene mining-cut benchmark-side y/o sidecar LP/BZ o si todavía se queda en la familia shell×bench proxy. En Marvin, el probe competitivo LP/BZ ya no deja el gap solo como una lectura cualitativa: ahora clasifica el bloqueo dominante entre `precedence-coverage`, `budget-depletion`, `round-repair-local-search-mismatch` y `schedule-level-proof-only`, publica además un `budget_coverage_experiment` auditable para distinguir si conviene priorizar expansión de cobertura, de presupuesto o si ninguno domina, y mantiene `parity_claim_status = diagnostic-only` hasta que exista evidencia reproducible de competitividad real. En `mclaughlin-limit`, ese bloque benchmark-side ya no queda solo en readiness/scaffold: además de la traza `pushback-equivalent-bench-cut-readiness`, el reporte ahora versiona un contrato explícito de benchmark-side mining-cut refinement (`mclaughlin-limit-pushback-bench-localized-mining-cuts` / build `front3-ar2.0-span2-n6-limit`) sobre la familia `shell × bench`, manteniendo el scope `mclaughlin-limit-only` y dejando explícito que sigue siendo una capa benchmark-side, no un generador paper-grade compartido. El scaffold `mclaughlin-limit-cut-sidecar-scaffold` se conserva para preparar las promociones futuras del sidecar LP/BZ, junto con prerequisitos estructurados (`benchmark_cut_prerequisites`, `lp_bz_sidecar_prerequisites`), `benchmark_cut_promotion_ready`, `lp_bz_sidecar_promotion_ready`, `*_blocking_prerequisite_ids`, reglas explícitas de evaluación (`benchmark_cut_promotion_rule`, `lp_bz_sidecar_promotion_rule`) y listas auditables (`benchmark_cut_exit_criteria`, `lp_bz_sidecar_exit_criteria`). Esa misma ruta ya puede además publicar un primer sidecar LP/BZ parcial y auditable sobre el kernel relajado `shell × bench`, restringido explícitamente a `mclaughlin-limit-only`, con `solve_status`, `coverage_completeness`, `coverage_basis_points` y gaps de relajación; ese bound sigue siendo `diagnostic-only`, pero el contrato benchmark-side ya distingue expresamente el escalón intermedio `partial-bound-available` antes de cualquier promoción más fuerte. Ese camino también queda resumido en `mclaughlin_limit_promotion_checklist` (`mr207-v4`) para dejar explícito qué parte ya está auditada, qué sigue `scaffold-only`, qué regla bloquea cada promoción, qué exit criteria siguen pendientes y por qué `mclaughlin-full` sigue fuera como stress-only. El reporte incluye drift temporal candidato-vs-referencia por bloque, versiona una baseline `cpit-period-routed` para separar el gap de ruteo/destino del gap temporal del solver y además expone las relajaciones LP staged (`LPcpit` / `LPpcpsp`) cuando están disponibles en el dataset. Cada corrida sigue clasificada como **paper-comparable** o **exploratoria** según el pipeline realmente ejecutado.

Los artefactos externos/versionados de Marvin viven en `datasets/benchmarks/marvin/references/`, mientras que `datasets/benchmarks/marvin/outputs/` queda reservado para reportes generados por el repo. `marvin-planning` aplica el workflow experimental hoy disponible sobre Marvin: precedencias deterministas, `upit` heurístico, bancos geométricos, schedule por bancos y pushbacks sintéticos. `marvin-benchmark` normaliza y audita `prec`, `upit`, `cpit`, `pcpsp` y las relajaciones LP, incluye ahora una comparación **exacta** `mine-rs` vs UPIT Marvin basada en `marvin.upit` + `marvin.prec`, y además ejecuta rutas internas end-to-end de `mine-rs` para comparar candidatos propios contra las referencias **CPIT y PCPSP** cuando es posible. `stochastic-planning` agrega un prototipo pequeño de ranking estocástico: genera un ensemble SGS sintético, evalúa dos schedules candidatos con la economía/riesgo actual y deja explícitos los criterios de decisión y sus limitaciones. Estas salidas siguen siendo útiles para exploración y benchmarking interno, pero no deben interpretarse todavía como un solver estocástico industrial completo.

## Desarrollo local de Python

La base del packaging Python ya usa Maturin y expone el paquete `miners` desde `python/miners`.

El contrato local soportado para contributors es deliberadamente pequeño y explícito:

1. crear un `venv` limpio con Python `>=3.11`;
2. actualizar `pip` e instalar `maturin` dentro de ese entorno;
3. ejecutar `maturin develop` desde la raíz del repo;
4. correr la suite `unittest` pública del paquete Python.

Ese camino valida, sin depender de conocimiento tribal, que:

- `crates/mine-python` compila e instala el módulo nativo dentro del `venv`;
- `python/miners` queda importable como `miners` con las dependencias mínimas declaradas hoy (`numpy` y `pandas`);
- la superficie pública base del SDK Python sigue viva para el workflow recomendado `load -> validate -> analyze -> export`.

Hoy este es el flujo soportado para desarrollo local. No reemplaza todavía una disciplina completa de wheels y releases ni adelanta el trabajo futuro de `miners.tools`.

```powershell
python -m venv .venv
.\.venv\Scripts\python -m pip install --upgrade pip
.\.venv\Scripts\python -m pip install maturin
.\.venv\Scripts\python -m maturin develop
.\.venv\Scripts\python -m unittest discover -s tests -p "test_python_*.py"
```

El paquete local ya expone una superficie Python mínima usable:

```python
import miners

grid = miners.GridDefinition(
    origin=miners.Coordinate3D(0.0, 0.0, 0.0),
    block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
    shape=(2, 1, 1),
)

model = miners.BlockModel(
    grid=grid,
    schema=[
        miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
        miners.ColumnSchema("tonnes", "float", unit="t", mining_role="tonnage"),
    ],
    float_columns={
        "cu": [0.8, 1.1],
        "tonnes": [12.0, 15.0],
    },
)

print(model.summary().block_count)
print(model.validate().to_json())
```

## Guía para contribuir

Si vienes desde la comunidad open source, la mejor contribución no es agregar superficie por agregar, sino reforzar boundaries, determinismo y utilidad real para workflows mineros.

Usa esta referencia rápida para ubicar cada cambio en la capa correcta:

| Ubicación | Responsabilidad |
| --- | --- |
| `crates/mine-core` | Tipos y contratos deterministas compartidos. |
| Crates de dominio futuros | Lógica minera específica como block models, indexing o validación. |
| `crates/mine-sdk` | Fachada pública Rust para consumidores y capas superiores. |
| `crates/mine-tools` | Tools deterministas con contratos serializables para automatización. |
| `crates/mine-python` | Binding nativo PyO3/Maturin; no debe contener lógica minera crítica. |
| `python/miners` | Ergonomía Python, type hints y reexports para usuarios finales. |
| `python/mine-agents` | Orquestación agentica Python-first encima del SDK y de `mine-tools`. |

Reglas prácticas:

- Implementa la lógica minera primero en Rust.
- Reexporta desde `mine-sdk` antes de subir a Python.
- Mantén errores y outputs serializables.
- No mezcles dependencias agenticas dentro del SDK base.
- Prefiere formatos abiertos e integración con ecosistemas existentes antes que formatos cerrados.
- No conviertas el proyecto en una GUI-first app desde el README ni desde la implementación.

La fachada de `mine-sdk` ahora puede consumirse por dominio (`mine_sdk::blockmodel::BlockModel`, `mine_sdk::validation::ValidationOptions`, `mine_sdk::io::read_block_model_csv`, etc.) y también conserva los reexports planos existentes para compatibilidad.
