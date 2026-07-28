# Guardrail público de performance

Estado de este documento: definición cualitativa activa; baseline cuantitativa pendiente (MR-229).

## Propósito

Este documento define el workflow público que deberá medir la **baseline cuantitativa de performance** de `mine-rs` y cómo interpretar sus futuras mediciones sin sobreprometer.

No busca:

- declarar benchmarks universales;
- reclamar comparabilidad industrial entre máquinas;
- rediseñar la infraestructura de benchmarks;
- mezclar claims de producto con investigación benchmark-side.

Su función actual es más acotada: fijar un **guardrail cualitativo** para los workflows públicos `alpha`. El repo todavía no versiona mediciones, entorno de referencia ni umbrales cuantitativos de carga, validación y analytics; por eso MR-229 permanece pendiente.

## Qué guardrail sí existe hoy

Hoy el guardrail público de producto debe leerse como:

1. el workflow Python recomendado `load -> validate -> analyze -> export`;
2. el workflow complementario de `miners.tools` sobre modelos pequeños y deterministas;
3. la expectativa de que ambos sigan siendo usables como superficie **estable** del SDK `alpha`.

La evidencia funcional ejecutable para ese guardrail vive en:

- `examples/python/pandas_load_validate_analyze_export.py`
- `examples/python/numpy_load_validate_export.py`
- `examples/python/tools_workflow.py`

Estos ejemplos verifican comportamiento, no performance. No registran tiempos, memoria, máquina de referencia ni umbrales de regresión; por tanto no constituyen la baseline cuantitativa solicitada por MR-229.

## Workflow a medir

El workflow público que debe usarse como guardrail principal es:

```text
load -> validate -> analyze -> export
```

Lectura correcta por etapa:

1. **load**: carga desde `pandas` o `numpy` a través de `miners`;
2. **validate**: chequeos estructurados con `ValidationReport`;
3. **analyze**: `summary()`, `basic_statistics()`, `grouped_statistics()` y `grade_tonnage()`;
4. **export**: salida nuevamente a `pandas` o `numpy`.

Mientras este flujo siga siendo el camino recomendado del SDK `alpha`, su performance debe tratarse como señal de producto: si una contribución lo vuelve claramente más lento, más frágil o menos predecible en sus ejemplos públicos, debe leerse como regresión aunque el core siga compilando.

## Rol de `miners.tools` en el guardrail

`miners.tools` forma parte de la superficie **estable** recomendada para automatización en `alpha`, no de una capa separada de benchmark.

Por eso el futuro baseline público también deberá incluir el caso mínimo donde un modelo pequeño pase por tools deterministas como:

- `inspect_model`
- `validate_model`
- `query_blocks`
- `aggregate_blocks`

La lectura correcta es:

- el workflow humano principal sigue siendo `miners` y `load -> validate -> analyze -> export`;
- `miners.tools` cubre la variante product-facing para automatización y contratos estructurados;
- ambos deben seguir llamando la misma lógica determinista del SDK, no rutas especiales benchmark-side.

## Qué queda fuera del futuro baseline público

Este documento **no** redefine ni reemplaza:

- los microbenchmarks en `benchmarks/`;
- los harnesses y adaptadores benchmark-side;
- la telemetría y campañas de runtime en `examples/marvin-benchmark`.

Separación correcta:

- `docs/references/public-performance-baseline.md`: especificación del guardrail de producto y de la baseline cuantitativa pendiente;
- `benchmarks/`: investigación micro, costo relativo por operación y evolución técnica del core;
- `examples/marvin-benchmark`: comparabilidad, diagnóstico benchmark-side y telemetría de runtime dependiente de máquina.

En particular, `examples/marvin-benchmark` ya documenta explícitamente que sus tiempos de pared son **machine-dependent** y sirven para tracking del repo, no para comparaciones absolutas cross-machine. Esa lectura pertenece al frente benchmark-side, no al claim principal del SDK público.

## Cómo interpretar futuras mediciones

La interpretación correcta es intencionalmente conservadora:

- **sí**: señal de regresión o guardrail para workflows públicos `alpha`;
- **sí**: confirmación de que la superficie estable sigue siendo razonablemente usable;
- **no**: ranking absoluto de performance entre laptops, runners o estaciones de trabajo;
- **no**: evidencia suficiente para reclamar capacidad industrial general;
- **no**: sustituto de los benchmarks especializados del repo.

En otras palabras:

```text
la futura baseline deberá decir "el flujo público mínimo sigue dentro del guardrail";
no dice "mine-rs ya ganó una competencia universal de performance".
```

## Criterio mínimo para cerrar MR-229

La baseline cuantitativa mínima deberá medir de forma reproducible:

1. cargar un `BlockModel` pequeño desde `pandas` o `numpy`;
2. validarlo sin desviar la ruta normal del producto;
3. correr analytics públicos básicos;
4. exportarlo;
5. ejecutar tools deterministas equivalentes sobre un modelo igual de pequeño;
6. registrar fixture, comando, entorno, tiempos y/o memoria, y umbrales de interpretación.

Hasta que esos datos estén versionados, solo puede hablarse de salud funcional o de observaciones locales, no de una baseline pública cuantitativa cerrada. Una vez exista, una regresión deberá tratarse primero como regresión del producto `alpha`, antes de discutir comparabilidad bibliográfica.

## Relación con otros documentos

- `docs/references/sdk-alpha-scope.md`: define qué superficies son parte del SDK `alpha`.
- `docs/references/maturity-matrix.md`: clasifica `miners` y `miners.tools` como **estable** y el frente Marvin/MineLib como **benchmark-side**.
- `docs/references/python-sdk-design.md`: fija el workflow público recomendado y los ejemplos Python ejecutables.
- `docs/references/benchmark-diagnosis.md`: lectura correcta del frente benchmark-side y de comparabilidad bibliográfica.
