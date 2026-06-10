# Diagnóstico de benchmarks MineLib y estado de comparabilidad

## Resumen ejecutivo

Hoy **sí existe un valor "de ~800 millones" para Marvin**, pero corresponde a **CPIT** (`820,726,048`) y no a la referencia **PCPSP** que el repo usa cuando compara scheduling con dos destinos. La referencia pública actual para Marvin PCPSP es `885,968,070`, mientras que la relajación LP-PCPSP pública llega a `911,704,665`. Eso significa que, para Marvin PCPSP, el objetivo comparable no es "llegar a 800M", sino **cerrar la brecha contra ~886M** (`datasets/benchmarks/marvin/marving-info.txt:31-45`).

La mejor corrida principal versionada hoy por `mine-rs` en Marvin queda en `664,161,466.99`, con clasificación explícita `exploratory-local`, y por tanto está aproximadamente **25.0% por debajo** de la referencia PCPSP pública (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:249-253`, `datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1383-1438`). El problema ya no es solo "mejorar un heurístico": el repo todavía mezcla **gap algorítmico real** con **gap de comparabilidad bibliográfica/protocolar**.

## Qué valor debemos perseguir

### Marvin: referencias públicas correctas

| Formulación | Tipo | Valor oficial | Fuente |
| --- | --- | ---: | --- |
| UPIT | pit final | 1,415,655,436 | `datasets/benchmarks/marvin/marving-info.txt:27-30` |
| CPIT | schedule factible 1 destino | 820,726,048 | `datasets/benchmarks/marvin/marving-info.txt:31-34` |
| LP-CPIT | relajación, no factible | 863,916,131 | `datasets/benchmarks/marvin/marving-info.txt:35-38` |
| PCPSP | schedule factible 2 destinos | 885,968,070 | `datasets/benchmarks/marvin/marving-info.txt:39-42` |
| LP-PCPSP | relajación, no factible | 911,704,665 | `datasets/benchmarks/marvin/marving-info.txt:43-46` |

Conclusión: el recuerdo de "~800M" probablemente viene de **CPIT** o de resultados históricos previos a las mejores soluciones hoy versionadas, pero **no** es el target correcto si se quiere comparar el pipeline actual de Marvin PCPSP.

### McLaughlin: la variante también importa

| Instancia | PCPSP oficial | Observación |
| --- | ---: | --- |
| `mclaughlin-limit` | 1,321,662,551 | Es la variante alineable con la literatura MineLib más común (`datasets/benchmarks/mclaughlin-limit/mclaughlin-limit-info.txt:37-44`). |
| `mclaughlin-full` | 1,510,126,435 | No es la misma variante que `mclaughlin-limit`; no debe compararse como si fuera la misma tabla de paper (`datasets/benchmarks/mclaughlin/mclaughlin-info.txt:35-42`). |

## Estado actual del repo

### Marvin

| Artefacto | Valor descontado | Lectura correcta |
| --- | ---: | --- |
| Referencia PCPSP MineLib | 885,968,061.49 | Mejor schedule factible público cargado en el repo (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:530-530`). |
| Baseline `cpit-period-routed` | 820,726,047.95 | Replica el orden temporal CPIT con ruteo posterior; explica por qué "~800M" sí aparece en Marvin, pero no es la referencia PCPSP (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:620-633`). |
| Candidato principal `ready_frontier` | 664,161,466.99 | Mejor candidato principal hoy versionado; sigue `exploratory-local` (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1383-1403`). |
| Candidato LP/BZ round-repair | 661,177,100.28 | La ruta LP/BZ todavía no supera al candidato principal (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1143-1157`). |
| Bound LP/BZ en sidecar | 899,374,039.13 | Señal de que el gap entero es grande, pero el bound sigue viniendo de una ruta benchmark-side parcial (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1013-1023`). |

### McLaughlin

| Dataset | Referencia PCPSP | Candidato repo | Brecha observable |
| --- | ---: | ---: | --- |
| `mclaughlin-limit` | 1,321,662,545.35 | 503,540,970.31 | Muy lejos y todavía sin LP/BZ sidecar (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1716-1778`, `datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1908-1928`). |
| `mclaughlin-full` | 1,510,126,434.32 | 502,994,681.27 | Además de la brecha, la variante `full` no es comparable con la literatura `limit` (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:2149-2154`, `datasets/benchmarks/outputs/multi-mine-scheduling-report.json:2248-2305`, `datasets/benchmarks/outputs/multi-mine-scheduling-report.json:2436-2456`). |

