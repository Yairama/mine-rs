# Alcance oficial del SDK alpha

Estado de este documento: activo para la etapa `0.x`.

## Propósito

Este documento define qué debe entender un usuario cuando hoy se habla de `mine-rs` como un **SDK alpha**.

No busca prometer una estabilidad `1.0`. Busca hacer explícito:

- qué superficies ya forman parte del producto técnico usable;
- qué partes siguen siendo experimentales;
- qué piezas pertenecen a benchmarking, investigación o wiring interno;
- qué claims sí y no debe hacer hoy el proyecto.

## Declaración de estado

`mine-rs` ya puede presentarse como:

```text
SDK alpha para ingeniería de minas, con core Rust determinista,
superficie Python inicial usable y tools deterministas base.
```

No debe presentarse todavía como:

- SDK público estable `1.0`;
- suite minera completa;
- plataforma agentica lista para producción;
- scheduler industrial paper-comparable en todos sus frentes.

> Para una lectura transversal de madurez por superficie, ver la matriz canónica en [`docs/references/maturity-matrix.md`](maturity-matrix.md). Este documento define el alcance `alpha`; la matriz complementa ese alcance con etiquetas de madurez repo-wide sin duplicar detalle aquí.

## Qué significa `alpha` en `mine-rs`

En esta etapa `alpha` el proyecto sí promete:

- una arquitectura coherente y explícita;
- una superficie pública base ya usable;
- determinismo en el core y outputs serializables;
- documentación honesta sobre madurez y límites;
- evolución activa con cambios todavía posibles en API y ergonomía.

En esta etapa `alpha` el proyecto no promete:

- congelamiento fuerte de toda la API;
- compatibilidad total entre cualquier release `0.x` sin notas de migración;
- estabilidad uniforme en todos los módulos del repo;
- cierre completo del roadmap de planning avanzado o benchmarking académico.

> Las garantías exactas de versionado, compatibilidad y notas de release para esta etapa viven en [`alpha-release-policy.md`](alpha-release-policy.md).

## Superficie pública recomendada hoy

La superficie pública recomendada del SDK alpha se divide en tres niveles: Rust, Python y tools.

### 1. Rust recomendado

La entrada pública recomendada en Rust es `mine-sdk`.

Debe considerarse parte del SDK alpha, salvo indicación explícita en contrario, la superficie enfocada en:

- tipos core de dominio:
  - `Coordinate3D`
  - `BlockDimensions`
  - `GridDefinition`
  - IDs, metadata y schema de columnas
- `BlockModel` y summaries base;
- indexing `xyz ↔ ijk`, indexación lineal y vecindad;
- IO CSV y Parquet del modelo de bloques;
- validación estructurada y `ValidationReport`;
- analytics base y curva ley-tonelaje;
- reblocking básico y reconciliación;
- economía base y `EconomicBlockModel`;
- tools deterministas reexportadas o conectadas al SDK;
- primitives de planning básico ya formalizadas como contratos serializables: bancos/phase tagging básicos, `MiningScenario`, `PrecedenceGraph`, `Schedule` y sus reportes, cuando se usen con restricciones explícitas.

El import `mine_sdk::planning` no otorga una clasificación uniforme a todo el módulo. TopoSort CPIT/PCPSP, el bound Lagrangiano LP/BZ, el pseudoflow paramétrico, pushbacks, cuts y scheduling avanzado siguen siendo experimentales o benchmark-side según su uso.

Camino de imports recomendado para nuevo código Rust:

- `mine_sdk::core`
- `mine_sdk::blockmodel`
- `mine_sdk::io`
- `mine_sdk::validation`
- `mine_sdk::economics`
- `mine_sdk::planning`
- `mine_sdk::reblock`
- `mine_sdk::experimental` solo para prototipos opt-in

### 2. Python recomendado

La entrada pública recomendada en Python es `miners`.

Debe considerarse parte del SDK alpha, salvo indicación explícita en contrario, la superficie enfocada en:

