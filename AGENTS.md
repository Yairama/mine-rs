# AGENTS.md

Guía para agentes que trabajen en `mine-rs`.

## Objetivo del proyecto

`mine-rs` busca convertirse en un SDK de utilidades para ingeniería de minas, con:

- Core determinista en Rust.
- API Python como superficie principal para usuarios técnicos.
- Tools serializables para automatización.
- Capa agentica futura basada en VFS, task tools y subagents.

El proyecto no debe tratarse como una GUI minera monolítica. La prioridad es construir infraestructura computacional minera: tipos de dominio, motores reproducibles, APIs estables, interoperabilidad y herramientas que puedan componerse.

## Estado actual

El repositorio está en etapa temprana. La documentación define la dirección objetivo, pero muchas APIs aún no existen.

Antes de implementar, revisa:

- `README.md`
- `docs/backlog.md`
- `docs/references/vision.md`
- `docs/references/product-scope.md`
- `docs/references/domain-capabilities.md`
- `docs/references/architecture.md`
- `docs/references/repository-strategy.md`
- `docs/references/python-sdk-design.md`
- `docs/references/agentic-layer.md`
- `docs/references/roadmap.md`
- `docs/references/temporal-backlog.md`

## Decisión arquitectónica vigente

Usar monorepo con separación interna fuerte por crates y paquetes. No separar repositorios todavía.

Estructura objetivo:

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

## Límites entre capas

### Rust core

Responsable de lógica minera determinista:

- Modelos de bloques.
- Indexing `xyz ↔ ijk`.
- Validación.
- Reblocking.
- IO.
- Economía.
- Planeamiento.
- Contratos serializables.

No debe depender de Python, agentes, prompts ni runtimes LLM.

### `mine-sdk`

Fachada pública Rust. Debe reexportar capacidades de crates internos y ser la entrada estable para capas superiores.

### `mine-tools`

Tools deterministas orientadas a automatización y agentes. Debe depender de `mine-sdk`, no al revés.

### `mine-python` y `python/miners`

`mine-python` es el binding nativo PyO3/Maturin. `python/miners` es el paquete Python público.

La lógica minera crítica debe vivir en Rust. Python debe aportar ergonomía, type hints, integración con pandas/numpy y experiencia de usuario.

### `python/mine-agents`

Capa agentica Python-first. Debe vivir separada del SDK base y depender de `miners` y de los contratos de `mine-tools`.

Los agentes orquestan; el SDK calcula.

## Dirección de dependencias

Mantén esta dirección:

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
- La capa agentica no define cálculos mineros.
- Los cálculos deben ser ejecutados por funciones deterministas.

## Regla agentica central

```text
Los agentes razonan y orquestan.
Las tools deterministas calculan y validan.
```

Un agente puede planificar, explicar, delegar y verificar, pero no debe inventar resultados técnicos que deberían salir de una tool del SDK.

## Skills necesarias para trabajar en este repo

Cuando un agente trabaje en este proyecto, debe operar como si tuviera estas skills activas:

### 1. Arquitectura Rust SDK

Capacidad para diseñar crates, boundaries, errores, traits, módulos y APIs públicas con foco en estabilidad y performance.

### 2. Ingeniería de minas computacional

Conocimiento funcional de block models, coordenadas, bancos, fases, pushbacks, secuencias, tonelaje, ley, metal, cutoffs, validación, reblocking y escenarios.

### 3. Python bindings y packaging

Capacidad para diseñar bindings con PyO3/Maturin, paquetes Python, type hints, interoperabilidad con pandas/numpy/Arrow y experiencia notebook-first.

### 4. Data engineering columnar

Capacidad para razonar sobre Arrow, Parquet, schemas, metadata, datasets grandes, serialización y compatibilidad entre Rust y Python.

### 5. Deterministic tools

Capacidad para diseñar tools con input/output schema, JSON estructurado, errores explícitos, artefactos reproducibles y contratos versionables.

### 6. Agentic systems

Capacidad para diseñar VFS, task tools, subagents, verifier agents, tool calling y separación entre reasoning y cómputo determinista.

### 7. Documentación técnica y producto

Capacidad para mantener alineados README, docs de arquitectura, roadmap, backlog operativo y documentación de usuario.

## Skills instaladas

Este repositorio usa el formato abierto de Agent Skills. Las skills están instaladas en `.github/skills/` para GitHub Copilot, usando el CLI `@tech-leads-club/agent-skills` cuando aplica.