## Diagnóstico profundo

### 1. El gap principal en Marvin no es "faltan 20 o 30 millones"; faltan ~222 millones contra PCPSP

El candidato principal queda en `664,161,466.99` frente a `885,968,061.49`, con una diferencia absoluta de `221,806,594.50` y relativa de `0.250355...` (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1433-1438`). Por tanto, el estado actual no es "casi comparable": todavía está **un cuarto abajo** del best-known público.

### 2. El pipeline todavía no es paper-comparable

El propio artefacto multi-mine marca a Marvin como `exploratory-local` y enumera gaps estructurales:

1. la procedencia de bloques ya se declara como contrato explícito `marvin-paperlike-v2-shells-pushbacks-mining-cuts`, y ahora además publica el puente cuantitativo `selected blocks -> shell×bench pushback phases -> localized-cut phases -> scheduling units`; aun así, esa cadena sigue siendo una reconstrucción benchmark-side y no todavía un generador bibliográfico reproducido. En otras palabras, el benchmark-side ya dejó el gap suficientemente estrecho y auditable como para decir que el próximo salto no es "otro tweak local", sino un cambio core-side / protocolario que convierta esa procedencia en contrato compartido de inputs para scheduling;
2. la familia principal todavía es `nested-shell-bench` derivada de factores revenue/cost-aware, no un pipeline bibliográfico reproducido de pushbacks/mining cuts;
3. la ruta LP/BZ activa usa unidades benchmark-side `pushback-bench-localized-cut-phase`;
4. el probe competitivo LP/BZ ya clasifica de forma empírica si el bloqueo dominante parece venir de `precedence-coverage`, `budget-depletion`, `round-repair-local-search-mismatch` o si ya no queda más que `schedule-level-proof-only`, publica además un `budget_coverage_experiment` auditable para distinguir si conviene priorizar expansión de cobertura, de presupuesto o si ninguno domina, pero mantiene `parity_claim_status = diagnostic-only` porque todavía no existe prueba reproducible de competitividad real;
5. ramp access, working width, lineage / bench continuity y complete cut design siguen modelados como **proxies benchmark-side parciales** (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:253-349`).

Mientras esa clasificación no cambie, cualquier comparación directa contra papers mezcla calidad del algoritmo con diferencias de protocolo.

### 3. El scheduler principal comprime demasiado el horizonte temporal

La referencia PCPSP usa 14 periodos activos; el candidato principal usa 10 (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:524-530`, `datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1392-1403`). Además:

- `earlier_than_reference_count = 6642`
- `mean_absolute_period_delta = 2.7165`
- `max_absolute_period_delta = 7`

(`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1478-1485`)

Eso indica que el solver todavía concentra demasiado material antes de tiempo. Con descuento del 10%, esa distorsión temporal pega fuerte en NPV y es consistente con una política que no logra reproducir la secuencia operativa real del benchmark.

### 4. El desacople destino-periodo sigue siendo muy bajo

La membresía `(periodo, destino)` compartida entre el candidato principal y la referencia tiene `jaccard_index = 0.07934`, es decir, apenas ~7.9% de coincidencia (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1539-1543`). El scheduler no solo está desfasado en tiempo: también está ruteando de forma muy distinta a la solución pública.

### 5. La señal LP/BZ es prometedora en bound, pero débil en candidato entero

El sidecar LP/BZ sí entrega una señal útil:

- bound `899,374,039.13`, ya cerca del LP-PCPSP público `911,704,665`;
- solve `optimal` en `minilp`;
- pero con `40820` filas de precedencia efectivamente forzadas sobre `408200` totales, vía estrategia `hybrid_checkpoint`

(`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1013-1023`)

Sin embargo, al bajar a candidato entero, la ruta queda en `661,177,100.28` y el optimizador local queda frenado por un presupuesto de **1 sola iteración** con `termination_reason = "max-iterations-reached"` (`datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1085-1091`, `datasets/benchmarks/outputs/multi-mine-scheduling-report.json:1121-1145`). Esto sugiere dos cosas:

1. el bound no se está convirtiendo en schedule factible con suficiente calidad;
2. la etapa de round/repair/local improvement es todavía demasiado conservadora para cerrar el gap.