- helpers públicos `load_from_pandas(...)`, `load_from_numpy(...)`, `export_to_pandas(...)` y `export_to_numpy(...)`;
- IO público `read_csv(...)`, `write_csv(...)`, `read_parquet(...)` y `write_parquet(...)`;
- construcción y lectura explícita de `BlockModel` cuando haga falta control más fino;
- tipos core expuestos a Python;
- indexing con `GridDefinition.xyz_to_ijk(...)`, `ijk_to_xyz(...)`, `ijk_to_linear(...)` y `linear_to_ijk(...)`;
- reblocking con `AggregationRule`, `DistributionRule`, `superblock(...)` y `subblock(...)`;
- la excepción pública `MineError` como contrato único actual para fallas operativas;
- `summary()`;
- `validate()` y `ValidationReport`;
- interoperabilidad base con pandas y numpy ya soportada por el repo;
- analytics públicos ya expuestos:
  - `basic_statistics()`
  - `grouped_statistics()`
  - `grade_tonnage()`

Camino recomendado hoy para usuarios Python:

1. `read_csv(...)`, `read_parquet(...)`, `load_from_pandas(...)` o `load_from_numpy(...)`
2. indexing explícito desde `GridDefinition` cuando el workflow lo requiera
3. `validate()`
4. `summary()` / `basic_statistics()` / `grouped_statistics()` / `grade_tonnage()`
5. `superblock(...)` o `subblock(...)` con reglas declarativas cuando cambie la resolución
6. `write_csv(...)`, `write_parquet(...)`, `export_to_pandas(...)` o `export_to_numpy(...)`

Este flujo forma parte de la superficie pública recomendada del SDK alpha. Los wrappers fluent o encadenables no deben presentarse como camino principal.

En manejo de errores, el contrato público actual también es deliberadamente simple: la raíz `miners` expone un único tipo de excepción pública, `MineError`. Ese tipo concentra categorías alineadas con Rust (`Io`, `Schema`, `Grid`, `Validation`, `Reblock`, `Economics`, `Planning`, `InvalidParameter`, `Numeric`), pero todavía no se presenta como una jerarquía Python separada.

Los hallazgos ordinarios de validación no deben confundirse con esa ruta de excepciones. El camino normal para revisar problemas de calidad o consistencia del modelo sigue siendo `validate()` + `ValidationReport`; levantar `MineError` queda reservado para fallas que impiden completar correctamente la operación pedida.

La evidencia ejecutable y notebook-first de este camino debe concentrarse en `examples/python/`, enlazada desde `README.md` como hub de descubrimiento. Hoy ese pack ya debe cubrir al menos `pandas_load_validate_analyze_export.py`, `numpy_load_validate_export.py` y `tools_workflow.py`, siempre usando la raíz `miners` o `miners.tools`; cualquier wrapper opt-in en `miners.experimental` queda fuera del camino principal salvo etiquetado experimental explícito.

### 3. Tools recomendadas

El set actual de tools deterministas forma parte del alcance alpha como superficie de automatización, con madurez base suficiente para uso técnico y futura integración agentica:

- `inspect_model`
- `validate_model`
- `query_blocks`
- `aggregate_blocks`
- `grade_tonnage`
- `create_scenario`
- `evaluate_scenario`
- `compare_scenarios`

Estas tools sí forman parte de la narrativa pública del SDK alpha, pero la futura experiencia agentica que las orqueste todavía no.

## Superficie experimental

Las siguientes áreas deben tratarse como **experimentales** aunque ya existan en el repo:

- APIs fluent o wrappers explícitamente marcados como experimentales;
- layouts o representaciones sparse todavía en evolución;
- TopoSort CPIT/PCPSP y el bound Lagrangiano LP/BZ;
- pseudoflow paramétrico y rutas optimizadas de shells todavía abiertas;
- rutas avanzadas de pushbacks, cuts y scheduling cuya madurez todavía dependa de cierres adicionales de comparabilidad;
- prototipos estocásticos;
- optimizaciones de performance cuya semántica pública aún no esté consolidada.

Regla práctica:

```text
Si una capacidad existe pero todavía no debería tomarse como contrato de producto
para usuario general, debe llamarse experimental de forma explícita.
```

