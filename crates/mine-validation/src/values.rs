use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::ColumnMiningRole;

use crate::{ValidationIssue, ValidationIssueCode, ValidationReport, ValidationSeverity};

/// Valida rangos y sanidad numérica de columnas mineras críticas.
#[must_use]
pub fn validate_block_model_values(model: &BlockModel) -> ValidationReport {
    let mut report = ValidationReport::new();

    for (column_id, column_schema) in model.schema().iter() {
        let Some(column_data) = model.column(column_id) else {
            continue;
        };

        match (column_schema.mining_role(), column_data) {
            (ColumnMiningRole::Grade, ColumnData::Floats(values)) => {
                let affected_count = values.iter().filter(|value| !value.is_finite()).count();

                if affected_count > 0 {
                    report.push(
                        ValidationIssue::new(
                            ValidationSeverity::Error,
                            ValidationIssueCode::NonFiniteGradeValue,
                            format!(
                                "column `{column_id}` contains {affected_count} non-finite grade value(s)"
                            ),
                        )
                        .with_location(column_id.as_str())
                        .with_affected_count(affected_count)
                        .with_recommendation(
                            "Replace NaN or infinite grade values before running downstream calculations.",
                        ),
                    );
                }
            }
            (ColumnMiningRole::Tonnage, ColumnData::Floats(values)) => {
                let affected_count = values
                    .iter()
                    .filter(|value| !value.is_finite() || **value < 0.0)
                    .count();

                if affected_count > 0 {
                    report.push(
                        ValidationIssue::new(
                            ValidationSeverity::Error,
                            ValidationIssueCode::InvalidTonnageValue,
                            format!(
                                "column `{column_id}` contains {affected_count} invalid tonnage value(s)"
                            ),
                        )
                        .with_location(column_id.as_str())
                        .with_affected_count(affected_count)
                        .with_recommendation(
                            "Ensure tonnage values are finite and greater than or equal to zero.",
                        ),
                    );
                }
            }
            (ColumnMiningRole::Tonnage, ColumnData::Integers(values)) => {
                let affected_count = values.iter().filter(|value| **value < 0).count();

                if affected_count > 0 {
                    report.push(
                        ValidationIssue::new(
                            ValidationSeverity::Error,
                            ValidationIssueCode::InvalidTonnageValue,
                            format!(
                                "column `{column_id}` contains {affected_count} invalid tonnage value(s)"
                            ),
                        )
                        .with_location(column_id.as_str())
                        .with_affected_count(affected_count)
                        .with_recommendation(
                            "Ensure tonnage values are finite and greater than or equal to zero.",
                        ),
                    );
                }
            }
            (ColumnMiningRole::Density, ColumnData::Floats(values)) => {
                let affected_count = values
                    .iter()
                    .filter(|value| !value.is_finite() || **value <= 0.0)
                    .count();

                if affected_count > 0 {
                    report.push(
                        ValidationIssue::new(
                            ValidationSeverity::Error,
                            ValidationIssueCode::InvalidDensityValue,
                            format!(
                                "column `{column_id}` contains {affected_count} invalid density value(s)"
                            ),
                        )
                        .with_location(column_id.as_str())
                        .with_affected_count(affected_count)
                        .with_recommendation(
                            "Ensure density values are finite and strictly greater than zero.",
                        ),
                    );
                }
            }
            (ColumnMiningRole::Recovery, ColumnData::Floats(values)) => {
                let affected_count = values
                    .iter()
                    .filter(|value| !value.is_finite() || **value < 0.0 || **value > 1.0)
                    .count();

                if affected_count > 0 {
                    report.push(
                        ValidationIssue::new(
                            ValidationSeverity::Error,
                            ValidationIssueCode::InvalidRecoveryValue,
                            format!(
                                "column `{column_id}` contains {affected_count} invalid recovery value(s)"
                            ),
                        )
                        .with_location(column_id.as_str())
                        .with_affected_count(affected_count)
                        .with_recommendation(
                            "Ensure recovery values are finite and stay within the inclusive 0..=1 range.",
                        ),
                    );
                }
            }
            _ => {}
        }
    }

    report
}
