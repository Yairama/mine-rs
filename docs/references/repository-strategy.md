# Estrategia de repositorio, crates y paquetes

Este documento define cómo separar el SDK Rust, la capa Python y la capa agentica de `mine-rs`.

## Decisión

`mine-rs` debe comenzar como un monorepo con separación interna fuerte por crates Rust y paquetes Python.

No se recomienda separar en repositorios distintos en la etapa inicial. La razón principal es que el proyecto todavía está definiendo contratos de dominio, APIs, schemas y flujos entre Rust, Python y agentes. Separar repos demasiado pronto aumentaría coordinación, versionado cruzado y fricción de desarrollo sin aportar suficiente beneficio.

La separación correcta por ahora es:

- Crates Rust para el core, dominio, SDK y tools deterministas.
- Un crate Rust específico para bindings Python.
- Un paquete Python para la experiencia de usuario.
- Un paquete agentico separado, probablemente Python-first, dentro del mismo repositorio.
- Contratos de tools compartidos y versionados dentro del monorepo.

## Regla principal

```text
Separar capas por paquete/crate dentro del monorepo.
Separar repositorios solo cuando haya estabilidad, releases independientes o necesidades operativas reales.
```

## Estructura recomendada

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

Esta estructura es objetivo. No necesita crearse toda desde el primer commit. Debe crecer por necesidad.

## Qué va en cada capa

### `mine-core`

Crate base con tipos comunes, errores, traits, metadata y utilidades compartidas.

Debe evitar dependencias pesadas y no debe depender de Python, agentes, CLI ni IO específico.

### Crates de dominio

Crates como `mine-blockmodel`, `mine-indexing`, `mine-validation`, `mine-reblock`, `mine-io`, `mine-economics` y `mine-planning` contienen capacidades concretas.

Estos crates pueden depender de `mine-core` y entre ellos cuando sea necesario, pero deben mantener boundaries claros.

### `mine-sdk`

Crate fachada del SDK Rust.

Este crate debe ser la entrada pública recomendada para usuarios Rust y para capas superiores. Puede reexportar tipos y funciones de crates internos.

Función principal:

- Ocultar complejidad modular.
- Estabilizar la API pública.
- Evitar que Python y agentes dependan de detalles internos.

### `mine-tools`

Crate de tools deterministas y contratos serializables.

Debe exponer operaciones orientadas a automatización:

- `inspect_model`.
- `validate_model`.
- `query_blocks`.
- `aggregate_blocks`.
- `grade_tonnage`.
- `create_scenario`.
- `evaluate_scenario`.
- `compare_scenarios`.

Este crate debe depender de `mine-sdk`, no al revés.

### `mine-python`

Crate PyO3/Maturin que compila el módulo nativo para Python.

Debe ser un puente, no un lugar para lógica minera crítica. Su responsabilidad es convertir tipos, mapear errores y exponer APIs Python.

Debe depender de:

- `mine-sdk` para API humana.
- `mine-tools` para API tool/JSON cuando aplique.

### `python/miners`

Paquete Python público para usuarios finales.

Puede envolver el módulo nativo generado por `mine-python` y agregar ergonomía Python:

- Helpers.
- Type hints.
- Integración con pandas/numpy.
- Objetos de alto nivel.
- Documentación de ejemplos.

Nombre recomendado del paquete importable:

```python
import miners
```

### `python/mine-agents`

Paquete agentico separado objetivo; no está implementado.

Cuando se implemente, deberá vivir separado del SDK Python humano para evitar mezclar dependencias de LLM, LangGraph/LangChain, VFS y orchestration con la librería base. Ese trabajo está pospuesto hasta estabilizar tools, Python, artefactos/VFS y disciplina de releases.

La capa agentica debería depender de:

- `miners`.
- Tool schemas.
- VFS.
- Runtime agentico.

No debería ser una dependencia del SDK.

## ¿La capa agentica debe ser un crate Rust?

No como primera decisión.

La metodología basada en `deep-agents-from-scratch` vive naturalmente en el ecosistema Python/LangGraph. Por eso, el orquestador agentico debería comenzar como paquete Python separado dentro del monorepo.

Sí puede haber crates Rust para piezas deterministas usadas por agentes:

- `mine-tools` para tools y schemas.
- `mine-agent-vfs` si se requiere un VFS determinista en Rust.
- `mine-agent-verifier` si se requiere verificación local robusta.

Pero el "agent runtime" principal no debería estar en Rust solo por simetría arquitectónica. Debe estar donde sea más natural para el stack agentico.

## Dirección de dependencias

La dirección debe ser estricta:

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

- El core nunca depende de Python.
- El core nunca depende de agentes.
- `mine-sdk` no depende de `mine-python`.
- `mine-sdk` no depende de `python/mine-agents`.
- `mine-tools` puede depender del SDK, pero el SDK no debe depender de tools agenticas.
- La capa agentica consume tools; no define la lógica minera.

## Versionado recomendado

Mientras el proyecto esté temprano:

- Versionado unificado del monorepo.
- CI conjunto.
- Cambios coordinados entre Rust y Python.
- Releases manuales o pre-releases.

Cuando las APIs maduren:

- `mine-sdk` puede versionarse como crate Rust estable.
- `miners` puede publicarse en PyPI.
- `mine-agents` puede publicarse como paquete separado o mantenerse experimental.

## Cuándo separar repositorios

Separar repositorios solo cuando se cumpla alguna condición fuerte:

- El SDK Rust tiene API estable y ciclo de release independiente.
- La capa agentica requiere despliegue, seguridad o infraestructura propia.
- El paquete Python tiene comunidad, issues y releases independientes.
- El monorepo vuelve lenta la CI o el desarrollo.
- Existen equipos separados manteniendo capas diferentes.

Antes de eso, separar repos agregaría overhead innecesario.

## Decisión práctica para el MVP

Para el MVP, la estructura mínima recomendada es:

```text
crates/
├─ mine-core/
├─ mine-blockmodel/
├─ mine-sdk/
├─ mine-tools/
└─ mine-python/

python/
├─ miners/
└─ mine-agents/
```

Si se quiere simplificar aún más al inicio:

```text
crates/
├─ mine-core/
├─ mine-sdk/
├─ mine-tools/
└─ mine-python/
```

Luego se extraen crates especializados cuando el dominio lo justifique.

## Resumen

La recomendación es mantener un solo repositorio y separar internamente:

- `mine-sdk` como fachada Rust pública.
- `mine-python` como binding nativo.
- `python/miners` como paquete Python de usuario.
- `mine-tools` como capa de tools deterministas.
- `python/mine-agents` como capa agentica Python-first.

Esta estructura mantiene velocidad de desarrollo, boundaries claros y capacidad de separar repositorios más adelante sin romper el diseño.
