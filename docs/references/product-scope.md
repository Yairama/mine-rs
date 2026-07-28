# Alcance de producto

Este documento define qué busca cubrir `mine-rs`, qué queda fuera del alcance inicial y cómo se separan el SDK, la capa Python, la capa agentica y una posible UI futura.

## Producto base

El producto base es un SDK de utilidades mineras.

Esto significa que `mine-rs` debe proveer componentes programables para que otros construyan workflows. El foco inicial no es entregar una aplicación final para usuarios no técnicos, sino una librería confiable para ingeniería, análisis y automatización.

## Qué entra en el SDK

### Modelos de bloques

El modelo de bloques es el objeto central del proyecto. El SDK debe permitir:

- Definir grillas.
- Representar bloques.
- Manejar coordenadas.
- Almacenar atributos por bloque.
- Gestionar metadata.
- Consultar, filtrar y agregar información.
- Trabajar con datasets grandes.

### Indexación espacial

El SDK debe ofrecer conversiones confiables entre:

- Coordenadas `x, y, z`.
- Índices `i, j, k`.
- Índices lineales.

También debe contemplar grillas rotadas y, en fases posteriores, modelos sparse o irregulares.

### Validación

Debe existir un conjunto de validadores para detectar problemas comunes:

- Bloques duplicados.
- Bloques faltantes.
- Extents inconsistentes.
- Grillas no regulares.
- Columnas requeridas ausentes.
- Tipos de datos incompatibles.
- Valores inválidos o fuera de rango.

La validación debe producir reportes estructurados, no solo texto.

### Reblocking

El SDK debe incluir operaciones de reblocking con reglas explícitas:

- Superblocking.
- Subblocking.
- Agregación ponderada.
- Conservación de tonelaje.
- Conservación de metal.
- Reportes de reconciliación.

### IO e interoperabilidad

El SDK debe poder leer y escribir formatos usados en flujos reales:

- CSV.
- Parquet.
- Arrow IPC.
- Exportaciones visuales como VTK/VTU.
- Exportaciones compatibles con software minero cuando sea viable.

La prioridad inicial debe estar en formatos abiertos y reproducibles.

### Analytics mineros

El SDK debe cubrir cálculos básicos y luego avanzados:

- Tonelaje.
- Ley media.
- Metal contenido.
- Curvas ley-tonelaje.
- Reportes por dominio, fase, banco o destino.
- Comparaciones entre escenarios.

### Economía minera

En fases posteriores, el SDK debe incluir primitives económicas:

- Precios.
- Recuperaciones.
- Costos.
- Revenue.
- Cashflow.
- NPV.
- Sensibilidades.

Estas funciones deben ser explícitas en sus supuestos.

### Planeamiento y secuencias

El SDK debe proveer primitives para construir lógica de planeamiento:

- Bench generation.
- Phase tagging.
- Pushbacks.
- Precedence graphs.
- Restricciones de avance vertical.
- Secuencias.
- Escenarios.
- Comparación de planes.

El objetivo no es reemplazar optimizadores especializados desde el primer día, sino crear primitives robustas para construir workflows de planeamiento.

## Qué entra en la capa Python

La capa Python es la interfaz principal para usuarios finales.

Debe permitir:

- Cargar modelos.
- Ejecutar validaciones.
- Transformar modelos.
- Calcular reportes.
- Integrarse con pandas y numpy.
- Usarse en notebooks.
- Exportar resultados.
- Invocar tools de forma programática.

La capa Python no debe duplicar la lógica crítica si ya existe en Rust. Debe envolverla de forma ergonómica.

## Qué entra en la capa agentica

Estado actual: no implementada y explícitamente pospuesta hasta estabilizar tools, SDK Python, contratos de artefactos/VFS y disciplina de releases.

Cuando se implemente, la capa agentica deberá construirse sobre el SDK, no dentro del core minero.

Debe encargarse de:

- Interpretar instrucciones.
- Crear planes de trabajo.
- Dividir tareas.
- Delegar a subagents.
- Ejecutar tools deterministas.
- Escribir artefactos a un VFS.
- Verificar consistencia de resultados.
- Explicar outputs al usuario.

La capa agentica no debe inventar cálculos ni reemplazar validadores.

## Qué queda fuera del alcance inicial

En la etapa inicial no se prioriza:

- Una GUI completa.
- Un sistema enterprise multiusuario.
- Manejo avanzado de permisos.
- Integración directa con todos los formatos propietarios.
- Optimización matemática compleja de pit final desde cero.
- Simulación geotécnica o metalúrgica detallada.
- Reemplazo de software minero especializado.
- Agentes autónomos sin validación determinista.

Estos temas pueden aparecer en fases futuras, pero no deben bloquear la construcción del SDK base.

## Límites importantes

### SDK no es agente

El SDK calcula, valida y transforma. El agente orquesta, explica y coordina. Esta separación evita que el sistema sea difícil de auditar.

### Python no reemplaza Rust

Python debe ser la interfaz cómoda. Rust debe ser la fuente de verdad para performance, tipos y operaciones críticas.

### Backlog no es documentación de producto

`docs/backlog.md` contiene tickets operativos. Los documentos estratégicos deben explicar por qué existe el proyecto y cómo se organiza, mientras el backlog indica qué construir.

## MVP conceptual

Un MVP útil para ingenieros debería permitir:

1. Cargar un block model desde CSV o Parquet.
2. Definir o inferir una grilla.
3. Validar duplicados, gaps y extents.
4. Ejecutar conversiones `xyz ↔ ijk`.
5. Calcular estadísticas y curvas ley-tonelaje.
6. Rebloquear con reglas simples.
7. Exportar resultados.
8. Usar todo desde Python.

Una futura capa agentica podrá empezar con tools sobre este MVP, una vez cerradas las dependencias de estabilización anteriores:

- `inspect_model`.
- `validate_model`.
- `query_blocks`.
- `aggregate_blocks`.
- `grade_tonnage`.

## Criterio de éxito

`mine-rs` será exitoso si logra que tareas mineras frecuentes puedan expresarse como código claro, reproducible y auditable, con performance suficiente para modelos grandes y una API Python que se sienta natural para usuarios técnicos.
