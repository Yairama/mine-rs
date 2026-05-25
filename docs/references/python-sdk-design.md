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

## Estilo conceptual

```python
# Ejemplo conceptual: API no implementada todavía.
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
# Ejemplo conceptual.
df = model.to_pandas(columns=["x", "y", "z", "cu", "tonnes"])

model = BlockModel.from_pandas(
    df,
    grid=grid,
    coordinate_columns=("x", "y", "z"),
)
```

Consideraciones:

- Evitar copias innecesarias cuando sea posible.
- Preservar tipos.
- Mantener metadata fuera del dataframe cuando corresponda.
- Reportar columnas incompatibles.

## Interoperabilidad con numpy

numpy es importante para operaciones numéricas y workflows científicos.

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

Los errores Rust deben mapearse a excepciones Python específicas.

Ejemplos conceptuales:

- `MineError`.
- `SchemaError`.
- `ValidationError`.
- `GridError`.
- `IoError`.
- `ReblockError`.
- `PlanningError`.

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
- Los errores son comprensibles.
- Los outputs pueden convertirse a pandas o JSON.
- Las operaciones críticas delegan a Rust.
- Los ejemplos de notebooks son reproducibles.
- La API no oculta supuestos mineros importantes.
