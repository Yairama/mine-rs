# Capacidades de dominio

Este documento organiza las capacidades mineras objetivo de `mine-rs`. No representa el estado implementado, sino el mapa funcional hacia el que debe evolucionar el SDK.

## 1. Block model engine

El motor de modelos de bloques es la base del proyecto.

### Objetivos

- Representar modelos regulares, rotados y eventualmente sparse.
- Manejar atributos por bloque de forma columnar.
- Soportar metadata global y por columna.
- Permitir operaciones sobre modelos grandes.
- Mantener compatibilidad con workflows Python.

### Funcionalidades esperadas

- `BlockModel`.
- `GridDefinition`.
- `BlockDimensions`.
- `Coordinate3D`.
- Atributos por bloque.
- Column store.
- Metadata serializable.
- Selección de columnas.
- Filtros por coordenadas, dominio, banco, fase o atributo.

### Preguntas de diseño

- Cómo representar subbloques.
- Cómo manejar modelos sparse.
- Cómo separar geometría de atributos.
- Cómo evitar copias innecesarias entre Rust y Python.

## 2. Indexing y coordenadas

La indexación es necesaria para ubicar bloques, validar grillas y construir operaciones eficientes.

### Funcionalidades esperadas

- Conversión `xyz → ijk`.
- Conversión `ijk → xyz`.
- Índices lineales.
- Soporte para origen, dimensiones y rotación.
- Tolerancias numéricas explícitas.
- Validación de límites.

### Casos de uso

- Detectar duplicados.
- Detectar gaps.
- Buscar bloques vecinos.
- Crear precedencias.
- Agrupar por banco.
- Transformar modelos exportados desde otros sistemas.

## 3. Validación de modelos

La validación debe detectar problemas antes de que un modelo sea usado para cálculos, escenarios o agentes.

### Validadores iniciales

- Duplicados espaciales.
- Bloques faltantes.
- Extents inconsistentes.
- Coordenadas fuera de grilla.
- Inconsistencias de dimensiones.
- Columnas obligatorias faltantes.
- Valores nulos en campos críticos.
- Tipos de datos no esperados.

### Output esperado

Los validadores deben producir un reporte estructurado:

- Severidad.
- Código de issue.
- Mensaje.
- Ubicación o filtro afectado.
- Conteo de bloques afectados.
- Recomendación.
- Metadata para exportación JSON.

## 4. Reblocking

El reblocking debe ser determinista y auditable.

### Funcionalidades esperadas

- Superblocking.
- Subblocking.
- Reblocking adaptativo futuro.
- Reglas declarativas de agregación.
- Promedios ponderados.
- Sumas conservativas.
- Mínimos, máximos y conteos.
- Reportes before/after.

### Reglas típicas

- Tonelaje: suma.
- Metal: suma.
- Ley: promedio ponderado por tonelaje.
- Densidad: promedio ponderado por volumen o tonelaje.
- Categorías: regla explícita, no inferida.

### Criterios de calidad

- Conservación de masa cuando aplique.
- Conservación de metal cuando aplique.
- Tolerancias reportadas.
- Reproducibilidad del resultado.

## 5. IO e interoperabilidad

La utilidad del SDK depende de poder entrar y salir de workflows reales.

### Formatos objetivo

- CSV para interoperabilidad simple.
- Parquet para almacenamiento columnar.
- Arrow para intercambio eficiente.
- VTK/VTU para visualización.
- Exportaciones compatibles con software minero cuando sea viable.

### Consideraciones

- Preservar metadata.
- Evitar pérdida silenciosa de tipos.
- Reportar columnas no reconocidas.
- Permitir schemas explícitos.
- Soportar datasets grandes.

## 6. Analytics mineros

Las funciones analíticas deben cubrir cálculos frecuentes.

### Funcionalidades iniciales

- Tonelaje total.
- Ley media.
- Metal contenido.
- Estadísticas por dominio.
- Estadísticas por banco.
- Estadísticas por fase.
- Curvas ley-tonelaje.
- Tablas de cut-off.

### Funcionalidades futuras

- Sensibilidades.
- Comparación de escenarios.
- Reportes multi-variable.
- Métricas por destino.
- Indicadores de reconciliación.

## 7. Economía minera

La economía debe construirse con supuestos explícitos.

### Primitives esperadas

- Precio.
- Recuperación.
- Costo mina.
- Costo planta.
- Costo de venta.
- Revenue.
- Margen.
- Cashflow.
- NPV.

### Principio

No debe existir una "economía implícita". Cada cálculo económico debe exponer sus parámetros y permitir auditar sus fórmulas.

## 8. Planeamiento, pushbacks y secuencias

El planeamiento debe empezar con primitives simples y evolucionar hacia simulaciones más completas.

### Funcionalidades objetivo

- Generación de benches.
- Asignación de fases.
- Pushbacks.
- Precedence graph.
- Restricciones de avance vertical.
- Reglas de extracción.
- Secuencias por periodo.
- Escenarios.
- Comparación de escenarios.

### Uso esperado

Un usuario debería poder construir un escenario con reglas explícitas y obtener outputs verificables:

- Bloques por periodo.
- Tonelaje por periodo.
- Ley por periodo.
- Metal por periodo.
- Cashflow por periodo.
- Violaciones de restricciones.

## 9. Visualización y exportación

`mine-rs` no necesita ser una herramienta visual al inicio, pero sí debe producir artefactos útiles.

### Exportaciones objetivo

- Archivos para ParaView.
- Tablas para notebooks.
- JSON estructurado.
- CSV resumidos.
- Parquet enriquecido.
- Reportes Markdown o HTML futuros.

## 10. Tools deterministas

Las tools son la interfaz ideal entre SDK y agentes.

### Tools iniciales

- `inspect_model`.
- `validate_model`.
- `query_blocks`.
- `aggregate_blocks`.
- `grade_tonnage`.
- `create_scenario`.
- `evaluate_scenario`.
- `compare_scenarios`.

### Contrato esperado

Cada tool debe definir:

- Input schema.
- Output schema.
- Errores posibles.
- Supuestos.
- Artefactos producidos.
- Referencias a datos usados.

## 11. Preparación y estimación de resource model

Aunque `mine-rs` ya trabaja bien con block models existentes, una ruta end-to-end útil debe cubrir también la preparación del modelo que alimenta economía y planeamiento.

### Funcionalidades objetivo

- Compositing determinista.
- Domaining duro y auditoría de límites.
- Declustering y estadísticas ponderadas.
- Variografía experimental.
- Ajuste de modelos autorizados.
- Estimadores deterministas base.
- Regularización de soporte de bloque.
- Validación del modelo estimado.
- Métricas explícitas de clasificación.

### Límites iniciales

- El MVP no debe empezar por simulación condicional pesada.
- La clasificación debe producir métricas y evidencia, no automatizar compliance.
- La geología implícita y el wireframing quedan fuera del alcance inicial del SDK base.

### Referencia recomendada

La secuencia sugerida, los artefactos intermedios y las referencias bibliográficas para esta parte del motor viven en `docs/references/mining-engine-roadmap.md`.

## Priorización sugerida

1. Modelo de bloques.
2. Indexing.
3. IO básico.
4. Validación.
5. Python bindings.
6. Analytics básicos.
7. Reblocking.
8. Tools deterministas.
9. Planeamiento.
10. Capa agentica.
