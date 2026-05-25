use mine_blockmodel::BlockModel;
use mine_core::{ColumnMiningRole, RequiredColumn};

use crate::{ValidationIssue, ValidationIssueCode, ValidationReport, ValidationSeverity};

/// Valida columnas requeridas, tipos lógicos y unidades mineras críticas.
#[must_use]
pub fn validate_block_model_schema(
    model: &BlockModel,
    required_columns: &[RequiredColumn],
) -> ValidationReport {
    let mut report = ValidationReport::new();

    for required_column in required_columns {
        let Some(column_schema) = model.schema().get(required_column.name()) else {
            report.push(
                ValidationIssue::new(
                    ValidationSeverity::Error,
                    ValidationIssueCode::MissingRequiredColumn,
                    format!("required column `{}` is missing", required_column.name()),
                )
                .with_location(required_column.name().as_str())
                .with_affected_count(model.block_count())
                .with_recommendation("Add the missing column to the model schema and storage."),
            );

            continue;
        };

        if column_schema.logical_type() != required_column.logical_type() {
            report.push(
                ValidationIssue::new(
                    ValidationSeverity::Error,
                    ValidationIssueCode::WrongLogicalType,
                    format!(
                        "column `{}` has logical type `{:?}` but `{:?}` was required",
                        required_column.name(),
                        column_schema.logical_type(),
                        required_column.logical_type()
                    ),
                )
                .with_location(required_column.name().as_str())
                .with_recommendation(
                    "Align the schema logical type with the expected mining meaning.",
                ),
            );
        }
    }

    for (column_id, column_schema) in model.schema().iter() {
        let requires_unit = matches!(
            column_schema.mining_role(),
            ColumnMiningRole::Grade | ColumnMiningRole::Tonnage | ColumnMiningRole::Density
        );

        if requires_unit && column_schema.unit().is_none() {
            report.push(
                ValidationIssue::new(
                    ValidationSeverity::Warning,
                    ValidationIssueCode::MissingMeasurementUnit,
                    format!(
                        "column `{column_id}` has mining role `{:?}` but no measurement unit",
                        column_schema.mining_role()
                    ),
                )
                .with_location(column_id.as_str())
                .with_recommendation(
                    "Declare an explicit measurement unit to avoid ambiguous downstream calculations.",
                ),
            );
        }
    }

    report
}
