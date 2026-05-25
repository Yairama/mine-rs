//! Contratos para manejar realizaciones condicionales con lineage explicito.
//!
//! Este modulo define artefactos serializables para ensembles geologicos sin
//! introducir sampling ni algoritmos estocasticos dentro del core actual.

use std::collections::BTreeSet;

use mine_core::{ArtifactId, ColumnId, Metadata, MineError, ModelId};
use serde::{Deserialize, Serialize};

/// Formatos abiertos admitidos para almacenar realizaciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealizationStorageFormat {
    /// Almacenamiento columnar en Parquet.
    Parquet,
    /// Intercambio columnar Arrow IPC.
    ArrowIpc,
    /// Contrato JSON estructurado.
    Json,
}

/// Soporte comun que deben compartir todas las realizaciones del ensemble.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizationSupport {
    block_count: usize,
    is_sparse: bool,
    grid_artifact_id: ArtifactId,
}

impl RealizationSupport {
    /// Construye el soporte comun de las realizaciones.
    pub fn new(
        block_count: usize,
        is_sparse: bool,
        grid_artifact_id: ArtifactId,
    ) -> Result<Self, MineError> {
        if block_count == 0 {
            return Err(MineError::invalid_parameter(
                "block_count",
                "realization support block count must be greater than zero",
            ));
        }

        Ok(Self {
            block_count,
            is_sparse,
            grid_artifact_id,
        })
    }

    /// Cantidad de bloques materializados esperada para cada realizacion.
    #[must_use]
    pub const fn block_count(&self) -> usize {
        self.block_count
    }

    /// Indica si el soporte esperado es sparse.
    #[must_use]
    pub const fn is_sparse(&self) -> bool {
        self.is_sparse
    }

    /// Artefacto que describe la grilla o layout comun del ensemble.
    #[must_use]
    pub fn grid_artifact_id(&self) -> &ArtifactId {
        &self.grid_artifact_id
    }
}

/// Lineage minimo requerido para una realizacion condicional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionalRealizationLineage {
    conditioning_artifact_ids: Vec<ArtifactId>,
    estimator_artifact_id: Option<ArtifactId>,
}

impl ConditionalRealizationLineage {
    /// Construye el lineage validando que los artefactos de condicionamiento sean explicitos.
    pub fn new(
        conditioning_artifact_ids: Vec<ArtifactId>,
        estimator_artifact_id: Option<ArtifactId>,
    ) -> Result<Self, MineError> {
        if conditioning_artifact_ids.is_empty() {
            return Err(MineError::invalid_parameter(
                "conditioning_artifact_ids",
                "conditional realizations require at least one conditioning artifact",
            ));
        }

        let mut seen = BTreeSet::new();
        for artifact_id in &conditioning_artifact_ids {
            if !seen.insert(artifact_id.clone()) {
                return Err(MineError::validation(format!(
                    "duplicate conditioning artifact id `{artifact_id}` in realization lineage"
                )));
            }
        }

        Ok(Self {
            conditioning_artifact_ids,
            estimator_artifact_id,
        })
    }

    /// Artefactos de condicionamiento usados para generar la realizacion.
    #[must_use]
    pub fn conditioning_artifact_ids(&self) -> &[ArtifactId] {
        &self.conditioning_artifact_ids
    }

    /// Artefacto del estimador o simulador, cuando existe.
    #[must_use]
    pub fn estimator_artifact_id(&self) -> Option<&ArtifactId> {
        self.estimator_artifact_id.as_ref()
    }
}

/// Contrato serializable de una realizacion condicional individual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalRealization {
    realization_id: ArtifactId,
    realization_index: usize,
    base_model_id: ModelId,
    sampled_columns: Vec<ColumnId>,
    storage_artifact_id: ArtifactId,
    storage_format: RealizationStorageFormat,
    method: String,
    random_seed: u64,
    support: RealizationSupport,
    lineage: ConditionalRealizationLineage,
    metadata: Metadata,
}

