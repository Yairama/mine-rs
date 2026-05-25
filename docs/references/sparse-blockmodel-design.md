# Diseño experimental de block models sparse

## Estado

Este documento describe la primera representación **experimental** de modelos sparse en `mine-rs`.

No reemplaza al `BlockModel` denso actual. Extiende su layout interno para permitir que un modelo materialice solo una parte de la grilla sin romper la API regular ya existente.

## Objetivo

Habilitar una base determinista para:

- representar bloques no materializados;
- distinguir entre gaps inválidos y sparse permitido;
- preservar una ruta compatible con filtros, analytics y tools existentes;
- evitar una bifurcación temprana entre `DenseBlockModel` y `SparseBlockModel`.

## Decisión de diseño

`BlockModel` mantiene la misma responsabilidad y el mismo crate (`mine-blockmodel`), pero ahora puede construirse de dos formas:

- `BlockModel::new(...)` para modelos densos;
- `BlockModel::new_sparse(...)` para modelos con materialización explícita.

La representación elegida es una `BlockLayout`:

- `Dense`
- `Sparse { materialized_linear_indices }`

## Invariantes actuales

La variante sparse impone estas reglas:

1. `materialized_linear_indices` debe estar en orden estrictamente creciente.
2. No se permiten duplicados.
3. Ningún índice puede salir de la capacidad de la grilla.
4. La longitud de cada columna materializada debe coincidir con la cantidad de índices sparse.
5. El orden de filas del storage columnar debe coincidir exactamente con el orden de `materialized_linear_indices`.

Estas reglas permiten mantener determinismo y evitar mapeos ambiguos entre filas materializadas y posiciones de grilla.

## Tradeoffs aceptados

### Ventajas

- No rompe la ruta densa existente.
- Permite reutilizar analytics y filtros porque siguen operando por fila materializada.
- Hace explícita la diferencia entre:
  - cantidad de filas materializadas;
  - capacidad total de la grilla.
- Deja una base clara para validadores de gaps y futuras operaciones sparse-aware.

### Costos

- Parte del IO actual sigue orientado a modelos densos.
- Algunas operaciones todavía reportan `extent` derivado de la grilla completa y no del subconjunto materializado.
- La detección de duplicados dentro de `BlockModel` no ocurre como validador posterior, porque la constructor sparse ya rechaza duplicados en sus invariantes.

## Semántica actual

- `block_count()` pasa a significar **bloques materializados**.
- `grid_cell_count()` devuelve la capacidad total de la grilla.
- `is_sparse()` indica si la layout es sparse.
- `linear_index_at(row_index)` permite recuperar la posición real de una fila materializada.
- `missing_linear_indices()` devuelve las celdas faltantes respecto de la grilla base.

## Relación con validación

La suite actual de validación usa esta base para:

- validar roundtrip espacial sobre bloques materializados;
- detectar bloques faltantes;
- permitir modelos sparse cuando el caller lo declara explícitamente (`allow_sparse=true`).

Esto no cierra todavía:

- validación de duplicados sobre artefactos previos a la normalización a `BlockModel`;
- validación de extents observados vs esperados para sparse y rotados;
- una ruta de IO sparse-first completa.

## Criterio de evolución

Si el proyecto necesita más capacidades sparse, la siguiente expansión natural es:

1. preservar layout sparse en más rutas de IO;
2. definir `extent` observado sobre bloques materializados;
3. agregar reportes más ricos para gaps por rangos y no solo por conteo/ejemplos;
4. evaluar si conviene introducir vistas o iteradores sparse especializados antes de crear un tipo nuevo separado.

## Regla arquitectónica

La representación sparse sigue viviendo en `mine-blockmodel`, porque pertenece al modelo de dominio determinista y no a Python, tools o la capa agentica.