### 6. La mejor familia shell-driven actual sigue siendo demasiado gruesa

En `comparison-report.json`, el mejor techo de la familia estricta actual aparece en `strict-shell-sequential` con `664,161,466.99`, y las variantes localizadas mejoran solo hasta ~`662,950,385.30` (`datasets/benchmarks/marvin/outputs/comparison-report.json:2326250-2326318`, `datasets/benchmarks/marvin/outputs/comparison-report.json:2326551-2326627`). El repo ya agotó bastante tuning local sobre:

- factor count,
- access policy,
- localized fronts,
- band width,
- predecessor-link policy,
- shape gates.

La lectura más importante es que **el cuello de botella ya no parece ser un tweak adicional pequeño**, sino la falta de una ley geométrica/operativa más fiel a la literatura para mining cuts y acceso.

### 7. McLaughlin está menos maduro que Marvin

`mclaughlin-limit` ya no está exactamente en el mismo punto que `mclaughlin-full`: el benchmark-side empezó a promover una ruta `nested-shell-bench` reconstruida desde `*.upit` + precedencias y ahora la declara como proxy explícito de shells -> fases shell×bench pushback-equivalent -> scheduling. Además, el reporte multi-mine ya puede versionar `primary_unit_family_traceability` para cuantificar `selected blocks -> shell×bench phases -> scheduling units`, y ahora también fija evidencia estructurada `benchmark_side_evidence` para dejar explícito cuándo la familia activa sigue en `no-benchmark-cut-refinement` / `no-lp-bz-sidecar` frente a `mclaughlin-full` como variante stress-only. El slice actual además estrecha ese hueco de cut-side con una traza `pushback-equivalent-bench-cut-readiness`, suma el scaffold contractual `mclaughlin-limit-cut-sidecar-scaffold`, publica prerequisitos estructurados para los contratos futuros de cut refinement / LP-BZ sidecar, añade `benchmark_cut_promotion_ready`, `lp_bz_sidecar_promotion_ready`, `*_blocking_prerequisite_ids`, reglas explícitas `benchmark_cut_promotion_rule` / `lp_bz_sidecar_promotion_rule` y listas `benchmark_cut_exit_criteria` / `lp_bz_sidecar_exit_criteria` para que los criterios benchmark-side de promoción queden más explícitos y testeables en el JSON. Además, ya existe un primer sidecar LP/BZ parcial sobre `mclaughlin-limit-only` que publica un bound relajado del kernel `shell × bench` con `solve_status`, `coverage_completeness`, `coverage_basis_points` y gaps auditables; ese artefacto sigue marcado `diagnostic-only`, pero el contrato benchmark-side ya distingue el escalón intermedio `partial-bound-available` antes de cualquier promoción más fuerte. Sobre esa base, `mclaughlin_limit_promotion_checklist` (`mr207-v4`) deja explícito —sin sobreprometer implementación— qué pasos del camino benchmark-side ya están auditados, cuáles siguen `scaffold-only`, qué regla bloquea cada promoción, qué exit criteria siguen pendientes y cómo el gate temporal/ruteo solo pasa a ser vinculante una vez exista una ruta de cut refinement y/o LP/BZ sidecar más representativo sobre `mclaughlin-limit`. Aun así, McLaughlin sigue por detrás de Marvin porque:

- `mclaughlin-limit` todavía no tiene mining cuts bibliográficos ni un sidecar LP/BZ comparable de mining cuts; lo que existe hoy es un primer bound relajado `shell × bench` útil como diagnóstico, no como evidencia de cierre;
- la nueva ruta comparable sigue siendo una **equivalencia benchmark-side**, no una reproducción paper-grade completa;
- `mclaughlin-full` debe seguir leyéndose solo como benchmark local de stress, con separación explícita de la variante `limit`.

Por eso el benchmark multi-mine todavía no sirve como prueba de comparabilidad fuerte: Marvin ya tiene un sidecar LP/BZ y McLaughlin todavía no.

### 8. Hay un problema operativo de reproducibilidad del harness

Durante esta revisión, la corrida `cargo run -p marvin-benchmark -- --mode focused-mr187 --quiet` falló por resolución de rutas a `marvin.blocks` desde el entorno actual, pese a que el README la documenta como comando directo del workspace. No cambia los artefactos ya versionados, pero sí muestra que la regeneración del benchmark no está totalmente endurecida como workflow reproducible de mantenimiento.

