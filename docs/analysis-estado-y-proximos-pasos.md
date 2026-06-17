# Análisis de estado y próximos pasos de `mine-rs`

Fecha: 2026-06-12

## Objetivo de este análisis

Responder cuatro preguntas estratégicas:

1. ¿En qué estado real está hoy el proyecto?
2. ¿Ya entró en una fase suficientemente estable como para sumar valor?
3. ¿Conviene tratarlo ya como un SDK y ordenar el roadmap alrededor de eso?
4. ¿Debe priorizarse ya la capa Python o todavía no?

## Fuentes revisadas

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
- `docs/references/mining-engine-roadmap.md`
- `docs/references/benchmark-diagnosis.md`
- `docs/references/literature-parity.md`

Además, se verificó el estado operativo mínimo del repo:

- `cargo build --workspace`: OK.
- `python -m unittest discover -s tests -p "test_python_*.py"`: OK.

## Resumen ejecutivo

La conclusión principal es esta:

```text
mine-rs ya dejó de ser solo una idea o una fundación vacía.
Ya es una base técnica útil.
Pero todavía no es un SDK público maduro 1.0.
```

La recomendación estratégica es:

1. Tratar `mine-rs` desde ahora como un SDK en fase `alpha`.
2. Priorizar la productización de la capa Python existente, no seguir postergándola.
3. Mantener la línea de benchmarks avanzados y scheduling como un track paralelo de I+D, no como bloqueo para el SDK base.
4. No priorizar todavía la capa agentica como frente principal.

La respuesta corta a las preguntas del proyecto es:

- ¿Ya suma valor? Sí, en un alcance claro y técnico.
- ¿Ya debe generarse como SDK? Sí, pero como `SDK alpha`, no como producto estable 1.0.
- ¿Ya debe hacerse la capa Python? Sí. De hecho, ya empezó; el próximo paso correcto no es “empezarla”, sino endurecerla, documentarla y convertirla en la superficie principal para usuarios.

## Lectura del estado real

## 1. El proyecto ya tiene columna vertebral real

Hoy el repositorio ya no está en una etapa fundacional vacía. Tiene:

- workspace Rust consolidado;
- crates de dominio separados y alineados con la arquitectura objetivo;
- fachada `mine-sdk`;
- tools deterministas en `mine-tools`;
- bindings `mine-python` funcionales;
- paquete Python `miners` importable;
- tests Python de humo sobre flujos reales;
- un backlog muy avanzado en dominios core;
- evidencia benchmark seria en UPIT, CPIT y PCPSP.

Eso cambia la naturaleza del proyecto. Ya no conviene gestionarlo como “proyecto exploratorio que algún día será SDK”. Ya conviene gestionarlo como:

```text
SDK técnico en alpha + línea paralela de investigación/benchmark avanzado.
```

## 2. La capa Python ya existe, pero todavía no está productizada

Este punto es crítico para la decisión.

La pregunta ya no es si conviene “empezar Python”. La realidad del repo es que Python ya está presente:

- `pyproject.toml` ya define el paquete `miners` como `0.1.0` y lo clasifica como `Alpha`.
- `python/miners/__init__.py` ya expone `BlockModel`, `GridDefinition`, `ValidationReport`, analytics y workflow experimental.
- los tests Python validan construcción de modelos, validación, pandas, numpy y analytics base.

Entonces la decisión correcta es:

```text
Sí a Python, pero como trabajo de endurecimiento, ergonomía, documentación y release discipline.
No como experimento lateral ni como capa “para después”.
```

## 3. La capa agentica no está lista para ser prioridad

La arquitectura y la documentación son consistentes en esto: los agentes deben vivir encima del SDK y de las tools, no dentro del core.

Además, el estado real del repo lo confirma:

- `python/mine-agents/` existe solo como placeholder;
- las tools deterministas iniciales sí existen;
- la superficie humana Python ya existe parcialmente;
- el valor inmediato para usuarios técnicos todavía está más en notebooks, pandas, validación, analytics y reblocking que en orquestación agentica.

Conclusión:

```text
Todavía no conviene poner la mayor energía en agentes.
Conviene primero consolidar el SDK y la UX Python.
```

## 4. El proyecto ya tiene dos tracks distintos y hay que reconocerlo explícitamente

Hoy `mine-rs` mezcla dos motores de valor:

