use std::collections::{BTreeMap, BTreeSet};

use mine_sdk::{BlockModel, ColumnData, ColumnId, Coordinate3D, MineError};
use serde::{Deserialize, Serialize};

use crate::contract::{ToolDescriptor, ToolResponse};

pub(crate) const QUERY_BLOCKS_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "query_blocks",
    description: "Filtra, pagina y materializa filas con columnas seleccionadas del modelo.",
    input_version: "1",
    output_version: "1",
};

/// Filtro soportado por `query_blocks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryFilter {
    /// Conserva filas cuya columna flotante sea mayor o igual a un mínimo.
    FloatMinimum {
        /// Columna flotante a evaluar.
        column: ColumnId,
        /// Umbral mínimo incluido.
        minimum: f64,
    },
    /// Conserva filas cuya columna de texto coincide exactamente con un valor.
    TextMatch {
        /// Columna de texto a comparar.
        column: ColumnId,
        /// Valor textual esperado.
        value: String,
    },
    /// Conserva filas dentro de un rango espacial inclusivo.
    CoordinateRange {
        /// Coordenada mínima del rango.
        minimum: Coordinate3D,
        /// Coordenada máxima del rango.
        maximum: Coordinate3D,
    },
}

/// Entrada para `query_blocks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryBlocksInput {
    /// Filtros aplicados de forma conjuntiva.
    pub filters: Vec<QueryFilter>,
    /// Columnas a devolver. Si queda vacío, se devuelven todas las columnas.
    pub selected_columns: Vec<ColumnId>,
    /// Desplazamiento inicial de la página.
    pub offset: usize,
    /// Máximo de filas a devolver.
    pub limit: usize,
}

impl Default for QueryBlocksInput {
    fn default() -> Self {
        Self {
            filters: Vec::new(),
            selected_columns: Vec::new(),
            offset: 0,
            limit: 100,
        }
    }
}

/// Valor serializable devuelto por `query_blocks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryValue {
    /// Valor entero.
    Integer(i64),
    /// Valor flotante.
    Float(f64),
    /// Valor booleano.
    Boolean(bool),
    /// Valor de texto.
    Text(String),
}

/// Fila individual devuelta por `query_blocks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryRow {
    /// Índice lineal implícito dentro de la grilla.
    pub linear_index: usize,
    /// Valores serializados por columna.
    pub values: BTreeMap<ColumnId, QueryValue>,
}

/// Salida de `query_blocks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryBlocksOutput {
    /// Filtros aplicados durante la consulta.
    pub filters: Vec<QueryFilter>,
    /// Columnas devueltas en cada fila.
    pub selected_columns: Vec<ColumnId>,
    /// Número total de filas que cumplen la consulta antes de paginar.
    pub total_matches: usize,
    /// Número de filas devueltas en la página actual.
    pub returned_count: usize,
    /// Indica si hubo truncamiento por paginación.
    pub truncated: bool,
    /// Offset sugerido para la siguiente página.
    pub next_offset: Option<usize>,
    /// Filas materializadas por la consulta.
    pub rows: Vec<QueryRow>,
}

