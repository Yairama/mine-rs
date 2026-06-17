# Diseño del SDK Python

La capa Python es la superficie principal para usuarios de `mine-rs`. Aunque el core se implemente en Rust, la experiencia de trabajo debe sentirse natural para ingenieros de minas que usan notebooks, pandas, numpy y scripts de automatización.

## Objetivo

Crear una API Python que permita usar las capacidades del SDK sin conocer Rust.

La API debe servir para:

- Exploración interactiva.
- Pipelines reproducibles.
- Automatización.
- Integración con agentes.
- Construcción de reportes.
- Prototipado de escenarios.

## Ubicación en el repositorio

La capa Python debe separarse en dos piezas:

- `crates/mine-python`: crate PyO3/Maturin que compila el módulo nativo.
- `python/miners`: paquete Python público para usuarios finales.

`mine-python` debe depender de `mine-sdk` y, cuando aplique, de `mine-tools`. `python/miners` debe concentrarse en ergonomía Python: type hints, helpers, integración con pandas/numpy y documentación de ejemplos.

La capa Python no debe depender de la capa agentica. Los agentes pueden depender de `miners`, pero `miners` no debe depender de agentes.

## Contrato local soportado para instalación y validación

Mientras el SDK siga en etapa `alpha`, el camino soportado para contributors debe ser único y reproducible:

1. crear un `venv` limpio con Python `>=3.11`;
2. instalar o actualizar `pip` y `maturin` dentro de ese mismo entorno;
3. ejecutar `python -m maturin develop` desde la raíz del repo;
4. validar con `python -m unittest discover -s tests -p "test_python_*.py"`.

Este contrato local debe comunicar con claridad qué queda validado:

- el módulo nativo de `mine-python` compila e instala correctamente;
- el paquete público `miners` resuelve sus dependencias mínimas declaradas;
- la superficie de import recomendada en la raíz `miners` sigue usable para el workflow público `load -> validate -> analyze -> export`, sin obligar al contributor a conocer rutas internas del repo.

Si una contribución rompe este flujo, debe tratarse como regresión del SDK Python alpha aunque el core Rust siga compilando.

Por ahora, este contrato **no** equivale a una política completa de wheels, publishing o releases, y tampoco adelanta la futura exposición de `miners.tools`.

> La guía de UX y workflow de este documento se mantiene separada de la política de versionado y releases `0.x`; para esas garantías exactas, ver [`alpha-release-policy.md`](alpha-release-policy.md).

## Workflow público recomendado hoy

Mientras `mine-rs` siga consolidando su API `alpha`, la documentación pública debe promover un único camino ejecutable y reconocible:

1. `load_from_pandas(...)` o `load_from_numpy(...)`
2. `validate()`
3. `summary()` / `basic_statistics()` / `grouped_statistics()` / `grade_tonnage()`
4. `export_to_pandas(...)` o `export_to_numpy(...)`

Este flujo prioriza discoverability y ergonomía notebook-first sin esconder supuestos mineros críticos. La carga y exportación viven como helpers públicos en `miners`; el análisis y la validación viven como métodos explícitos de `BlockModel`.

Ejemplo actual con pandas:

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

Ejemplo actual con numpy:

```python
from miners import export_to_numpy, load_from_numpy

model = load_from_numpy(
    grid=grid,
    schema=schema,
    float_columns={"cu": cu, "tonnes": tonnes},
    integer_columns={"bench": bench},
)

report = model.validate()
curve = model.grade_tonnage("cu", "tonnes", [0.3, 0.5, 0.7])
arrays = export_to_numpy(model, columns=["cu", "tonnes", "bench"])
```

La API fluent avanzada debe quedar en `miners.experimental`. Puede seguir existiendo para exploración opt-in, pero no debe presentarse como flujo recomendado ni mezclarse con la narrativa principal del SDK alpha.

`README.md` debe tratar `examples/python/` como la entrada práctica para este workflow. Hoy ese pack ejecutable ya debe cubrir, como mínimo, los scripts `pandas_load_validate_analyze_export.py`, `numpy_load_validate_export.py` y `tools_workflow.py`, todos apoyados en la superficie pública actual (`miners` y `miners.tools`). Este documento conserva snippets cortos para explicar el diseño y deja explícitamente aparte cualquier API futura o experimental.

## Principios de API

### Clara antes que clever

Los nombres deben mapear directamente a conceptos mineros:

- `BlockModel`.
- `GridDefinition`.
- `ValidationReport`.
- `ReblockRules`.
- `GradeTonnageCurve`.
- `MiningScenario`.

### Explícita en supuestos

Las operaciones que dependan de parámetros deben pedirlos de forma clara. Por ejemplo, una curva ley-tonelaje debe indicar qué columna es ley, qué columna es tonelaje y qué cutoffs se usan.

### Compatible con notebooks

Los objetos deben tener representaciones útiles:

- Resumen legible.
- Conversión a dataframe.
- Exportación simple.
- Métodos de inspección rápida.

### Sin magia silenciosa

El SDK no debe inferir decisiones mineras críticas sin avisar. Si una columna de tonelaje, densidad o ley no está clara, la API debe pedirla explícitamente o emitir un error comprensible.

## APIs futuras o conceptuales

Todo ejemplo que no corresponda al flujo público actual o que no esté respaldado por `examples/python/` debe marcarse como conceptual.

