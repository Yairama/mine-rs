use mine_sdk::{
    BlockModel, RequiredColumn, ValidationOptions, ValidationReport,
    validate_block_model_with_options,
};
use serde::{Deserialize, Serialize};

use crate::contract::{ToolDescriptor, ToolResponse};

pub(crate) const VALIDATE_MODEL_DESCRIPTOR: ToolDescriptor = ToolDescriptor {
    name: "validate_model",
    description: "Ejecuta validaciones estructurales y devuelve un ValidationReport serializable.",
    input_version: "1",
    output_version: "1",
};

/// Entrada para `validate_model`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateModelInput {
    /// Columnas que deben existir con el tipo lógico esperado.
    #[serde(default)]
    pub required_columns: Vec<RequiredColumn>,
    /// Tolerancia espacial usada por validadores de grilla.
    #[serde(default = "default_validation_tolerance")]
    pub tolerance: f64,
    /// Ejecuta validaciones de schema si está habilitado.
    #[serde(default = "default_validation_flag")]
    pub validate_schema: bool,
    /// Ejecuta validaciones espaciales de grilla regular si está habilitado.
    #[serde(default = "default_validation_flag")]
    pub validate_regular_grid: bool,
    /// Ejecuta validaciones de bloques faltantes si está habilitado.
    #[serde(default = "default_validation_flag")]
    pub validate_missing_blocks: bool,
    /// Ejecuta validaciones de extents observados si está habilitado.
    #[serde(default = "default_validation_flag")]
    pub validate_extents: bool,
    /// Ejecuta validaciones de valores críticos si está habilitado.
    #[serde(default = "default_validation_flag")]
    pub validate_values: bool,
    /// Permite modelos sparse sin reportarlos como gaps.
    #[serde(default)]
    pub allow_sparse: bool,
}

impl Default for ValidateModelInput {
    fn default() -> Self {
        Self {
            required_columns: Vec::new(),
            tolerance: default_validation_tolerance(),
            validate_schema: true,
            validate_regular_grid: true,
            validate_missing_blocks: true,
            validate_extents: true,
            validate_values: true,
            allow_sparse: false,
        }
    }
}

/// Salida de `validate_model`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateModelOutput {
    /// Reporte estructurado producido por el validador.
    pub report: ValidationReport,
}

const fn default_validation_tolerance() -> f64 {
    1e-9
}

const fn default_validation_flag() -> bool {
    true
}

/// Ejecuta la suite de validación actual y devuelve un reporte estructurado.
#[must_use]
pub fn validate_model(
    model: &BlockModel,
    input: &ValidateModelInput,
) -> ToolResponse<ValidateModelOutput> {
    let options = match ValidationOptions::new()
        .with_required_columns(input.required_columns.clone())
        .with_tolerance(input.tolerance)
    {
        Ok(options) => options
            .with_schema_validation(input.validate_schema)
            .with_regular_grid_validation(input.validate_regular_grid)
            .with_missing_block_validation(input.validate_missing_blocks)
            .with_extent_validation(input.validate_extents)
            .with_value_validation(input.validate_values)
            .with_sparse_allowed(input.allow_sparse),
        Err(error) => return ToolResponse::failure(VALIDATE_MODEL_DESCRIPTOR, error),
    };

    ToolResponse::success(
        VALIDATE_MODEL_DESCRIPTOR,
        ValidateModelOutput {
            report: validate_block_model_with_options(model, &options),
        },
    )
}
