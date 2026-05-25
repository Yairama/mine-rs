use mine_blockmodel::BlockModel;
use mine_core::RequiredColumn;

use crate::{
    ValidationOptions, ValidationReport,
    schema::validate_block_model_schema,
    spatial::{
        validate_block_model_extents, validate_block_model_missing_blocks,
        validate_block_model_regular_grid,
    },
    values::validate_block_model_values,
};

/// Punto de entrada inicial para validar un `BlockModel`.
#[must_use]
pub fn validate_block_model(
    model: &BlockModel,
    required_columns: &[RequiredColumn],
) -> ValidationReport {
    validate_block_model_with_options(
        model,
        &ValidationOptions::default().with_required_columns(required_columns.to_vec()),
    )
}

/// Ejecuta la suite de validación usando una configuración explícita.
#[must_use]
pub fn validate_block_model_with_options(
    model: &BlockModel,
    options: &ValidationOptions,
) -> ValidationReport {
    let mut report = ValidationReport::new();

    if options.validates_schema() {
        report.extend(validate_block_model_schema(model, options.required_columns()).issues);
    }

    if options.validates_regular_grid() {
        report.extend(validate_block_model_regular_grid(model, options.tolerance()).issues);
    }

    if options.validates_missing_blocks() {
        report.extend(validate_block_model_missing_blocks(model, options.allows_sparse()).issues);
    }

    if options.validates_extents() {
        report.extend(
            validate_block_model_extents(model, options.tolerance(), options.allows_sparse())
                .issues,
        );
    }

    if options.validates_values() {
        report.extend(validate_block_model_values(model).issues);
    }

    report
}