/// Filtra bloques, selecciona columnas y devuelve filas paginadas.
#[must_use]
pub fn query_blocks(
    model: &BlockModel,
    input: &QueryBlocksInput,
) -> ToolResponse<QueryBlocksOutput> {
    if input.limit == 0 {
        return ToolResponse::failure(
            QUERY_BLOCKS_DESCRIPTOR,
            MineError::invalid_parameter("limit", "query_blocks limit must be greater than zero"),
        );
    }

    let selected_columns = resolved_selected_columns(model, &input.selected_columns);

    if let Err(error) = ensure_selected_columns_exist(model, &selected_columns) {
        return ToolResponse::failure(QUERY_BLOCKS_DESCRIPTOR, error);
    }

    let mut matching_indices = (0..model.block_count()).collect::<Vec<_>>();

    for filter in &input.filters {
        let selection = match apply_query_filter(model, filter) {
            Ok(selection) => selection,
            Err(error) => return ToolResponse::failure(QUERY_BLOCKS_DESCRIPTOR, error),
        };
        let allowed_indices = selection.indices().iter().copied().collect::<BTreeSet<_>>();

        matching_indices.retain(|index| allowed_indices.contains(index));
    }

    let total_matches = matching_indices.len();
    let page_start = input.offset.min(total_matches);
    let page_end = page_start.saturating_add(input.limit).min(total_matches);
    let rows = match build_query_rows(
        model,
        &selected_columns,
        &matching_indices[page_start..page_end],
    ) {
        Ok(rows) => rows,
        Err(error) => return ToolResponse::failure(QUERY_BLOCKS_DESCRIPTOR, error),
    };
    let next_offset = (page_end < total_matches).then_some(page_end);

    ToolResponse::success(
        QUERY_BLOCKS_DESCRIPTOR,
        QueryBlocksOutput {
            filters: input.filters.clone(),
            selected_columns,
            total_matches,
            returned_count: rows.len(),
            truncated: next_offset.is_some(),
            next_offset,
            rows,
        },
    )
}

fn resolved_selected_columns(model: &BlockModel, selected_columns: &[ColumnId]) -> Vec<ColumnId> {
    if !selected_columns.is_empty() {
        return selected_columns.to_vec();
    }

    model
        .schema()
        .iter()
        .map(|(column_id, _)| column_id.clone())
        .collect()
}

fn ensure_selected_columns_exist(
    model: &BlockModel,
    selected_columns: &[ColumnId],
) -> Result<(), MineError> {
    for column_id in selected_columns {
        if model.column(column_id).is_none() {
            return Err(MineError::schema(format!(
                "column `{column_id}` does not exist in block model storage"
            )));
        }
    }

    Ok(())
}

fn apply_query_filter(
    model: &BlockModel,
    filter: &QueryFilter,
) -> Result<mine_sdk::BlockSelection, MineError> {
    match filter {
        QueryFilter::FloatMinimum { column, minimum } => {
            model.filter_by_float_min(column, *minimum)
        }
        QueryFilter::TextMatch { column, value } => model.filter_by_text_match(column, value),
        QueryFilter::CoordinateRange { minimum, maximum } => {
            model.filter_by_coordinate_range(*minimum, *maximum)
        }
    }
}

fn build_query_rows(
    model: &BlockModel,
    selected_columns: &[ColumnId],
    indices: &[usize],
) -> Result<Vec<QueryRow>, MineError> {
    let mut rows = Vec::with_capacity(indices.len());

    for row_index in indices {
        let mut values = BTreeMap::new();
        let linear_index = model.linear_index_at(*row_index)?;

        for column_id in selected_columns {
            let Some(column_data) = model.column(column_id) else {
                return Err(MineError::schema(format!(
                    "column `{column_id}` does not exist in block model storage"
                )));
            };

            values.insert(column_id.clone(), query_value_at(column_data, *row_index)?);
        }

        rows.push(QueryRow {
            linear_index,
            values,
        });
    }

    Ok(rows)
}

fn query_value_at(column_data: &ColumnData, index: usize) -> Result<QueryValue, MineError> {
    match column_data {
        ColumnData::Integers(values) => values
            .get(index)
            .copied()
            .map(QueryValue::Integer)
            .ok_or_else(|| {
                MineError::validation("query row index is out of bounds for integer column")
            }),
        ColumnData::Floats(values) => values
            .get(index)
            .copied()
            .map(QueryValue::Float)
            .ok_or_else(|| {
                MineError::validation("query row index is out of bounds for float column")
            }),
        ColumnData::Booleans(values) => values
            .get(index)
            .copied()
            .map(QueryValue::Boolean)
            .ok_or_else(|| {
                MineError::validation("query row index is out of bounds for boolean column")
            }),
        ColumnData::Texts(values) => {
            values
                .get(index)
                .cloned()
                .map(QueryValue::Text)
                .ok_or_else(|| {
                    MineError::validation("query row index is out of bounds for text column")
                })
        }
    }
}
