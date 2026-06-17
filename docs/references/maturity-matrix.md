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
| Rust core recomendado | **estable** | `mine-sdk` como entrada pública y sus módulos de dominio recomendados (`core`, `blockmodel`, `io`, `validation`, `economics`, `planning`, `reblock`) | Es la ruta Rust recomendada para consumo técnico del SDK. |
| Python recomendado | **estable** | `miners` como paquete público raíz y el workflow `load -> validate -> analyze -> export` | Es la ruta Python recomendada para notebooks, scripts y adopción general. |
| Tools recomendadas | **estable** | `miners.tools` y las tools deterministas públicas ya expuestas | Forma parte de la superficie de automatización recomendada del SDK actual. |
| Experimental / opt-in | **experimental** | `mine_sdk::experimental`, `miners.experimental` y bridges o wrappers deprecados mantenidos solo por compatibilidad transitoria | Puede usarse de forma explícita, pero no debe presentarse como camino principal ni como contrato estabilizado. |
| Benchmark-side / investigación | **benchmark-side** | `examples/marvin-*`, `examples/stochastic-planning`, harnesses de benchmark y adaptadores de comparabilidad/investigación | Sirve para diagnóstico, reproducción bibliográfica y evolución técnica, no como superficie principal del SDK. |

## Regla práctica de comunicación

Si una superficie cae en **estable**, puede usarse en documentación principal y ejemplos recomendados.

Si cae en **experimental**, debe etiquetarse como opt-in de forma explícita.

Si cae en **benchmark-side**, debe explicarse como soporte de investigación o comparabilidad, no como ruta base de producto.

La baseline pública de performance del SDK `alpha` debe leerse como guardrail de producto para la superficie **estable**; no reclasifica como estables los harnesses ni los artefactos benchmark-side.

## Relación con otros documentos

- `docs/references/sdk-alpha-scope.md`: narrativa de alcance, claims y límites del SDK `alpha`.
- `docs/references/python-sdk-design.md`: criterios del camino Python recomendado.
- `docs/references/benchmark-diagnosis.md`: estado y lectura correcta del frente benchmark-side.
- `docs/backlog.md`: ejecución operativa y tickets, no clasificación canónica de superficies.
