# Capa agentica

La capa agentica de `mine-rs` debe construirse sobre el SDK, no dentro del core de cálculo. Su función es orquestar, planificar, explicar y verificar workflows mineros usando tools deterministas.

La inspiración metodológica mencionada para esta capa es `github.com/langchain-ai/deep-agents-from-scratch/`, especialmente ideas como task tools, subagents, virtual filesystem y separación entre razonamiento y ejecución.

## Estado actual

Esta capa no está implementada. No existe hoy un paquete `python/mine-agents`, runtime de agentes, VFS agentico ni orquestación por subagents que forme parte del producto.

Su implementación está explícitamente pospuesta hasta estabilizar, en este orden, la superficie de tools deterministas, el SDK Python, los contratos de artefactos/VFS y la disciplina operativa de releases. El resto de este documento describe diseño objetivo, no capacidades disponibles.

## Principio central

```text
Los agentes razonan y orquestan.
Las tools deterministas calculan y validan.
```

Este principio evita que un modelo de lenguaje "invente" cálculos mineros. Un agente puede decidir llamar `validate_model`, pero no debe reemplazar internamente la validación estructural del SDK.

## Rol de la capa agentica

La capa agentica debe permitir que un usuario haga solicitudes como:

- "Revisa este modelo de bloques y dime si tiene problemas."
- "Genera una curva ley-tonelaje para cobre y resume los cutoffs relevantes."
- "Compara dos escenarios de minado."
- "Crea una secuencia básica respetando avance vertical máximo."
- "Explícame qué bloques están causando inconsistencias."

El agente debe transformar esas solicitudes en pasos verificables.

## Ubicación recomendada

La capa agentica debe estar separada del SDK base, pero no necesita vivir en otro repositorio al inicio.

La recomendación es:

- Mantenerla dentro del monorepo.
- Crear un paquete Python separado, por ejemplo `python/mine-agents`.
- Hacer que dependa de `miners` y de los contratos de `mine-tools`.
- No hacer que `miners` dependa de la capa agentica.
- Crear crates Rust agenticos solo para piezas deterministas si hacen falta.

Esto encaja mejor con la inspiración en `deep-agents-from-scratch`, porque el runtime agentico y ecosistemas como LangGraph/LangChain viven naturalmente en Python.

### ¿Debe ser un crate Rust?

No como runtime principal inicial.

La parte agentica puede usar crates Rust para componentes específicos:

- `mine-tools` para tools deterministas.
- `mine-agent-vfs` si se necesita un VFS local robusto.
- `mine-agent-verifier` si se necesita verificación determinista.

Pero el orquestador de agentes, subagents, prompts, graph runtime y task tools debería empezar como paquete Python separado dentro del monorepo.

## Arquitectura conceptual

```text
User
↓
Mine Agent
↓
Task Tool
↓
Subagents especializados
↓
mine-rs deterministic tools
↓
JSON structured outputs
↓
VFS artifacts
↓
Verifier / explanation
```

## Componentes

### Mine Agent

Agente principal que recibe la intención del usuario, mantiene el contexto general y decide qué tareas deben ejecutarse.

Responsabilidades:

- Interpretar la solicitud.
- Preguntar aclaraciones cuando falten datos.
- Crear un plan.
- Delegar subtareas.
- Resumir resultados.
- Mantener trazabilidad.

### Task Tool

Mecanismo para delegar trabajos concretos. Una task debe tener:

- Objetivo.
- Inputs.
- Outputs esperados.
- Criterios de éxito.
- Tools permitidas.
- Artefactos a producir.

### Subagents especializados

Subagents posibles:

- Model inspector.
- Validation agent.
- Reblocking agent.
- Economics agent.
- Planning agent.
- Scenario comparison agent.
- Verifier agent.

Cada subagent debe tener un ámbito claro. Por ejemplo, un validation agent no debería crear escenarios económicos.

### Virtual filesystem

El VFS funciona como memoria de trabajo y repositorio de artefactos.

Puede contener:

- Perfiles de modelo.
- Reportes de validación.
- Curvas generadas.
- Tablas intermedias.
- Escenarios.
- Comparaciones.
- Notas del agente.
- Logs de decisiones.

Los artefactos deben tener nombres, metadata y formato claros.

### Deterministic tools

Las tools son wrappers estructurados sobre el SDK.

Ejemplos:

- `inspect_model`.
- `validate_model`.
- `query_blocks`.
- `aggregate_blocks`.
- `grade_tonnage`.
- `create_scenario`.
- `evaluate_scenario`.
- `compare_scenarios`.

Cada tool debe tener input y output schema.

### Verifier

El verifier revisa consistencia.

Puede validar:

- Si los outputs existen.
- Si las tools se ejecutaron con parámetros correctos.
- Si los resultados contradicen validaciones previas.
- Si las conclusiones del agente están soportadas por datos.
- Si hay supuestos no declarados.

## Tool contracts

Cada tool debe declarar:

- Nombre.
- Descripción.
- Input schema.
- Output schema.
- Errores posibles.
- Artefactos producidos.
- Supuestos.
- Versión.

Ejemplo conceptual:

```json
{
  "tool": "grade_tonnage",
  "input": {
    "model_ref": "vfs://models/base.parquet",
    "grade_column": "cu",
    "tonnage_column": "tonnes",
    "cutoffs": [0.1, 0.2, 0.3]
  },
  "output": {
    "curve_ref": "vfs://reports/grade_tonnage_cu.json",
    "summary": {
      "rows": 3
    }
  }
}
```

## Outputs estructurados

La capa agentica debe preferir JSON estructurado para outputs intermedios. Esto permite:

- Validación.
- Comparación.
- Reuso.
- Persistencia.
- Auditoría.
- Renderizado posterior.

Los resúmenes en lenguaje natural deben derivarse de datos estructurados, no reemplazarlos.

## Flujo ejemplo

Solicitud:

```text
Revisa este modelo de bloques y genera una curva ley-tonelaje para cobre.
```

Flujo:

1. El Mine Agent registra el archivo de entrada.
2. Ejecuta `inspect_model`.
3. Ejecuta `validate_model`.
4. Si hay errores críticos, detiene el flujo o pide confirmación.
5. Si el modelo es usable, ejecuta `grade_tonnage`.
6. Guarda reportes en VFS.
7. El verifier revisa que los outputs existan y sean consistentes.
8. El agente entrega una explicación y enlaces a artefactos.

## Riesgos a evitar

### Cálculos dentro del prompt

El agente no debe calcular resultados mineros complejos manualmente en texto.

### Outputs sin trazabilidad

Toda conclusión importante debe poder rastrearse a una tool o artefacto.

### Subagents sin límites

Los subagents deben tener responsabilidades claras para evitar comportamiento impredecible.

### Validaciones opcionales

Para workflows críticos, la validación no debe ser un paso opcional silencioso.

## Relación con el SDK

La capa agentica depende del SDK en tres niveles:

- Objetos de dominio.
- Tools deterministas.
- Artefactos serializables.

Si una capacidad no existe en el SDK, el agente debe reconocer la limitación y no simularla como si fuera real.

## Roadmap agentico futuro y pospuesto

1. Definir schemas de tools.
2. Implementar `inspect_model` y `validate_model`.
3. Crear VFS simple.
4. Crear task tool.
5. Crear inspector agent.
6. Crear validation agent.
7. Crear grade-tonnage agent.
8. Crear verifier.
9. Crear planning agent.
10. Crear scenario comparison agent.