### Track A: SDK utilitario para ingeniería de minas

Entrega valor directo a usuarios que necesitan:

- cargar modelos;
- validarlos;
- exportarlos;
- analizarlos;
- rebloquear;
- evaluarlos desde Python.

### Track B: I+D de planeamiento y benchmarking contra MineLib/literatura

Entrega valor técnico y reputacional en:

- pit final exacto;
- shells anidados;
- scheduling;
- bounds LP/BZ/Lagrangianos;
- comparabilidad con literatura.

Ambos tracks son válidos, pero no deben gobernarse igual.

El error estratégico más probable sería este:

```text
esperar a cerrar todo el gap bibliográfico de scheduling antes de consolidar el SDK Python.
```

Eso retrasaría valor real para usuarios sin necesidad.

## Diagnóstico por eje

| Eje | Estado | Lectura estratégica |
| --- | --- | --- |
| Arquitectura y boundaries | Fuerte | La separación Rust core / SDK / tools / Python / agentes está bien definida y ya vive en el repo. |
| Core Rust para block models | Fuerte | Ya cubre un alcance amplio y útil: block model, indexing, IO, validation, reblock, analytics, economics, planning base. |
| `mine-sdk` como fachada | Medio-fuerte | Ya existe y es usable, pero todavía falta endurecer qué parte se considera superficie pública recomendada. |
| Tools deterministas | Fuerte | Ya hay un set inicial razonable y coherente con la visión agentica futura. |
| Python SDK | Medio | Ya es funcional, pero todavía necesita UX, ejemplos, packaging y una frontera clara entre estable y experimental. |
| Benchmarks y credibilidad científica | Medio-fuerte | Muy bien en UPIT y sólido en varias piezas; todavía parcial en cierre de comparabilidad fuerte para scheduling avanzado. |
| Capa agentica | Baja | La arquitectura está lista, la implementación real todavía no. |
| Release engineering / benchmark infra | Media | Build y tests pasan, pero aún hay pendientes de telemetría y criterion (`MR-215`, `MR-216`). |
| Gobernanza documental | Media | La documentación es rica, pero ya empieza a aparecer drift entre backlog, roadmap y estado efectivo de algunos frentes. |

## ¿Ya estamos en una fase estable?

La respuesta correcta no es sí/no. Hay que separar cuatro tipos de estabilidad.

## 1. Estabilidad estructural

Sí.

La arquitectura base ya parece suficientemente estable:

- el monorepo tiene sentido;
- los crates están bien separados;
- la dirección de dependencias es coherente;
- Python ya está ubicado donde corresponde;
- la capa agentica está correctamente contenida fuera del core.

No parece necesario rediseñar el repositorio desde cero.

## 2. Estabilidad funcional del core

Sí, dentro de un alcance acotado.

Para workflows base de block model, el proyecto ya superó la fase puramente experimental. Hoy se pueden sostener como capacidades reales:

- construcción de modelos;
- IO CSV/Parquet;
- validación estructurada;
- analytics base;
- curva ley-tonelaje;
- reblocking;
- interoperabilidad inicial con pandas y numpy.

Eso ya es suficiente para hablar de una base útil.

## 3. Estabilidad de API pública

Todavía parcial.

Aunque existe mucha superficie, todavía no está completamente claro qué parte debe considerarse:

- estable para usuarios externos;
- experimental;
- interna o benchmark-side.

Ese ordenamiento es justamente una de las próximas tareas más importantes.

## 4. Estabilidad científico-comparativa

Parcial.

En benchmark y planning, el proyecto muestra mucha madurez técnica, pero no todo está todavía en condición de reclamo fuerte frente a la literatura.

La foto más sana es esta:

- paridad fuerte en UPIT;
- avances fuertes en CPIT;
- mejora importante en PCPSP;
- cierre incompleto en comparabilidad paper-grade de scheduling avanzado.

Esto no invalida el SDK. Solo significa que la parte de planeamiento avanzado debe seguir etiquetada con el nivel de madurez correcto.

## ¿Ya suma valor?

Sí, pero el valor actual está concentrado en un perfil de usuario específico.

Hoy `mine-rs` ya puede sumar valor a:

- ingenieros de minas que trabajan en notebooks o scripts;
- geólogos/modeladores que necesitan validar y transformar modelos;
- equipos técnicos que quieren pipelines reproducibles y auditables;
- desarrolladores que necesitan una base Rust/Python abierta para block models.

Hoy todavía suma menos valor a:

- usuarios que esperan una GUI completa;
- equipos que necesitan optimización de scheduling industrial cerrada y paper-grade en todos los frentes;
- workflows agenticos completos listos para producción.

La decisión estratégica importante es no evaluar el proyecto contra el segundo grupo cuando su valor inmediato ya está en el primero.

## ¿Ya debe generarse como SDK?

Sí.

Más precisamente:

```text
Ya debe gestionarse, documentarse y empaquetarse como SDK alpha.
```

Eso implica varios cambios de mentalidad:

1. Definir una superficie pública recomendada.
2. Separar explícitamente APIs estables de APIs experimentales.
3. Ordenar examples y documentación para usuarios, no solo para contributors.
4. Versionar con disciplina de producto, aunque todavía sea `0.x`.
5. Evitar que los frentes de benchmark experimental dicten toda la experiencia del usuario base.

## Qué debería entrar en ese “SDK alpha”

La superficie alpha recomendada debería enfocarse en lo que ya tiene valor y coherencia hoy:

- `BlockModel` y tipos de grilla;
- IO CSV/Parquet;
- validation y `ValidationReport`;
- indexing `xyz ↔ ijk`;
- analytics base y grade-tonnage;
- reblocking base;
- exportes abiertos principales;
- economics base;
- un subconjunto claro de planning/evaluation que ya sea auditable y no dependa de claims benchmark-side frágiles;
- tools deterministas iniciales.

## Qué no debería venderse todavía como superficie plenamente estable

- capa agentica;
- scheduling avanzado que aún sigue en cierre de comparabilidad;
- prototipos marcados como experimentales;
- cualquier camino cuyo valor actual dependa de wiring benchmark-side muy específico.

## ¿Debe priorizarse la capa Python?

Sí, claramente sí.

De hecho, para la visión del proyecto, este debería ser el frente principal de producto en la próxima etapa.

La razón es simple:

```text
El core Rust ya está lo bastante adelantado como para que el mayor retorno marginal ahora venga de hacerlo usable por ingenieros desde Python.
```

### Por qué Python debe ser la prioridad inmediata

1. Es la superficie de uso declarada por la visión del proyecto.
2. Ya existe una base funcional sobre la cual iterar.
3. Es el puente más directo hacia notebooks, pandas, numpy y adopción real.
4. Permite capturar feedback de usuarios antes de congelar demasiada API en Rust.
5. Desacopla la creación de valor del cierre completo de la agenda de benchmarking avanzado.

### Qué significa priorizar Python de forma correcta

No significa mover lógica a Python.

Significa:

- envolver mejor la lógica Rust ya existente;
- mejorar naming, ergonomía y ejemplos;
- dejar claras las excepciones y los outputs;
- ofrecer flujos completos de usuario;
- añadir documentación notebook-first;
- publicar una experiencia `pip install`/`maturin develop` que se sienta seria.

## Recomendación estratégica principal

La próxima etapa debería organizarse en dos carriles explícitos.

## Carril 1: Productización del SDK base

Este carril debe ser la prioridad principal.

Objetivo:

```text
convertir el estado técnico actual en un SDK alpha claro, usable y demostrable desde Python.
```

Entregables esperados:

- superficie pública recomendada documentada;
- separación estable vs experimental;
- ejemplos Python reproducibles;
- documentación de instalación y uso real;
- tests end-to-end Python/Rust para los flujos principales;
- criterio mínimo de performance y benchmark reproducible para operaciones core;
- estrategia de versionado `0.x` y release notes.

## Carril 2: I+D de scheduling y paridad con literatura

Este carril debe continuar, pero como frente paralelo.

Objetivo:

```text
seguir mejorando la credibilidad y competitividad del motor avanzado de planning sin bloquear la salida del SDK alpha.
```

Entregables esperados:

- cierre incremental de `MR-187`, `MR-205`, `MR-210`, `MR-212`, `MR-213`, `MR-214`, `MR-215`;
- mayor claridad entre resultados core y benchmark-side;
- más telemetría y comparabilidad;
- eventual promoción de partes maduras al núcleo estable.

## Próximos pasos recomendados