```python
# Ejemplo conceptual: no representa el camino público recomendado hoy.
from miners import BlockModel, GridDefinition

grid = GridDefinition(
    origin=(0.0, 0.0, 0.0),
    block_size=(10.0, 10.0, 10.0),
    shape=(100, 80, 40),
)

model = BlockModel.read_csv(
    "blocks.csv",
    grid=grid,
    x="x",
    y="y",
    z="z",
)

report = model.validate()
report.raise_on_errors()
```

## Interoperabilidad con pandas

pandas debe ser una vía natural de entrada y salida:

```python
# Ejemplo actual de la superficie recomendada.
from miners import export_to_pandas, load_from_pandas

model = load_from_pandas(
    dataframe=df,
    grid=grid,
    schema=schema,
)

df_exportado = export_to_pandas(model, columns=["cu", "tonnes"])
```

Consideraciones:

- Evitar copias innecesarias cuando sea posible.
- Preservar tipos.
- Mantener metadata fuera del dataframe cuando corresponda.
- Reportar columnas incompatibles.

## Interoperabilidad con numpy

numpy es importante para operaciones numéricas y workflows científicos.

Camino recomendado hoy:

```python
from miners import export_to_numpy, load_from_numpy

model = load_from_numpy(
    grid=grid,
    schema=schema,
    float_columns={"cu": cu, "tonnes": tonnes},
)

arrays = export_to_numpy(model, columns=["cu", "tonnes"])
```

Usos esperados:

- Extraer columnas como arrays.
- Inyectar atributos calculados.
- Interoperar con algoritmos externos.
- Trabajar con máscaras booleanas.

La API debe evitar exponer memoria de forma insegura. Si existe zero-copy parcial, debe documentarse claramente.

## Interoperabilidad con Arrow y Parquet

Arrow y Parquet son candidatos naturales para el almacenamiento columnar.

Objetivos:

- Lectura eficiente.
- Escritura eficiente.
- Compatibilidad con Python.
- Compatibilidad con data engineering.
- Preservación de schema.

## Manejo de errores en Python

Hoy la superficie pública de Python expone una sola excepción: `miners.MineError`.

Ese contrato único debe mantenerse explícito en docs, ejemplos y type hints mientras no exista una jerarquía pública más fina. No debe documentarse una familia de excepciones separadas que hoy no existe.

Internamente, `MineError` preserva las categorías base del modelo de errores Rust. Para usuarios Python, eso significa que un mismo tipo público puede representar errores de:

- `Io`
- `Schema`
- `Grid`
- `Validation`
- `Reblock`
- `Economics`
- `Planning`
- `InvalidParameter`
- `Numeric`

La categoría concreta puede reflejarse en el mensaje y en el origen Rust del fallo, pero el tipo Python público sigue siendo uno solo.

Las validaciones ordinarias del modelo siguen otro camino: `model.validate()` y helpers equivalentes devuelven `ValidationReport`. Los hallazgos normales de schema, grilla, cobertura o consistencia no deben describirse como la ruta principal de excepción; primero se inspeccionan en el reporte. Solo cuando una operación no puede ejecutarse correctamente por un error de contrato, input o estado, corresponde levantar `miners.MineError`.

Los mensajes deben explicar:

- Qué falló.
- Dónde falló.
- Qué input fue problemático.
- Qué puede hacer el usuario.

## Reportes

Los reportes deben ser objetos, no strings sueltos.

```python
# Ejemplo conceptual.
report = model.validate()

report.has_errors
report.summary()
report.to_json()
report.to_pandas()
```

Esto facilita notebooks, agentes y pipelines.

## Tools desde Python

Las tools deterministas pueden exponerse también desde Python:

```python
# Ejemplo conceptual.
from miners.tools import inspect_model, grade_tonnage

inspection = inspect_model({"path": "model.parquet"})
curve = grade_tonnage({
    "path": "model.parquet",
    "grade": "cu",
    "tonnage": "tonnes",
})
```

Este estilo ayuda a conectar el SDK con agentes que trabajan con JSON schemas.

## Diseño para agentes sin dañar la API humana

La API Python debe servir a humanos primero, pero también permitir contratos estructurados para agentes.

Una posible separación:

- API humana: objetos, métodos y notebooks.
- API tool: funciones con inputs/outputs serializables.

Ambas deben llamar al mismo core Rust para evitar divergencias.

## Documentación de ejemplos

Cuando el SDK exista, la documentación Python debería incluir:

- Cargar CSV.
- Cargar Parquet.
- Validar modelo.
- Convertir coordenadas.
- Calcular curva ley-tonelaje.
- Rebloquear.
- Exportar a VTK.
- Crear escenario simple.
- Invocar tools.

## Criterios de aceptación de la capa Python

- Una persona puede instalar el paquete y cargar un modelo simple.
- El flujo soportado para contributors (`venv -> maturin develop -> unittest`) está documentado y es reproducible.
- El workflow público recomendado (`load -> validate -> analyze -> export`) está documentado y distingue con claridad la superficie experimental en `miners.experimental`.
- `README.md` y `examples/python/` funcionan juntos como entrypoint ejecutable para usuarios Python, sin mezclar ejemplos conceptuales con la superficie recomendada.
- Los errores son comprensibles.
- Los outputs pueden convertirse a pandas o JSON.
- Las operaciones críticas delegan a Rust.
- Los ejemplos de notebooks son reproducibles.
- La API no oculta supuestos mineros importantes.