Aplicación concreta de esa regla en la etapa actual:

- en Python, la raíz `miners` queda como superficie recomendada y las APIs opt-in viven en `miners.experimental`;
- el wrapper `miners.experimental.experimental_workflow(...)` sigue disponible como exploración avanzada, pero no reemplaza el flujo público `load -> validate -> analyze -> export`;
- en Rust, la raíz `mine_sdk` queda como superficie recomendada y los prototipos opt-in viven en `mine_sdk::experimental`;
- `ExperimentalVariogram*` permanece en la superficie recomendada porque allí "experimental" es terminología geostatística del dominio, no un marcador de estabilidad del SDK.

## Superficie benchmark-side, research o interna

Las siguientes piezas no deben presentarse como superficie principal del SDK alpha para usuarios generales:

- harnesses de MineLib, Marvin y McLaughlin;
- adapters específicos de benchmark;
- wiring de reportes de comparabilidad;
- sidecars LP/BZ o artefactos auxiliares de evaluación académica;
- módulos cuyo propósito principal sea investigación, diagnóstico o reproducción bibliográfica;
- placeholders de la futura capa agentica.

Estas piezas sí son estratégicamente valiosas para el proyecto, pero su valor principal hoy es:

- validación técnica;
- comparabilidad con literatura;
- investigación aplicada;
- soporte interno para evolución del core.

No deben confundirse con el camino principal de adopción del SDK alpha.

El guardrail público de performance en [`public-performance-baseline.md`](public-performance-baseline.md) complementa esta separación desde el lado producto, pero todavía no contiene la baseline cuantitativa versionada exigida por MR-229. Tampoco reemplaza los diagnósticos ni los artefactos benchmark-side usados para comparabilidad o investigación.

## Qué puede prometer hoy el proyecto

Hoy `mine-rs` sí puede prometer, con honestidad técnica:

- un core Rust reproducible para block models y operaciones relacionadas;
- un paquete Python alpha ya usable para workflows técnicos base;
- validación estructurada y outputs auditables;
- interoperabilidad con formatos abiertos de uso real;
- una base seria para automatización y crecimiento futuro.

## Qué no debe prometer hoy el proyecto

Hoy `mine-rs` no debe prometer:

- reemplazo total de suites comerciales completas;
- estabilidad fuerte de toda la API pública;
- capa agentica implementada o madura;
- comparabilidad paper-grade cerrada para todo scheduling avanzado;
- cobertura completa de todas las instancias y formulaciones MineLib como claim ya resuelto.

## Lectura recomendada por tipo de usuario

### Usuario Python técnico

Camino recomendado hoy:

1. `miners`
2. `examples/python/`
3. IO CSV/Parquet o carga pandas/numpy desde la raíz pública
4. indexing, validación, analytics y reblocking públicos según el workflow
5. escritura CSV/Parquet o exportación pandas/numpy

### Consumidor Rust

Camino recomendado hoy:

1. `mine-sdk`
2. módulos por dominio reexportados por el SDK
3. contratos serializables del core y tools

### Investigador o contributor de planning avanzado

Camino recomendado hoy:

1. `docs/backlog.md`
2. `docs/references/mining-engine-roadmap.md`
3. `docs/references/literature-parity.md`
4. harnesses benchmark-side del repo

## Regla de comunicación para la siguiente etapa

La narrativa pública recomendada del proyecto debe ser:

```text
mine-rs ya es un SDK alpha usable para block models, validación,
interoperabilidad abierta, analytics y workflows técnicos desde Python.

El frente de planning avanzado y paridad bibliográfica sigue activo,
pero no define por sí solo el estado del SDK base.
```

## Relación con otros documentos

- `README.md`: entrada principal del proyecto y resumen del estado actual.
- `docs/references/python-sdk-design.md`: criterios de diseño de la experiencia Python.
- `docs/references/architecture.md`: responsabilidades y boundaries por capa.
- `docs/backlog.md`: ejecución operativa de la consolidación alpha.
- `docs/analysis-estado-y-proximos-pasos.md`: análisis estratégico que motivó este documento.
