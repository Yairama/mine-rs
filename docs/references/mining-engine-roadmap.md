# Roadmap de motor minero end-to-end

Este documento sintetiza una ruta técnica para que `mine-rs` cubra un motor minero open-pit / block-model-first desde un modelo validado hasta evaluación económica, pit final, pushbacks y planeamiento a largo plazo.

No describe estado implementado. Es una guía de diseño y priorización basada en literatura para construir backlog y documentación técnica coherentes.

## Objetivo

Construir un SDK determinista, auditable y serializable que permita:

1. preparar y estimar modelos de bloques útiles para evaluación;
2. valorizar bloques con supuestos económicos explícitos;
3. generar pit final y shells anidados;
4. diseñar pushbacks y fases;
5. construir schedules de largo plazo con restricciones explícitas;
6. evaluar escenarios y, después, incorporar incertidumbre.

## Alcance recomendado

Este roadmap asume una estrategia **open-pit first**:

- foco en modelos de bloques, economía, precedencias, pit shells, pushbacks y scheduling;
- prioridad en algoritmos deterministas, auditables y benchmarkeables;
- dejar para más adelante la geología implícita, los flujos GUI-first y los métodos estocásticos pesados.

## Pipeline objetivo

```text
ValidatedBlockModel
↓
EconomicBlockModel
↓
PrecedenceGraph / SlopeTemplate
↓
UltimatePit / PitShellSet
↓
PushbackPlan / PhaseDesign
↓
LongTermSchedule
↓
DestinationPlan / StockpilePlan
↓
ScenarioEvaluationReport / ScenarioComparisonReport
```

## Contratos recomendados entre etapas

| Etapa | Artefacto recomendado | Contenido mínimo |
| --- | --- | --- |
| Validación | `ValidatedBlockModel` | grilla, columnas, unidades, metadata, QA/QC y referencias a origen |
| Economía | `EconomicBlockModel` | supuestos, destinos, recoveries, revenue, costos y valor por bloque |
| Taludes | `SlopeTemplate` / `PrecedenceGraph` | reglas geotécnicas, offsets, arcos o representación comprimida |
| Pit final | `UltimatePitResult` / `PitShellSet` | membresía de bloques, métricas, metadata del solver |
| Pushbacks | `PushbackPlan` / `PhaseDesign` | shell fuente, bloques, bancos, reglas de nesting y acceso |
| Scheduling | `LongTermSchedule` | periodos, capacidades, violaciones, asignaciones y destino opcional |
| Destinos | `DestinationPlan` / `StockpilePlan` | flujos por periodo, balances y políticas de reclaim |
| Escenarios | `ScenarioEvaluationReport` / `ScenarioComparisonReport` | cashflow, NPV, KPIs, diferencias y supuestos |

## Secuencia recomendada de implementación

### Ola 1 — Determinismo fundacional

1. compositing;
2. domaining duro y auditoría;
3. declustering y estadísticas ponderadas;
4. variografía experimental y ajuste de modelos;
5. estimadores deterministas base;
6. regularización de soporte;
7. validación del modelo estimado.

Resultado esperado:

```text
mine-rs puede producir un EconomicBlockModel confiable desde un block model preparado y auditable.
```

### Ola 2 — Valor económico y pit final

1. contratos de supuestos económicos por destino;
2. fórmulas NSR / equivalent value;
3. valorización multi-destino por bloque;
4. precedencias con taludes variables;
5. transformación max-closure;
6. solver exacto de pit final;
7. shells anidados y reportes de métricas.

Resultado esperado:

```text
mine-rs puede pasar de un EconomicBlockModel a un PitShellSet benchmarkeable.
```

### Ola 3 — Pushbacks y planeamiento a largo plazo

1. contratos explícitos de pushbacks y fases;
2. diseño de fases a partir de shells;
3. scheduler agregado determinista;
4. capacidades mina/planta;
5. destinos y stockpiles;
6. evaluación económica por periodo;
7. comparación de escenarios.

Resultado esperado:

```text
mine-rs puede construir un LongTermSchedule reproducible y evaluarlo económicamente.
```

### Ola 4 — Incertidumbre y métodos avanzados

1. contratos para realizaciones condicionales;
2. SGS / SIS experimentales;
3. métricas de riesgo;
4. pit estocástico;
5. scheduling estocástico y comparación robusta.

