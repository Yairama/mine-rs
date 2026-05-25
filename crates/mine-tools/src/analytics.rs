use mine_sdk::{BasicStatistics, BlockModel, ColumnId, GradeTonnagePoint, GroupedStatistics};
use serde::{Deserialize, Serialize};

use crate::contract::{ToolDescriptor, ToolResponse};

pub(crate) const AGGREGATE_BLOCKS_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "aggregate_blocks",
    description: "Agrupa bloques por una columna categórica con tonelaje explícito.",
    input_version: "1",
    output_version: "1",
};

pub(crate) const GRADE_TONNAGE_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "grade_tonnage",
    description: "Calcula una curva ley-tonelaje con columnas y cutoffs explícitos.",
    input_version: "1",
    output_version: "1",
};

/// Entrada para `aggregate_blocks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateBlocksInput {
    /// Columna categórica usada para agrupar.
    pub group_by: ColumnId,
    /// Columna de tonelaje usada como peso.
    pub tonnage_column: ColumnId,
}

/// Salida de `aggregate_blocks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateBlocksOutput {
    /// Columna usada para agrupar.
    pub group_by: ColumnId,
    /// Columna de tonelaje usada como peso.
    pub tonnage_column: ColumnId,
    /// Estadísticas agregadas calculadas por grupo.
    pub groups: Vec<GroupedStatistics>,
}

/// Entrada para `grade_tonnage`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeTonnageInput {
    /// Columna de ley usada para construir la curva.
    pub grade_column: ColumnId,
    /// Columna de tonelaje usada como peso.
    pub tonnage_column: ColumnId,
    /// Lista explícita de cutoffs a evaluar.
    pub cutoffs: Vec<f64>,
}

/// Resumen ejecutivo de la curva ley-tonelaje calculada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeTonnageSummary {
    /// Cantidad total de bloques considerados en el modelo.
    pub total_block_count: usize,
    /// Tonelaje total del modelo antes de aplicar cutoffs.
    pub total_tonnage: f64,
    /// Cantidad de puntos finalmente calculados.
    pub cutoff_count: usize,
}

/// Salida de `grade_tonnage`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeTonnageOutput {
    /// Columna de ley usada en la evaluación.
    pub grade_column: ColumnId,
    /// Columna de tonelaje usada como peso.
    pub tonnage_column: ColumnId,
    /// Cutoffs solicitados por el consumidor.
    pub requested_cutoffs: Vec<f64>,
    /// Resumen ejecutivo de la ejecución.
    pub summary: GradeTonnageSummary,
    /// Tabla ley-tonelaje generada.
    pub points: Vec<GradeTonnagePoint>,
}

/// Agrupa bloques por una columna categórica con tonelaje explícito.
#[must_use]
pub fn aggregate_blocks(
    model: &BlockModel,
    input: &AggregateBlocksInput,
) -> ToolResponse<AggregateBlocksOutput> {
    match model.grouped_statistics(&input.group_by, &input.tonnage_column) {
        Ok(groups) => ToolResponse::success(
            AGGREGATE_BLOCKS_DESCRIPTOR,
            AggregateBlocksOutput {
                group_by: input.group_by.clone(),
                tonnage_column: input.tonnage_column.clone(),
                groups,
            },
        ),
        Err(error) => ToolResponse::failure(AGGREGATE_BLOCKS_DESCRIPTOR, error),
    }
}

/// Calcula una curva ley-tonelaje con supuestos explícitos.
#[must_use]
pub fn grade_tonnage(
    model: &BlockModel,
    input: &GradeTonnageInput,
) -> ToolResponse<GradeTonnageOutput> {
    let statistics = match model.basic_statistics(&input.tonnage_column) {
        Ok(statistics) => statistics,
        Err(error) => return ToolResponse::failure(GRADE_TONNAGE_DESCRIPTOR, error),
    };

    match model.grade_tonnage_curve(&input.grade_column, &input.tonnage_column, &input.cutoffs) {
        Ok(points) => ToolResponse::success(
            GRADE_TONNAGE_DESCRIPTOR,
            GradeTonnageOutput {
                grade_column: input.grade_column.clone(),
                tonnage_column: input.tonnage_column.clone(),
                requested_cutoffs: input.cutoffs.clone(),
                summary: build_grade_tonnage_summary(model, &statistics, points.len()),
                points,
            },
        ),
        Err(error) => ToolResponse::failure(GRADE_TONNAGE_DESCRIPTOR, error),
    }
}

fn build_grade_tonnage_summary(
    model: &BlockModel,
    statistics: &BasicStatistics,
    cutoff_count: usize,
) -> GradeTonnageSummary {
    GradeTonnageSummary {
        total_block_count: model.block_count(),
        total_tonnage: statistics.total_tonnage,
        cutoff_count,
    }
}
