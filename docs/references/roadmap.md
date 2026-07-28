# Roadmap

Este roadmap resume la dirección estratégica que ahora se detalla operativamente en `docs/backlog.md`.

## Fase 1: Fundación del repositorio

Objetivo: convertir el repositorio inicial en un workspace Rust preparado para crecer.

Entregables esperados:

- Workspace Rust en monorepo.
- Estructura inicial de crates o módulos con boundaries claros.
- Crate fachada `mine-sdk`.
- Crate `mine-python` para bindings.
- Crate `mine-tools` para tools deterministas.
- Paquetes Python separados para `miners` y, más adelante, `mine-agents`.
- Dependencias base.
- Linting.
- Tests.
- CI.
- Benchmarks.

Resultado esperado:

```text
El proyecto compila, tiene estructura base y permite desarrollar módulos mineros con calidad mínima.
```

## Fase 2: Core de modelos de bloques

Objetivo: construir las estructuras centrales del dominio.

Entregables esperados:

- `Coordinate3D`.
- `BlockDimensions`.
- `GridDefinition`.
- `BlockModel`.
- Metadata.
- Storage columnar inicial.

Resultado esperado:

```text
mine-rs puede representar un modelo de bloques de forma estructurada y serializable.
```

## Fase 3: Indexing engine

Objetivo: soportar conversiones espaciales deterministas.

Entregables esperados:

- `xyz → ijk`.
- `ijk → xyz`.
- Indexación lineal.
- Validación de límites.
- Soporte para grillas rotadas en una etapa posterior.

Resultado esperado:

```text
El SDK puede ubicar bloques y convertir entre coordenadas espaciales e índices de forma reproducible.
```

## Fase 4: IO e interoperabilidad básica

Objetivo: permitir que usuarios carguen y exporten modelos.

Entregables esperados:

- Lectura CSV.
- Escritura CSV.
- Lectura Parquet.
- Escritura Parquet.
- Integración Arrow.
- Preservación de schema y metadata.

Resultado esperado:

```text
El SDK puede entrar y salir de workflows reales usando formatos abiertos.
```

## Fase 5: Validación

Objetivo: detectar problemas estructurales antes de hacer cálculos.

Entregables esperados:

- Detección de duplicados.
- Validación de grilla regular.
- Detección de gaps.
- Validación de extents.
- `ValidationReport` estructurado.

Resultado esperado:

```text
Un usuario puede cargar un modelo y obtener un diagnóstico accionable de calidad estructural.
```

## Fase 6: Python SDK inicial

Objetivo: exponer el core a usuarios Python.

Entregables esperados:

- Bindings con PyO3.
- Import del paquete Python.
- Exposición de `BlockModel`.
- Interoperabilidad con pandas.
- Interoperabilidad inicial con numpy.
- Ejemplos básicos en notebooks o scripts.

Resultado esperado:

```text
Un ingeniero puede cargar, validar y consultar modelos desde Python.
```

## Fase 7: Analytics y curvas ley-tonelaje

Objetivo: entregar valor minero directo sobre modelos cargados.

Entregables esperados:

- Tonelaje total.
- Ley media.
- Metal contenido.
- Curvas ley-tonelaje.
- Reportes por dominio, banco o fase.

Resultado esperado:

```text
El SDK permite generar análisis básicos útiles para revisión de modelos y comunicación técnica.
```

## Fase 8: Reblocking

Objetivo: transformar modelos preservando trazabilidad y reconciliación.

Entregables esperados:

- Reglas de agregación.
- Superblocking.
- Subblocking.
- Agregaciones ponderadas.
- Reporte de reconciliación.

Resultado esperado:

```text
El SDK puede cambiar resolución de modelos con reglas explícitas y resultados auditables.
```

## Fase 9: Planeamiento primitives

Objetivo: construir bloques de planeamiento reutilizables.

Entregables esperados:

- Bench generation.
- Phase tagging.
- Precedence graph.
- Reglas de avance vertical.
- Schedule primitives.

Resultado esperado:

```text
El SDK permite crear escenarios básicos de planeamiento con restricciones explícitas.
```

## Fase 10: Economía y escenarios

Objetivo: conectar modelos y secuencias con evaluación económica.

Entregables esperados:

- Cálculo de metal.
- Revenue.
- Costos.
- Cashflow.
- NPV.
- Comparación de escenarios.

Resultado esperado:

```text
Un usuario puede comparar escenarios con métricas técnicas y económicas reproducibles.
```

## Fase 11: Tools deterministas

Objetivo: crear una interfaz estable para automatización y agentes.

Entregables esperados:

- `inspect_model`.
- `validate_model`.
- `query_blocks`.
- `aggregate_blocks`.
- `grade_tonnage`.
- `create_scenario`.
- `evaluate_scenario`.
- `compare_scenarios`.

Resultado esperado:

```text
Las capacidades del SDK pueden invocarse mediante contratos estructurados.
```

## Fase 12: Capa agentica

Estado: pospuesta y no implementada.

Objetivo futuro: construir agentes que orquesten tools del SDK únicamente después de estabilizar tools deterministas, SDK Python, contratos de artefactos/VFS y disciplina de releases.

Entregables esperados:

- Task tool.
- VFS.
- Tool contracts.
- Inspector agent.
- Validation agent.
- Economics agent.
- Planning agent.
- Verifier agent.
- Scenario comparison agent.

Resultado esperado:

```text
Un usuario puede pedir análisis complejos en lenguaje natural y recibir resultados basados en tools deterministas.
```

## Dependencia estratégica

La capa agentica debe esperar no solo a que existan suficientes tools deterministas, sino a que estén estabilizados el SDK Python, los contratos de artefactos/VFS y la disciplina de releases. Construir agentes antes de cerrar esas dependencias aumenta el riesgo de resultados no verificables.

## Ruta end-to-end basada en literatura

Además del roadmap histórico por fases, el proyecto ya cuenta con una ruta específica para cerrar el motor minero end-to-end en `docs/references/mining-engine-roadmap.md`.

Esa ruta propone:

1. preparar y estimar el resource model con primitives deterministas;
2. construir un `EconomicBlockModel` multi-destino;
3. resolver pit final y shells anidados con métodos exactos;
4. diseñar pushbacks/fases y scheduling de largo plazo;
5. dejar incertidumbre y optimización estocástica como ola posterior.

## Relación con el backlog

`docs/backlog.md` contiene tickets, prioridades y criterios de aceptación detallados. Este roadmap debe usarse para comunicar dirección y fases; el backlog debe usarse para ejecución diaria.