Resultado esperado:

```text
mine-rs puede comparar alternativas con sensibilidad y riesgo explícitos, sin romper el core determinista.
```

## Algoritmos recomendados por etapa

| Etapa | MVP recomendado | Posterior | Referencias principales |
| --- | --- | --- | --- |
| Preparación de resource model | compositing fijo, domaining duro, declustering por celdas | domaining probabilístico, workflows geológicos más complejos | [R01], [R02], [R03], [R04], [R05] |
| Continuidad espacial | variografía experimental, anisotropía, ajuste de modelos autorizados | co-variografía, multivariable | [R04], [R06], [R07], [R08] |
| Estimación | nearest neighbour, IDW, ordinary kriging, optional simple kriging | indicator kriging, co-kriging, KED | [R04], [R06], [R07], [R08] |
| Soporte de bloque | block kriging y regularización lineal | recoverable resources, MIK, UC/LUC | [R06], [R07], [R08] |
| Clasificación | motor de métricas y reglas configurables | clasificación probabilística | [R02], [R09], [R10] |
| Valorización | destination-aware value, NSR, equivalent value | geometalurgia avanzada, destinos no lineales | [R11], [R12], [R13], [R14] |
| Pit final | LG / max-closure / max-flow exacto | pseudoflow paramétrico y variantes | [R15], [R16], [R17], [R18], [R19] |
| Pushbacks | shells por revenue factor y diseño explícito | parametric pit limits más avanzados | [R17], [R18], [R20] |
| Scheduling | scheduler agregado, MILP/heurística determinista, capacidades y stockpiles | descomposición avanzada, Lagrangiano, metaheurísticas | [R20], [R21], [R22], [R23], [R24], [R25] |
| Incertidumbre | contratos de realizaciones y métricas de riesgo | SGS/SIS, pit y schedule estocásticos | [R07], [R24], [R25], [R26], [R27], [R28] |

## Qué debe quedar fuera del MVP inicial

- modelado geológico implícito y wireframing;
- automatización de reportes de compliance;
- reserve conversion completo;
- simulación metalúrgica avanzada;
- scheduling de corto plazo / dispatch;
- GUI-first design tools.

## Benchmarks y verificación

### Públicos y recomendados

- **MineLib** como benchmark principal para precedencias, pit final y scheduling abierto. [R29]
- **Marvin** como suite de paridad local para `blocks`, `prec`, `upit` y reports cuando los artefactos externos sean verificables.
- instancias abiertas tipo **Newman** para smoke tests de pit final y normalización de artefactos.

### Estrategia de verificación

1. fixtures pequeños sintéticos por etapa;
2. benchmarks abiertos para pit/schedule;
3. comparación serializable de shells, memberships, tonelaje, metal y NPV;
4. metadata explícita con solver, seed, tolerancias y hashes de inputs.

## Referencias

