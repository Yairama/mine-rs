use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

use crate::internal::validate_non_negative_finite;

/// Tolerancias explícitas para un reporte de reconciliación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconciliationTolerances {
    /// Diferencia absoluta máxima permitida para tonelaje total.
    pub tonnage_absolute: f64,
    /// Diferencia relativa máxima permitida para tonelaje total.
    pub tonnage_relative: f64,
    /// Diferencia absoluta máxima permitida para metal contenido.
    pub contained_metal_absolute: f64,
    /// Diferencia relativa máxima permitida para metal contenido.
    pub contained_metal_relative: f64,
    /// Diferencia absoluta máxima permitida para ley media.
    pub average_grade_absolute: f64,
    /// Diferencia relativa máxima permitida para ley media.
    pub average_grade_relative: f64,
    /// Diferencia absoluta máxima permitida para conteo de bloques materializados.
    pub block_count_absolute: usize,
}

impl ReconciliationTolerances {
    /// Construye tolerancias no negativas y finitas.
    pub fn new(
        tonnage_absolute: f64,
        tonnage_relative: f64,
        contained_metal_absolute: f64,
        contained_metal_relative: f64,
        average_grade_absolute: f64,
        average_grade_relative: f64,
        block_count_absolute: usize,
    ) -> Result<Self, MineError> {
        validate_non_negative_finite("tonnage_absolute", tonnage_absolute)?;
        validate_non_negative_finite("tonnage_relative", tonnage_relative)?;
        validate_non_negative_finite("contained_metal_absolute", contained_metal_absolute)?;
        validate_non_negative_finite("contained_metal_relative", contained_metal_relative)?;
        validate_non_negative_finite("average_grade_absolute", average_grade_absolute)?;
        validate_non_negative_finite("average_grade_relative", average_grade_relative)?;

        Ok(Self {
            tonnage_absolute,
            tonnage_relative,
            contained_metal_absolute,
            contained_metal_relative,
            average_grade_absolute,
            average_grade_relative,
            block_count_absolute,
        })
    }
}

/// Delta cuantitativo before/after para una métrica continua.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconciliationMetric {
    /// Valor before.
    pub before: Option<f64>,
    /// Valor after.
    pub after: Option<f64>,
    /// Diferencia absoluta `after - before`.
    pub absolute_difference: Option<f64>,
    /// Diferencia relativa `abs(after - before) / abs(before)`.
    pub relative_difference: Option<f64>,
    /// Indica si alguna tolerancia configurada fue excedida.
    pub tolerance_exceeded: bool,
}

/// Delta before/after para el conteo de bloques materializados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconciliationBlockCount {
    /// Conteo before.
    pub before: usize,
    /// Conteo after.
    pub after: usize,
    /// Diferencia absoluta en cantidad de bloques.
    pub absolute_difference: usize,
    /// Indica si la tolerancia de bloques fue excedida.
    pub tolerance_exceeded: bool,
}

/// Reporte serializable de reconciliación before/after.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconciliationReport {
    /// Comparación de tonelaje total.
    pub tonnage: ReconciliationMetric,
    /// Comparación de metal contenido.
    pub contained_metal: ReconciliationMetric,
    /// Comparación de ley media.
    pub average_grade: ReconciliationMetric,
    /// Comparación de bloques materializados.
    pub block_count: ReconciliationBlockCount,
}