## Qué falta para llegar a la zona correcta

### Prioridad 1: cerrar comparabilidad antes de pedir otro gran salto de NPV

El siguiente avance importante no es "otro heuristic tweak", sino eliminar los mayores gaps protocolarios:

1. dejar de sembrar desde `cpit-solution` como origen de bloques;
2. reemplazar la familia proxy `nested-shell-bench` por una ruta bibliográfica reproducible de pushbacks / mining cuts;
3. convertir ramp access, working width, lineage / bench continuity y complete cut design de proxies benchmark-side a contratos mucho más fieles a la literatura.

Mientras eso no ocurra, incluso un salto de 20-40M seguiría siendo ambiguo: no sabríamos cuánto corresponde a solver y cuánto a protocolo.

### Prioridad 2: transformar el sidecar LP/BZ en un pipeline entero serio

La oportunidad más clara está en la ruta LP/BZ, porque el bound ya vive cerca del LP oficial. Lo que falta es:

1. endurecer el solve LP para que no dependa de un checkpoint parcial de precedencias;
2. mejorar round/repair sobre unidades paper-like;
3. permitir más presupuesto y mejores movimientos al optimizador local;
4. medir explícitamente cuánto del gap entero viene de round, de repair y de local search.

Si el bound ya está cerca de la referencia LP pero el entero cae a ~661M, el mayor retorno no está en generar otro bound, sino en **cerrar la relajación-entero**.

### Prioridad 3: hacer que el solver use un horizonte más parecido al público

Pasar de 10 a algo cercano a 14 periodos activos en Marvin debería ser una meta explícita. La señal temporal actual (`6642` bloques adelantados respecto a la referencia) muestra que el scheduler aún no reproduce bien la secuencia de extracción y destino. Aquí conviene atacar:

1. reglas de release entre cuts/fases;
2. working width / ramp proxies más realistas;
3. heurísticas o repairs que penalicen la sobrecompresión temporal.

### Prioridad 4: llevar el mismo hardening a McLaughlin Limit

El proyecto todavía depende demasiado de Marvin para sus conclusiones más fuertes. El siguiente salto de credibilidad del benchmark multi-mine requiere:

1. endurecer la nueva ruta shells -> pushback-equivalent units de `mclaughlin-limit` hacia mining cuts más fieles;
2. agregar sidecar LP/BZ en `mclaughlin-limit`;
3. dejar `mclaughlin-full` solo como stress benchmark, no como comparación literaria.

## Próximos pasos recomendados del proyecto

1. **Hardening del harness benchmark:** corregir resolución de paths y dejar reproducibles los comandos documentados del README para refresh focalizado y multi-mine.
2. **Ruta Marvin paper-like real:** reemplazar el seed `cpit-solution` por una cadena reproducible shells -> pushbacks -> mining cuts -> LP/BZ -> round/repair.
3. **LP/BZ entero de verdad:** quitar el cuello de botella del checkpoint parcial y del local optimizer a una sola iteración.
4. **Diagnóstico temporal explícito como criterio de avance:** no medir solo NPV; seguir también `used_period_count`, `mean_absolute_period_delta`, `earlier_than_reference_count` y similitud `(period, destination)`.
5. **Generalización a `mclaughlin-limit`:** portar la misma ruta comparable antes de extraer conclusiones "multi-mine".
6. **Promoción del benchmark solo cuando cambie la clasificación:** el hito real no es solo subir de 664M a otro número; es poder cambiar `comparison_classification` desde `exploratory-local` a una clase realmente comparable.

## Conclusión

El proyecto **no está hoy en la zona de las mejores corridas MineLib comparables** para Marvin PCPSP. El target correcto no es "~800M", sino **~886M**. El repo ya tiene una base mucho más seria que antes — sobre todo porque distingue comparabilidad, bound LP, candidato entero y gaps bibliográficos —, pero todavía le faltan dos cierres grandes:

1. **comparabilidad paper-grade del pipeline**, y  
2. **capacidad de convertir el bound LP/BZ cercano al oficial en un schedule entero mucho mejor**.

La lectura correcta del estado actual es: el proyecto ya salió del estadio "benchmark smoke", pero todavía no ha cerrado el salto desde benchmark-side exploratory evidence hacia una reproducción bibliográfica fuerte.