- **[R01]** Journel, A. G., Huijbregts, C. J. (1978). *Mining Geostatistics*. Academic Press.
- **[R02]** Abzalov, M. (2016). *Applied Mining Geology*. Springer. https://doi.org/10.1007/978-3-319-39264-6
- **[R03]** Rossi, M. E., Deutsch, C. V. (2014). *Mineral Resource Estimation*. Springer. https://doi.org/10.1007/978-1-4020-5717-5
- **[R04]** Matheron, G. (1963). *Principles of Geostatistics*. Economic Geology, 58(8), 1246-1266. https://doi.org/10.2113/gsecongeo.58.8.1246
- **[R05]** Journel, A. G. (1983). *Nonparametric estimation of spatial distributions*. Mathematical Geology, 15, 445-468. https://doi.org/10.1007/BF01031292
- **[R06]** Wackernagel, H. (1995). *Multivariate Geostatistics*. Springer. https://doi.org/10.1007/978-3-662-03098-1
- **[R07]** Goovaerts, P. (1997). *Geostatistics for Natural Resources Evaluation*. Oxford University Press.
- **[R08]** Deutsch, C. V. (2015). *Cell Declustering Parameter Selection*. Geostatistics Lessons. https://geostatisticslessons.com/lessons/celldeclustering
- **[R09]** JORC Code (2012). *Australasian Code for Reporting of Exploration Results, Mineral Resources and Ore Reserves*. https://jorc.org/docs/JORC_code_2012.pdf
- **[R10]** CRIRSCO (2024). *International Reporting Template*. https://crirsco.com/wp-content/uploads/woocommerce_uploads/2024/06/CRIRSCO_International_Reporting_Template_June2024_Update_Approved_for_Release_20240627-dl8515.pdf
- **[R11]** Lane, K. F. (1988). *The Economic Definition of Ore: Cut-off Grades in Theory and Practice*. https://espace.library.uq.edu.au/view/UQ:246974
- **[R12]** Goldie, R., Tredger, P. (1991). *Net Smelter Return Models and Their Use in the Exploration, Evaluation and Exploitation of Polymetallic Deposits*. https://openalex.org/W1540459899
- **[R13]** Asad, M. W. A. (2005). *Cutoff grade optimization algorithm with stockpiling option for open pit mining operations of two economic minerals*. https://doi.org/10.1080/13895260500258661
- **[R14]** Goodfellow, R., Dimitrakopoulos, R. (2017). *Simultaneous Stochastic Optimization of Mining Complexes and Mineral Value Chains*. https://doi.org/10.1007/s11004-017-9680-3
- **[R15]** Lerchs, H., Grossmann, I. F. (1965). *Optimum Design of Open-Pit Mines*. https://openalex.org/W3217061993
- **[R16]** Picard, J.-C. (1976). *Maximal Closure of a Graph and Applications to Combinatorial Problems*. https://doi.org/10.1287/mnsc.22.11.1268
- **[R17]** Underwood, R. G., Tolwinski, B. (1998). *A mathematical programming viewpoint for solving the ultimate pit problem*. https://doi.org/10.1016/S0377-2217(97)00141-0
- **[R18]** Hochbaum, D. S., Chen, A. (2000). *Performance Analysis and Best Implementations of Old and New Algorithms for the Open-Pit Mining Problem*. https://doi.org/10.1287/opre.48.6.894.12392
- **[R19]** Khalokakaie, R., Dowd, P. A., Fowell, R. J. (2000). *Lerchs–Grossmann algorithm with variable slope angles*. https://doi.org/10.1179/mnt.2000.109.2.77
- **[R20]** Ramazan, S., Dagdelen, K. (1998). *A new push back design algorithm in open pit mining*. https://doi.org/10.1201/9781003761860-20
- **[R21]** Tolwinski, B. (1996). *A scheduling algorithm for open pit mines*. https://doi.org/10.1093/imaman/7.3.247
- **[R22]** Caccetta, L., Hill, S. P. (2003). *An Application of Branch and Cut to Open Pit Mine Scheduling*. https://doi.org/10.1007/A:1024835022186
- **[R23]** Lambert, W. B., Brickey, A., Newman, A. M., Eurek, K. (2014). *Open-Pit Block-Sequencing Formulations: A Tutorial*. https://doi.org/10.1287/inte.2013.0731
- **[R24]** Moreno, E., Rezakhah, M., Newman, A. M., Ferreira, F. C. L. (2017). *Linear models for stockpiling in open-pit mine production scheduling problems*. https://doi.org/10.1016/j.ejor.2016.12.014
- **[R25]** Rezakhah, M., Newman, A. M. (2020). *Open pit mine planning with degradation due to stockpiling*. https://doi.org/10.1016/j.cor.2018.11.009
- **[R26]** Ramazan, S., Dimitrakopoulos, R. (2013). *Production scheduling with uncertain supply: a new solution to the open pit mining problem*. https://doi.org/10.1007/s11081-012-9186-2
- **[R27]** Morales, N., Seguel, J., Cáceres, A., Moreno, E., Pincheira, J.-A. (2019). *Incorporation of Geometallurgical Attributes and Geological Uncertainty into Long-Term Open-Pit Mine Planning*. https://doi.org/10.3390/min9020108
- **[R28]** Jelvez, E., Morales, N., Ortiz, J. M. (2021). *Stochastic Final Pit Limits: An Efficient Frontier Analysis under Geological Uncertainty in the Open-Pit Mining Industry*. https://doi.org/10.3390/math10010100
- **[R29]** Espinoza, D., Goycoolea, M., Moreno, E., Newman, A. M. (2013). *MineLib: a library of open pit mining problems*. https://doi.org/10.1007/s10479-012-1258-3