Mantén este set compacto. No agregues skills por curiosidad: cada skill debe cubrir una capacidad recurrente del proyecto.

### Skills propias de mine-rs

- `mine-rs-architecture`: usar para decisiones de monorepo, crates, paquetes, boundaries y dirección de dependencias.
- `mining-domain-modeling`: usar para modelos de bloques, grillas, validación, reblocking, ley-tonelaje, economía, pushbacks y secuencias.
- `rust-python-sdk`: usar para diseño/implementación de Rust SDK, PyO3/Maturin, paquete Python, pandas/numpy/Arrow y errores.
- `deterministic-agent-tools`: usar para tools deterministas, JSON schemas, VFS, task tools, subagents y verifiers.
- `mine-rs-documentation`: usar para README, AGENTS.md, docs, roadmap, backlog y consistencia documental.

### Skills externas instaladas

- `tlc-spec-driven`: planificación seria de proyecto/features con Specify, Design, Tasks y Execute. Úsala para iniciativas grandes, specs, roadmap de implementación o trabajo multi-fase.
- `modular-design-principles`: diseño modular, boundaries, contratos, state ownership, fail independence y revisión de acoplamiento. Úsala para decisiones de arquitectura escalable.
- `coding-guidelines`: disciplina de implementación: simplicidad, cambios quirúrgicos, éxito verificable y evitar sobreingeniería. Úsala al escribir, modificar o revisar código.
- `tactical-ddd`: modelado táctico de dominio con entidades, value objects, aggregates, invariants y servicios de dominio. Úsala cuando el SDK empiece a implementar tipos mineros ricos.
- `security-threat-model`: threat modeling basado en el repositorio. Úsala para diseñar la seguridad de surfaces como Python bindings, tools, VFS, agentes, CLI o carga de archivos.
- `security-best-practices`: revisión y guía secure-by-default, especialmente para Python y capas agenticas. Úsala en tareas de seguridad o cuando se agreguen superficies expuestas.

### Skills revisadas y no instaladas

- `docs-writer`: no se instaló porque `mine-rs-documentation` ya cubre la documentación con contexto específico del proyecto.
- `codenavi`: no se instaló porque introduce una base `.notebook/` persistente y solapa con `tlc-spec-driven` y las reglas actuales del repo.
- Skills web, frontend, Nx, cloud, Figma, Playwright y CI GitHub no se instalaron porque no están alineadas con el foco actual del SDK minero Rust/Python.

Si una tarea coincide con alguna de estas descripciones, activa la skill correspondiente antes de trabajar.

## Cómo actuar ante tareas nuevas

1. Identifica si la tarea es de documentación, arquitectura, Rust, Python, tools o agentes.
2. Lee los documentos relevantes antes de modificar.
3. Mantén la decisión de monorepo salvo que el usuario pida explícitamente reevaluarla.
4. No crees crates o paquetes nuevos sin justificar el boundary.
5. No mezcles dependencias agenticas dentro del SDK base.
6. No prometas funcionalidades como existentes si aún son objetivo de diseño.
7. Prefiere cambios pequeños, consistentes y bien documentados.
8. Actualiza documentación cuando una decisión arquitectónica cambie.

## Convenciones de documentación

- Escribir documentación principal en español.
- Marcar ejemplos no implementados como conceptuales.
- Diferenciar claramente entre estado actual, objetivo y roadmap.
- Referenciar `docs/backlog.md` como backlog operativo.
- Mantener `docs/references/repository-strategy.md` como fuente principal para decisiones de repositorio/crates/paquetes.

## Convenciones de implementación futuras

Cuando el proyecto pase a implementación:

- Añadir tests junto con lógica de dominio.
- Hacer que validadores devuelvan reportes estructurados.
- Preferir errores explícitos y tipados.
- Evitar defaults silenciosos en supuestos mineros.
- Mantener outputs serializables para tools y agentes.
- Validar que Python y Rust llamen la misma lógica central.
- Medir performance en operaciones sobre modelos grandes.
- Cuando una implementación siga un paper, preprint arXiv o fuente técnica externa, dejar la referencia bibliográfica y DOI/URL en comentarios o docstrings del código correspondiente, distinguiendo claramente si se trata de literatura académica o práctica actual.

## Criterio de calidad

Una contribución es buena si:

- Respeta boundaries.
- Mantiene determinismo.
- Es usable desde Python o prepara esa ruta.
- Produce outputs auditables.
- No acopla agentes al core.
- Mejora la claridad del SDK para ingenieros de minas.
