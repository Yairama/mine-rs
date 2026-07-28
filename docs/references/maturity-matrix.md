# Matriz de madurez de superficies

Estado de este documento: referencia canónica activa para la etapa `0.x`.

## Propósito

Esta matriz resume cómo debe clasificarse hoy la superficie pública y cercana a lo público de `mine-rs`.

Su objetivo es dar una lectura rápida y consistente de madurez sin reescribir:

- las reglas completas del SDK `alpha`;
- los detalles operativos del backlog;
- los criterios de comparabilidad benchmark-side.

Para el detalle narrativo, ver `docs/references/sdk-alpha-scope.md`. Para el frente benchmark-side, ver `docs/references/benchmark-diagnosis.md` y la documentación relacionada de paridad/literatura. Para los guardrails públicos de performance del SDK `alpha`, ver `docs/references/public-performance-baseline.md`: ese documento acompaña la lectura de superficies estables y no sustituye el material benchmark-side.

## Buckets oficiales

- **estable**: superficie recomendada para documentación principal, ejemplos de adopción y uso técnico normal del SDK en la etapa actual.
- **experimental**: superficie opt-in o en transición, disponible para exploración pero no presentada como camino por defecto.
- **benchmark-side**: superficie orientada a investigación, comparabilidad, harnesses o adaptadores; no define el camino principal de adopción del SDK.

## Matriz

| Grupo de superficie | Bucket | Incluye hoy | Lectura correcta |
| --- | --- | --- | --- |
| Rust core recomendado | **estable** | `mine-sdk` como entrada pública para core, `BlockModel`, indexing, IO, validación, analytics, economía base y reblocking | Es la ruta Rust recomendada para consumo técnico del SDK alpha. La estabilidad es de la superficie listada, no de cada símbolo reexportado. |
| Planning básico y contratos | **estable** | bancos y phase tagging básicos, `MiningScenario`, `PrecedenceGraph`, `Schedule` y reportes/contratos serializables asociados | Pueden usarse como primitives auditables con restricciones explícitas; esta clasificación no promueve todo `mine_sdk::planning`. |
| Python recomendado | **estable** | `miners`: pandas/numpy, `read_csv`/`write_csv`, `read_parquet`/`write_parquet`, indexing de `GridDefinition`, validación/analytics, `AggregationRule`/`DistributionRule` y `superblock`/`subblock` | Es la ruta Python recomendada para notebooks, scripts y adopción general en alpha. |
| Tools recomendadas | **estable** | `miners.tools` y las tools deterministas públicas ya expuestas | Forma parte de la superficie de automatización recomendada del SDK actual. |
| Planning avanzado | **experimental** | TopoSort CPIT/PCPSP, bound Lagrangiano LP/BZ, pseudoflow paramétrico/shells optimizados, pushbacks, cuts y scheduling avanzado | Aunque parte del código esté reexportada por `mine-sdk`, sigue sujeta a validación algorítmica, performance y comparabilidad; no es contrato estable de producto. |
| Experimental / opt-in | **experimental** | `mine_sdk::experimental`, `miners.experimental`, layouts sparse en evolución y wrappers deprecados mantenidos solo por compatibilidad transitoria | Puede usarse de forma explícita, pero no debe presentarse como camino principal ni como contrato estabilizado. |
| Benchmark-side / investigación | **benchmark-side** | harnesses y adaptadores Marvin/MineLib/McLaughlin, LP/BZ sidecars, cuts y scheduling paper-comparable, `examples/stochastic-planning` y reportes generados de comparabilidad | Sirve para diagnóstico, reproducción bibliográfica y evolución técnica, no como superficie principal del SDK. |
| Capa agentica | **no implementada / pospuesta** | diseño de VFS, task tools, subagents y verifier | No existe runtime ni paquete agentico actual. Se pospone hasta estabilizar tools, Python, artefactos/VFS y disciplina de releases. |

## Regla práctica de comunicación

Si una superficie cae en **estable**, puede usarse en documentación principal y ejemplos recomendados.

Si cae en **experimental**, debe etiquetarse como opt-in de forma explícita.

Si cae en **benchmark-side**, debe explicarse como soporte de investigación o comparabilidad, no como ruta base de producto.

El documento público de performance define hoy un guardrail cualitativo para la superficie **estable**. No existe todavía baseline cuantitativa versionada (MR-229) y ese documento no reclasifica como estables los harnesses ni los artefactos benchmark-side.

## Relación con otros documentos

- `docs/references/sdk-alpha-scope.md`: narrativa de alcance, claims y límites del SDK `alpha`.
- `docs/references/python-sdk-design.md`: criterios del camino Python recomendado.
- `docs/references/benchmark-diagnosis.md`: estado y lectura correcta del frente benchmark-side.
- `docs/backlog.md`: ejecución operativa y tickets, no clasificación canónica de superficies.