impl ConditionalRealization {
    /// Construye una realizacion individual con sampling explicito.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        realization_id: ArtifactId,
        realization_index: usize,
        base_model_id: ModelId,
        sampled_columns: Vec<ColumnId>,
        storage_artifact_id: ArtifactId,
        storage_format: RealizationStorageFormat,
        method: impl Into<String>,
        random_seed: u64,
        support: RealizationSupport,
        lineage: ConditionalRealizationLineage,
        metadata: Metadata,
    ) -> Result<Self, MineError> {
        let method = method.into();
        if method.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "method",
                "realization method must not be empty",
            ));
        }

        validate_sampled_columns(&sampled_columns)?;

        Ok(Self {
            realization_id,
            realization_index,
            base_model_id,
            sampled_columns,
            storage_artifact_id,
            storage_format,
            method,
            random_seed,
            support,
            lineage,
            metadata,
        })
    }

    /// Identificador estable de la realizacion.
    #[must_use]
    pub fn realization_id(&self) -> &ArtifactId {
        &self.realization_id
    }

    /// Indice estable de la realizacion dentro del ensemble.
    #[must_use]
    pub const fn realization_index(&self) -> usize {
        self.realization_index
    }

    /// Modelo base al que pertenece la realizacion.
    #[must_use]
    pub fn base_model_id(&self) -> &ModelId {
        &self.base_model_id
    }

    /// Columnas simuladas o perturbadas por la realizacion.
    #[must_use]
    pub fn sampled_columns(&self) -> &[ColumnId] {
        &self.sampled_columns
    }

    /// Artefacto donde se almacena la realizacion.
    #[must_use]
    pub fn storage_artifact_id(&self) -> &ArtifactId {
        &self.storage_artifact_id
    }

    /// Formato abierto del artefacto de salida.
    #[must_use]
    pub const fn storage_format(&self) -> RealizationStorageFormat {
        self.storage_format
    }

    /// Metodo usado para generar la realizacion.
    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Seed explicita usada por la realizacion.
    #[must_use]
    pub const fn random_seed(&self) -> u64 {
        self.random_seed
    }

    /// Soporte comun que debe compartir con el resto del ensemble.
    #[must_use]
    pub fn support(&self) -> &RealizationSupport {
        &self.support
    }

    /// Lineage de condicionamiento y estimacion.
    #[must_use]
    pub fn lineage(&self) -> &ConditionalRealizationLineage {
        &self.lineage
    }

    /// Metadata adicional de la realizacion.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Ensemble de realizaciones condicionales con soporte consistente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalRealizationSet {
    ensemble_id: ArtifactId,
    base_model_id: ModelId,
    sampled_columns: Vec<ColumnId>,
    support: RealizationSupport,
    realizations: Vec<ConditionalRealization>,
    metadata: Metadata,
}

impl ConditionalRealizationSet {
    /// Construye un ensemble validando consistencia de soporte, columnas y lineage explicito.
    pub fn new(
        ensemble_id: ArtifactId,
        base_model_id: ModelId,
        sampled_columns: Vec<ColumnId>,
        support: RealizationSupport,
        realizations: Vec<ConditionalRealization>,
        metadata: Metadata,
    ) -> Result<Self, MineError> {
        validate_sampled_columns(&sampled_columns)?;

        if realizations.is_empty() {
            return Err(MineError::invalid_parameter(
                "realizations",
                "conditional realization set must contain at least one realization",
            ));
        }

        let mut seen_ids = BTreeSet::new();
        let mut seen_indices = BTreeSet::new();
        for realization in &realizations {
            if realization.base_model_id() != &base_model_id {
                return Err(MineError::validation(format!(
                    "realization `{}` belongs to model `{}` but ensemble expects `{}`",
                    realization.realization_id(),
                    realization.base_model_id(),
                    base_model_id
                )));
            }

            if realization.sampled_columns() != sampled_columns.as_slice() {
                return Err(MineError::validation(format!(
                    "realization `{}` does not match the ensemble sampled columns",
                    realization.realization_id()
                )));
            }

            if realization.support() != &support {
                return Err(MineError::validation(format!(
                    "realization `{}` does not match the ensemble support",
                    realization.realization_id()
                )));
            }

            if !seen_ids.insert(realization.realization_id().clone()) {
                return Err(MineError::validation(format!(
                    "duplicate realization id `{}` in conditional realization set",
                    realization.realization_id()
                )));
            }

            if !seen_indices.insert(realization.realization_index()) {
                return Err(MineError::validation(format!(
                    "duplicate realization index `{}` in conditional realization set",
                    realization.realization_index()
                )));
            }
        }

        Ok(Self {
            ensemble_id,
            base_model_id,
            sampled_columns,
            support,
            realizations,
            metadata,
        })
    }

    /// Identificador estable del ensemble.
    #[must_use]
    pub fn ensemble_id(&self) -> &ArtifactId {
        &self.ensemble_id
    }

    /// Modelo base comun a todas las realizaciones.
    #[must_use]
    pub fn base_model_id(&self) -> &ModelId {
        &self.base_model_id
    }

    /// Columnas que cada realizacion debe portar con el mismo contrato.
    #[must_use]
    pub fn sampled_columns(&self) -> &[ColumnId] {
        &self.sampled_columns
    }

    /// Soporte comun del ensemble.
    #[must_use]
    pub fn support(&self) -> &RealizationSupport {
        &self.support
    }

    /// Realizaciones del ensemble.
    #[must_use]
    pub fn realizations(&self) -> &[ConditionalRealization] {
        &self.realizations
    }

    /// Metadata global del ensemble.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Cantidad de realizaciones del ensemble.
    #[must_use]
    pub fn realization_count(&self) -> usize {
        self.realizations.len()
    }

    /// Busca una realizacion por id.
    #[must_use]
    pub fn realization(&self, realization_id: &ArtifactId) -> Option<&ConditionalRealization> {
        self.realizations
            .iter()
            .find(|realization| realization.realization_id() == realization_id)
    }
}

fn validate_sampled_columns(sampled_columns: &[ColumnId]) -> Result<(), MineError> {
    if sampled_columns.is_empty() {
        return Err(MineError::invalid_parameter(
            "sampled_columns",
            "conditional realizations require at least one sampled column",
        ));
    }

    let mut seen = BTreeSet::new();
    for column_id in sampled_columns {
        if !seen.insert(column_id.clone()) {
            return Err(MineError::validation(format!(
                "duplicate sampled column `{column_id}` in conditional realization contract"
            )));
        }
    }

    Ok(())
}
