use mine_blockmodel::BlockModel;
use mine_core::{MineError, RequiredColumn};
use serde::{Deserialize, Serialize};

use crate::{ValidationReport, suite::validate_block_model_with_options};

/// Configuración explícita para ejecutar la suite de validación del modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationOptions {
    required_columns: Vec<RequiredColumn>,
    tolerance: f64,
    validate_schema: bool,
    validate_regular_grid: bool,
    validate_missing_blocks: bool,
    validate_extents: bool,
    validate_values: bool,
    allow_sparse: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            required_columns: Vec::new(),
            tolerance: 1e-9,
            validate_schema: true,
            validate_regular_grid: true,
            validate_missing_blocks: true,
            validate_extents: true,
            validate_values: true,
            allow_sparse: false,
        }
    }
}

impl ValidationOptions {
    /// Construye un set de opciones con la suite completa habilitada.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reemplaza las columnas requeridas del validador.
    #[must_use]
    pub fn with_required_columns(mut self, required_columns: Vec<RequiredColumn>) -> Self {
        self.required_columns = required_columns;
        self
    }

    /// Ajusta la tolerancia usada por validadores espaciales.
    pub fn with_tolerance(mut self, tolerance: f64) -> Result<Self, MineError> {
        validate_tolerance(tolerance)?;
        self.tolerance = tolerance;
        Ok(self)
    }

    /// Activa o desactiva la validación de schema.
    #[must_use]
    pub fn with_schema_validation(mut self, enabled: bool) -> Self {
        self.validate_schema = enabled;
        self
    }

    /// Activa o desactiva la validación de grilla regular.
    #[must_use]
    pub fn with_regular_grid_validation(mut self, enabled: bool) -> Self {
        self.validate_regular_grid = enabled;
        self
    }

    /// Activa o desactiva la detección de bloques faltantes.
    #[must_use]
    pub fn with_missing_block_validation(mut self, enabled: bool) -> Self {
        self.validate_missing_blocks = enabled;
        self
    }

    /// Activa o desactiva la validación de extents observados.
    #[must_use]
    pub fn with_extent_validation(mut self, enabled: bool) -> Self {
        self.validate_extents = enabled;
        self
    }

    /// Activa o desactiva la validación de valores críticos.
    #[must_use]
    pub fn with_value_validation(mut self, enabled: bool) -> Self {
        self.validate_values = enabled;
        self
    }

    /// Permite modelos sparse cuando la falta de bloques es intencional.
    #[must_use]
    pub fn with_sparse_allowed(mut self, allowed: bool) -> Self {
        self.allow_sparse = allowed;
        self
    }

    /// Devuelve las columnas requeridas configuradas.
    #[must_use]
    pub fn required_columns(&self) -> &[RequiredColumn] {
        &self.required_columns
    }

    /// Devuelve la tolerancia espacial configurada.
    #[must_use]
    pub const fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Indica si debe ejecutarse la validación de schema.
    #[must_use]
    pub const fn validates_schema(&self) -> bool {
        self.validate_schema
    }

    /// Indica si debe ejecutarse la validación de grilla regular.
    #[must_use]
    pub const fn validates_regular_grid(&self) -> bool {
        self.validate_regular_grid
    }

    /// Indica si debe ejecutarse la validación de bloques faltantes.
    #[must_use]
    pub const fn validates_missing_blocks(&self) -> bool {
        self.validate_missing_blocks
    }

    /// Indica si debe ejecutarse la validación de extents observados.
    #[must_use]
    pub const fn validates_extents(&self) -> bool {
        self.validate_extents
    }

    /// Indica si debe ejecutarse la validación de valores críticos.
    #[must_use]
    pub const fn validates_values(&self) -> bool {
        self.validate_values
    }

    /// Indica si la suite debe aceptar modelos sparse sin reportarlos como gaps.
    #[must_use]
    pub const fn allows_sparse(&self) -> bool {
        self.allow_sparse
    }
}

/// Extensión de validación para ejecutar la suite directamente sobre `BlockModel`.
pub trait BlockModelValidationExt {
    /// Ejecuta la suite completa con opciones por defecto.
    fn validate(&self) -> ValidationReport;

    /// Ejecuta la suite usando una configuración explícita.
    fn validate_with_options(&self, options: &ValidationOptions) -> ValidationReport;
}

impl BlockModelValidationExt for BlockModel {
    fn validate(&self) -> ValidationReport {
        validate_block_model_with_options(self, &ValidationOptions::default())
    }

    fn validate_with_options(&self, options: &ValidationOptions) -> ValidationReport {
        validate_block_model_with_options(self, options)
    }
}

pub(crate) fn validate_tolerance(tolerance: f64) -> Result<(), MineError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        Err(MineError::numeric(
            "tolerance must be finite and greater than or equal to zero",
        ))
    } else {
        Ok(())
    }
}