/// Compara dos modelos before/after sobre tonelaje, metal, ley media y conteo de bloques.
pub fn reconcile_models(
    before: &BlockModel,
    after: &BlockModel,
    tonnage_column: &ColumnId,
    grade_column: &ColumnId,
    tolerances: &ReconciliationTolerances,
) -> Result<ReconciliationReport, MineError> {
    let before_tonnage = sum_float_column(before, tonnage_column)?;
    let after_tonnage = sum_float_column(after, tonnage_column)?;
    let before_contained_metal = sum_contained_metal(before, tonnage_column, grade_column)?;
    let after_contained_metal = sum_contained_metal(after, tonnage_column, grade_column)?;
    let before_average_grade =
        (before_tonnage > 0.0).then_some(before_contained_metal / before_tonnage);
    let after_average_grade =
        (after_tonnage > 0.0).then_some(after_contained_metal / after_tonnage);

    let tonnage = build_reconciliation_metric(
        Some(before_tonnage),
        Some(after_tonnage),
        tolerances.tonnage_absolute,
        tolerances.tonnage_relative,
    );
    let contained_metal = build_reconciliation_metric(
        Some(before_contained_metal),
        Some(after_contained_metal),
        tolerances.contained_metal_absolute,
        tolerances.contained_metal_relative,
    );
    let average_grade = build_reconciliation_metric(
        before_average_grade,
        after_average_grade,
        tolerances.average_grade_absolute,
        tolerances.average_grade_relative,
    );
    let block_count_difference = before.block_count().abs_diff(after.block_count());
    let block_count = ReconciliationBlockCount {
        before: before.block_count(),
        after: after.block_count(),
        absolute_difference: block_count_difference,
        tolerance_exceeded: block_count_difference > tolerances.block_count_absolute,
    };

    Ok(ReconciliationReport {
        tonnage,
        contained_metal,
        average_grade,
        block_count,
    })
}

fn build_reconciliation_metric(
    before: Option<f64>,
    after: Option<f64>,
    absolute_tolerance: f64,
    relative_tolerance: f64,
) -> ReconciliationMetric {
    match (before, after) {
        (Some(before), Some(after)) => {
            let absolute_difference = after - before;
            let relative_difference =
                (before != 0.0).then_some(absolute_difference.abs() / before.abs());
            let tolerance_exceeded = absolute_difference.abs() > absolute_tolerance
                || relative_difference.is_some_and(|value| value > relative_tolerance);

            ReconciliationMetric {
                before: Some(before),
                after: Some(after),
                absolute_difference: Some(absolute_difference),
                relative_difference,
                tolerance_exceeded,
            }
        }
        (None, None) => ReconciliationMetric {
            before: None,
            after: None,
            absolute_difference: None,
            relative_difference: None,
            tolerance_exceeded: false,
        },
        _ => ReconciliationMetric {
            before,
            after,
            absolute_difference: None,
            relative_difference: None,
            tolerance_exceeded: true,
        },
    }
}

fn sum_float_column(model: &BlockModel, column: &ColumnId) -> Result<f64, MineError> {
    let column_data = model.column(column).ok_or_else(|| {
        MineError::schema(format!(
            "reconciliation column `{column}` does not exist in block model storage"
        ))
    })?;

    match column_data {
        ColumnData::Floats(values) => {
            let mut total = 0.0;
            for value in values {
                if !value.is_finite() {
                    return Err(MineError::numeric(
                        "reconciliation requires finite float column values",
                    ));
                }
                total += *value;
            }
            Ok(total)
        }
        _ => Err(MineError::invalid_parameter(
            "columns",
            format!("reconciliation requires float column `{column}`"),
        )),
    }
}

fn sum_contained_metal(
    model: &BlockModel,
    tonnage_column: &ColumnId,
    grade_column: &ColumnId,
) -> Result<f64, MineError> {
    let tonnage_data = model.column(tonnage_column).ok_or_else(|| {
        MineError::schema(format!(
            "reconciliation column `{tonnage_column}` does not exist in block model storage"
        ))
    })?;
    let grade_data = model.column(grade_column).ok_or_else(|| {
        MineError::schema(format!(
            "reconciliation column `{grade_column}` does not exist in block model storage"
        ))
    })?;

    match (tonnage_data, grade_data) {
        (ColumnData::Floats(tonnages), ColumnData::Floats(grades)) => {
            if tonnages.len() != grades.len() {
                return Err(MineError::validation(
                    "reconciliation requires tonnage and grade columns with matching row counts",
                ));
            }

            let mut contained_metal = 0.0;
            for (tonnage, grade) in tonnages.iter().zip(grades.iter()) {
                if !tonnage.is_finite() || !grade.is_finite() {
                    return Err(MineError::numeric(
                        "reconciliation requires finite tonnage and grade values",
                    ));
                }
                contained_metal += *tonnage * *grade;
            }

            Ok(contained_metal)
        }
        _ => Err(MineError::invalid_parameter(
            "columns",
            format!(
                "reconciliation requires float columns `{tonnage_column}` and `{grade_column}`"
            ),
        )),
    }
}