## Prioridad 1: declarar el alcance del SDK alpha

Esto debería hacerse ya.

Definir explícitamente:

- qué módulos Rust son “uso recomendado”; 
- qué APIs Python son estables para `0.1.x`/`0.2.x`;
- qué piezas quedan marcadas como experimentales;
- qué claims no deben hacerse todavía.

Sin esa definición, el proyecto ya corre el riesgo de parecer más difuso de lo que realmente es.

## Prioridad 2: endurecer la experiencia Python

Este es probablemente el paso que más valor agrega por unidad de esfuerzo.

En concreto:

- examples notebook-first;
- guías de `load -> validate -> analyze -> reblock -> export`;
- wrappers Python claros para tools cuando aplique;
- mejor documentación de errores;
- coverage de tests sobre flujos públicos, no solo smoke tests.

## Prioridad 3: reforzar release engineering y performance baseline

Hay dos pendientes que conviene cerrar pronto porque afectan credibilidad de SDK:

- `MR-215`: telemetría homogénea de runtime;
- `MR-216`: restaurar la suite criterion versionada.

No son detalles secundarios. Son parte de la señal de madurez de una librería técnica seria.

## Prioridad 4: separar mejor producto SDK vs benchmark research

Conviene formalizar en docs y ejemplos una diferencia clara entre:

- capacidades listas para usuarios;
- capacidades avanzadas experimentales;
- harnesses de benchmark e investigación.

Esa separación evita dos daños:

1. sobreprometer a usuarios finales;
2. subestimar el valor real que el SDK ya tiene hoy.

## Prioridad 5: postergar la capa agentica como iniciativa principal

La capa agentica no debe desaparecer del roadmap, pero sí bajar de prioridad práctica.

La secuencia correcta sigue siendo:

```text
core Rust sólido -> Python usable -> tools estables -> agentes.
```

Adelantar demasiado la capa agentica ahora probablemente dispersaría esfuerzo en la capa menos madura y menos rentable del stack.

## Qué no recomendaría hacer ahora

1. No intentaría empujar una versión `1.0` pública todavía.
2. No haría que el roadmap del SDK quede rehén del cierre total de la campaña MineLib/LP-BZ.
3. No movería foco principal a `python/mine-agents`.
4. No ampliaría demasiado la superficie pública sin antes etiquetar estable vs experimental.
5. No mezclaría en la misma narrativa “SDK usable hoy” con “scheduler paper-comparable en todos los frentes”.

## Criterio práctico para decidir si ya salir como SDK alpha

La decisión recomendada es sí, si se cumplen estas condiciones de gobierno de producto:

1. El paquete Python puede instalarse y ejecutar los flujos principales sin pasos frágiles.
2. Existe una superficie pública documentada y acotada.
3. Las APIs experimentales están claramente marcadas.
4. Los ejemplos públicos cubren los casos de uso base reales.
5. El repo comunica con honestidad qué ya está maduro y qué sigue en investigación.

La buena noticia es que gran parte de la base técnica para eso ya existe.

## Propuesta de narrativa oficial para la siguiente fase

La narrativa recomendada del proyecto debería ser algo cercano a esto:

```text
mine-rs ya es un SDK alpha utilizable para block models, validación,
interoperabilidad abierta, analytics y workflows técnicos desde Python.

El frente de planeamiento avanzado y paridad bibliográfica sigue activo
como una línea de I+D de alto valor, pero no bloquea la consolidación
del SDK base.
```

## Conclusión final

`mine-rs` ya está en una fase suficientemente estable como para empezar a capturar valor real, pero ese valor debe organizarse con foco.

La mejor decisión ahora no es esperar a que todo el roadmap avanzado madure por completo. La mejor decisión es:

1. asumir formalmente que el proyecto ya es un SDK `alpha`;
2. consolidar la capa Python como superficie principal de uso;
3. mantener el frente de benchmarks y scheduling avanzado como carril paralelo de investigación aplicada;
4. dejar la capa agentica para después de estabilizar SDK + tools + UX Python.

En otras palabras:

```text
sí, ya deben tratarlo como SDK;
sí, ya deben priorizar Python;
no, todavía no conviene que la capa agentica sea el frente principal;
y no hace falta esperar la paridad total de scheduling para convertir
el core actual en un producto técnico útil.
```
