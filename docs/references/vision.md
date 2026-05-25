# Visión de mine-rs

`mine-rs` nace como una iniciativa para crear infraestructura computacional minera abierta, modular y reproducible. Su objetivo no es empezar como una aplicación gráfica monolítica, sino como una base SDK-first sobre la que puedan construirse librerías, notebooks, automatizaciones, herramientas agenticas y, eventualmente, interfaces de usuario.

## Misión

Construir un SDK de ingeniería de minas que permita representar, transformar, validar, analizar y planificar sobre datos mineros de forma determinista, eficiente e interoperable.

La misión práctica es darle a un ingeniero de minas una caja de herramientas programable para resolver tareas como:

- Cargar y validar modelos de bloques.
- Convertir coordenadas espaciales a índices de grilla.
- Rebloquear y reconciliar modelos.
- Calcular curvas ley-tonelaje.
- Evaluar escenarios económicos.
- Generar primitives para fases, benches, precedencias y secuencias.
- Automatizar análisis mediante Python.
- Exponer tools confiables para agentes.

## Visión

La visión de largo plazo es que `mine-rs` se convierta en una capa de infraestructura para minería computacional, comparable en filosofía a proyectos como Polars, DuckDB, PyTorch o Bevy, pero aplicada al dominio minero.

No se busca construir solamente "otro software minero", sino una plataforma base que permita:

- Reusar lógica minera en distintos productos.
- Construir workflows reproducibles.
- Integrar datos mineros con ecosistemas modernos de data engineering.
- Hacer que Python sea una interfaz cómoda sin sacrificar performance.
- Separar cálculo determinista de razonamiento agentico.
- Permitir que equipos técnicos auditen, extiendan y versionen sus procesos.

## Qué significa "infraestructura computacional minera"

Infraestructura computacional minera significa construir componentes que otros sistemas puedan usar:

- Tipos de datos para representar bloques, coordenadas, grillas, dominios, fases y escenarios.
- Motores para indexar, validar, agregar, calcular y exportar.
- Contratos de entrada/salida estables.
- APIs de bajo y alto nivel.
- Bindings Python para adopción práctica.
- Tools estructuradas para automatización y agentes.

Esto contrasta con una aplicación cerrada en la que la lógica queda atrapada detrás de una interfaz. `mine-rs` debe permitir que la lógica minera sea versionable, testeable y componible.

## Principios de diseño

### 1. Core determinista

Los cálculos deben ser reproducibles. Dado el mismo input, una operación debe producir el mismo output, con tolerancias explícitas cuando existan cálculos numéricos.

Esto es especialmente importante para:

- Validación de modelos.
- Reblocking.
- Agregaciones ponderadas.
- Reportes de reconciliación.
- Evaluación económica.
- Secuencias de minado.

### 2. Rust para el cómputo crítico

Rust debe encargarse de las partes que requieren performance, seguridad de memoria y estructuras robustas:

- Modelos de datos.
- Indexación.
- Operaciones sobre grandes volúmenes de bloques.
- IO columnar.
- Validadores.
- Motores de agregación.
- Primitives de planeamiento.

### 3. Python para la experiencia de usuario

Python debe ser la superficie principal para usuarios técnicos. El objetivo es que un ingeniero de minas pueda trabajar desde notebooks, scripts o pipelines sin tener que conocer Rust.

La API Python debe ser:

- Clara.
- Consistente.
- Tipada cuando sea posible.
- Compatible con pandas, numpy y Arrow.
- Fácil de integrar en flujos existentes.

### 4. SDK antes que GUI

Una UI puede existir en el futuro, pero no debe ser el centro del diseño inicial. Primero se deben construir primitives confiables.

La prioridad es que el proyecto pueda usarse como:

- Librería Rust.
- Librería Python.
- CLI futura.
- Backend de tools agenticas.
- Motor para aplicaciones futuras.

### 5. Interoperabilidad minera moderna

Los datos mineros rara vez viven en un solo formato. El SDK debe priorizar formatos abiertos y flujos reales:

- CSV.
- Parquet.
- Arrow.
- pandas.
- numpy.
- VTK/VTU para visualización.
- Exportaciones compatibles con herramientas mineras cuando sea viable.

### 6. AI-native, pero verificable

`mine-rs` debe poder alimentar agentes, pero no debe delegar la verdad técnica a un modelo de lenguaje. Los agentes pueden:

- Interpretar solicitudes.
- Diseñar planes.
- Seleccionar tools.
- Explicar resultados.
- Generar reportes.
- Pedir validaciones.

Pero los cálculos deben ejecutarse en tools deterministas del SDK.

## Audiencia objetivo

### Ingenieros de minas

Usuarios que necesitan analizar modelos, validar datos, generar escenarios, revisar curvas, construir secuencias y automatizar tareas repetitivas.

### Geólogos y modeladores

Usuarios que trabajan con modelos de bloques, dominios, atributos y validación estructural.

### Analistas de datos mineros

Usuarios que conectan modelos con Python, pandas, notebooks, dashboards y pipelines.

### Desarrolladores de software minero

Equipos que necesitan una base reusable para construir herramientas internas o productos.

### Agentes y sistemas automatizados

Sistemas que necesitan tools confiables para operar sobre datos mineros sin inventar cálculos.

## Resultado esperado

El resultado final debe ser un ecosistema donde un usuario pueda:

1. Cargar un modelo de bloques.
2. Validarlo estructuralmente.
3. Consultarlo y transformarlo.
4. Calcular métricas mineras.
5. Crear escenarios.
6. Evaluarlos y compararlos.
7. Generar reportes y artefactos.
8. Automatizar el flujo con agentes cuando tenga sentido.

## Frase guía

```text
mine-rs es infraestructura computacional minera: cálculos deterministas, APIs modernas y automatización agentica verificable.
```
