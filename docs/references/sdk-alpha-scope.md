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
- primitives de planning ya formalizadas como contratos serializables, cuando se usen con claims de madurez acordes a su estado actual.

### 2. Python recomendado

La entrada pública recomendada en Python es `miners`.

Debe considerarse parte del SDK alpha, salvo indicación explícita en contrario, la superficie enfocada en:

- construcción y lectura de `BlockModel`;
- tipos core expuestos a Python;
- `summary()`;
- `validate()` y `ValidationReport`;
- interoperabilidad base con pandas y numpy ya soportada por el repo;
- analytics públicos ya expuestos:
  - estadísticas básicas
  - grouped statistics
  - grade-tonnage

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
- rutas avanzadas de pushbacks, cuts y scheduling cuya madurez todavía dependa de cierres adicionales de comparabilidad;
- prototipos estocásticos;
- optimizaciones de performance cuya semántica pública aún no esté consolidada.

Regla práctica:

```text
Si una capacidad existe pero todavía no debería tomarse como contrato de producto
para usuario general, debe llamarse experimental de forma explícita.
```

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
- capa agentica madura;
- comparabilidad paper-grade cerrada para todo scheduling avanzado;
- cobertura completa de todas las instancias y formulaciones MineLib como claim ya resuelto.

## Lectura recomendada por tipo de usuario

### Usuario Python técnico

Camino recomendado hoy:

1. `miners`
2. examples y guías públicas
3. validación, analytics, IO y reblocking base

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
