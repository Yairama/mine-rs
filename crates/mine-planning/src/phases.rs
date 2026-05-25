use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

/// Asignación de fase derivada desde una columna categórica existente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseAssignment {
    /// Índice lineal del bloque dentro de la grilla.
    pub linear_index: usize,
    /// Identificador textual de la fase asignada.
    pub phase: String,
}

/// Resultado estructurado del etiquetado de fases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseTaggingReport {
    /// Columna usada como fuente de fases.
    pub source_column: ColumnId,
    /// Bloques correctamente etiquetados.
    pub assignments: Vec<PhaseAssignment>,
    /// Índices de bloques sin fase asignada.
    pub unassigned_indices: Vec<usize>,
}

/// Asigna fases a partir de una columna categórica existente del modelo.
pub fn assign_phases_from_column(
    model: &BlockModel,
    source_column: &ColumnId,
) -> Result<PhaseTaggingReport, MineError> {
    let Some(column_data) = model.column(source_column) else {
        return Err(MineError::schema(format!(
            "phase source column `{source_column}` does not exist in block model storage"
        )));
    };

    let mut assignments = Vec::new();
    let mut unassigned_indices = Vec::new();

    match column_data {
        ColumnData::Texts(values) => {
            for (row_index, value) in values.iter().enumerate() {
                let linear_index = model.linear_index_at(row_index)?;

                if value.trim().is_empty() {
                    unassigned_indices.push(linear_index);
                } else {
                    assignments.push(PhaseAssignment {
                        linear_index,
                        phase: value.clone(),
                    });
                }
            }
        }
        ColumnData::Integers(values) => {
            for (row_index, value) in values.iter().enumerate() {
                assignments.push(PhaseAssignment {
                    linear_index: model.linear_index_at(row_index)?,
                    phase: value.to_string(),
                });
            }
        }
        ColumnData::Booleans(values) => {
            for (row_index, value) in values.iter().enumerate() {
                assignments.push(PhaseAssignment {
                    linear_index: model.linear_index_at(row_index)?,
                    phase: value.to_string(),
                });
            }
        }
        ColumnData::Floats(_) => {
            return Err(MineError::schema(format!(
                "phase source column `{source_column}` must be categorical (text, integer or boolean)"
            )));
        }
    }

    Ok(PhaseTaggingReport {
        source_column: source_column.clone(),
        assignments,
        unassigned_indices,
    })
}
