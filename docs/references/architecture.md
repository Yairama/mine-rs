# Arquitectura

`mine-rs` debe evolucionar hacia una arquitectura por capas donde el core determinista vive en Rust, la experiencia de usuario principal vive en Python y la capa agentica se construye encima como un orquestador de tools.

## Arquitectura conceptual

```text
┌──────────────────────────────────────────────┐
│ UI futura / notebooks / CLI / automatización │
└──────────────────────┬───────────────────────┘
                       │
┌──────────────────────▼───────────────────────┐
│              Agentic Layer futura             │
│  task tools · VFS · subagents · verifier      │
└──────────────────────┬───────────────────────┘
                       │
┌──────────────────────▼───────────────────────┐
│               Python SDK Layer                │
│  API ergonomica · pandas · numpy · notebooks  │
└──────────────────────┬───────────────────────┘
                       │
┌──────────────────────▼───────────────────────┐
│                 Rust Core                     │
│  block models · IO · validation · planning    │
└──────────────────────────────────────────────┘
```

## Separación de responsabilidades

### Rust Core

Responsable de:

- Tipos de dominio.
- Estructuras de datos.
- Cómputo intensivo.
- Validación.
- Indexación.
- Reblocking.
- IO.
- Primitives de planeamiento.
- Serialización.
- Contratos deterministas.

El core debe ser testeable sin Python y no debe depender de agentes.

### Python SDK

Responsable de:

- Exponer APIs cómodas.
- Convertir datos entre Python y Rust.
- Integrarse con pandas, numpy y Arrow.
- Facilitar notebooks.
- Manejar errores de forma clara.
- Proveer documentación y ejemplos de usuario.

La capa Python debe minimizar lógica crítica propia. Cuando una operación minera sea central, debe delegarse al core Rust.

### Agentic Layer

Responsable de:

- Interpretar objetivos del usuario.
- Planificar tareas.
- Invocar tools deterministas.
- Coordinar subagents.
- Mantener artefactos en un VFS.
- Validar outputs.
- Explicar resultados.

La capa agentica debe depender de contracts claros. No debe acceder a estructuras internas si una tool ya existe.

## Modularización futura

La decisión recomendada es mantener un monorepo con separación interna fuerte por crates y paquetes. No conviene separar repositorios en la etapa inicial porque las APIs, contratos y schemas todavía van a cambiar juntos.

La separación objetivo es:

```text
mine-rs/
├─ crates/
│  ├─ mine-core/
│  ├─ mine-blockmodel/
│  ├─ mine-indexing/
│  ├─ mine-validation/
│  ├─ mine-reblock/
│  ├─ mine-io/
│  ├─ mine-economics/
│  ├─ mine-planning/
│  ├─ mine-sdk/
│  ├─ mine-tools/
│  ├─ mine-python/
│  └─ mine-cli/
│
├─ python/
│  ├─ miners/
│  └─ mine-agents/
│
├─ docs/
├─ examples/
├─ tests/
└─ benchmarks/
```

Esta estructura debe adoptarse gradualmente. En una etapa temprana, puede ser válido comenzar con menos crates y extraer módulos conforme crezca el dominio.

### Crate SDK

`mine-sdk` debe ser la fachada Rust pública. Su función es reexportar capacidades de los crates internos y ofrecer una API estable para usuarios Rust, bindings Python, CLI y tools.

Las capas superiores no deberían depender directamente de todos los crates internos si pueden depender de `mine-sdk`.

### Crate Python

`mine-python` debe ser el crate PyO3/Maturin que compila el módulo nativo usado desde Python. No debe contener lógica minera crítica; debe mapear tipos, errores y funciones entre Rust y Python.

### Paquete Python

El paquete público `python/miners` debe envolver el módulo nativo y ofrecer ergonomía Python, type hints, helpers e integración con pandas/numpy.

### Tools deterministas

`mine-tools` debe contener operaciones serializables y orientadas a automatización. Debe depender del SDK, no al revés.

### Capa agentica

La capa agentica debe vivir separada del SDK base. Por la inspiración en `deep-agents-from-scratch` y el ecosistema LangGraph/LangChain, el runtime agentico debería comenzar como un paquete Python separado, por ejemplo `python/mine-agents`.

Pueden existir crates Rust agenticos solo para piezas deterministas, como VFS, schemas o verificación, pero el orquestador principal no necesita ser un crate Rust al inicio.

Más detalle: [`repository-strategy.md`](repository-strategy.md).

## Dirección de dependencias

La dirección de dependencias debe ser estricta:

```text
mine-core
↓
crates de dominio
↓
mine-sdk
↓
mine-tools
↓
mine-python / mine-cli / python/mine-agents
```

Reglas:

- El core no depende de Python.
- El core no depende de agentes.
- `mine-sdk` no depende de `mine-python`.
- `mine-sdk` no depende de la capa agentica.
- La capa agentica consume tools; no define cálculos mineros.
- Los bindings Python exponen el SDK; no duplican lógica crítica.

## Modelo de datos

### BlockModel

`BlockModel` debe ser la entidad principal. Conceptualmente combina:

- Definición espacial.
- Atributos por bloque.
- Metadata.
- Storage columnar.
- Validaciones asociadas.

### GridDefinition

Define:

- Origen.
- Dimensiones de bloque.
- Número de bloques por eje.
- Rotación.
- Sistema de coordenadas si aplica.

### Columnas y atributos

Los atributos deben tratarse de forma columnar para permitir:

- Lecturas eficientes.
- Integración con Arrow.
- Exportación a Parquet.
- Operaciones vectorizadas.
- Interoperabilidad con Python.

## Contratos de datos

Los outputs importantes deben poder serializarse:

- Reportes de validación.
- Resultados de agregación.
- Curvas ley-tonelaje.
- Escenarios.
- Evaluaciones económicas.
- Comparaciones.

Cada contrato debería tener:

- Schema.
- Versionado.
- Campos requeridos.
- Errores esperados.
- Metadata de ejecución.

## Manejo de errores

Los errores deben ser explícitos. El SDK debe distinguir:

- Errores de IO.
- Errores de schema.
- Errores de validación.
- Errores de geometría.
- Errores de parámetros.
- Errores numéricos.
- Errores internos.

En Python, estos errores deben convertirse en excepciones específicas y comprensibles.

## Determinismo y reproducibilidad

Para operaciones críticas, el SDK debe documentar:

- Inputs.
- Parámetros.
- Tolerancias.
- Versiones de algoritmo.
- Orden de agregación cuando afecte resultados.
- Warnings.
- Resumen de reconciliación.

Esto permite que un resultado sea auditado, repetido y comparado.

## Performance

El proyecto debe priorizar performance donde impacta directamente:

- Lectura y escritura de modelos grandes.
- Indexación.
- Validación masiva.
- Agregación.
- Reblocking.
- Cálculos por escenario.

Rust, Arrow, Parquet y paralelismo con `rayon` son piezas naturales para esta dirección.

## Interoperabilidad

La arquitectura debe evitar encierro en formatos propios. Los formatos abiertos deben ser de primera clase:

- CSV para compatibilidad.
- Parquet para datasets grandes.
- Arrow para intercambio.
- JSON para reports y tools.
- VTK/VTU para visualización.

## Relación con `docs/backlog.md`

El backlog final contiene tickets concretos por área. Este documento describe la arquitectura objetivo. Cuando ambos entren en conflicto, el backlog debe ajustarse o dividirse, pero la regla de diseño debe mantenerse: core determinista, Python ergonómico y agentes como orquestadores.
