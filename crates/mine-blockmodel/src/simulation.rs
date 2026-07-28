use std::collections::{BTreeMap, BTreeSet};

use mine_core::{ArtifactId, ColumnId, Coordinate3D, Metadata, MetadataValue, MineError, ModelId};
use serde::{Deserialize, Serialize};

use crate::{
    ConditionalRealization, ConditionalRealizationLineage, ConditionalRealizationSet,
    EstimationPass, KrigingEstimate, RealizationStorageFormat, RealizationSupport,
    SimpleKrigingOptions, SpatialSample, VariogramModel, estimate_simple_kriging,
};

/// Nodo objetivo explícito para simulación secuencial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationTarget {
    /// Identificador estable del nodo objetivo.
    pub target_id: String,
    /// Coordenada puntual del nodo a simular.
    pub location: Coordinate3D,
    /// Dominio opcional del nodo; cuando existe, se respeta durante la búsqueda.
    pub domain: Option<String>,
}

impl SimulationTarget {
    /// Construye un objetivo validando identificador y dominio.
    pub fn new(
        target_id: impl Into<String>,
        location: Coordinate3D,
        domain: Option<String>,
    ) -> Result<Self, MineError> {
        let target_id = target_id.into();
        if target_id.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "target_id",
                "simulation target id must not be empty",
            ));
        }
        if let Some(domain) = &domain
            && domain.trim().is_empty()
        {
            return Err(MineError::invalid_parameter(
                "domain",
                "simulation target domain must not be empty when provided",
            ));
        }

        Ok(Self {
            target_id,
            location,
            domain,
        })
    }
}

/// Valor simulado en un nodo objetivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulatedNodeValue {
    /// Identificador del nodo objetivo.
    pub target_id: String,
    /// Coordenada del nodo.
    pub location: Coordinate3D,
    /// Dominio opcional del nodo.
    pub domain: Option<String>,
    /// Valor simulado.
    pub value: f64,
}

/// Resumen mínimo de una realización simulada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequentialSimulationSummary {
    /// Cantidad de nodos simulados.
    pub node_count: usize,
    /// Valor mínimo.
    pub min_value: f64,
    /// Valor máximo.
    pub max_value: f64,
    /// Media simple de la realización.
    pub mean_value: f64,
}

/// Resultado explícito de una realización secuencial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequentialSimulationRealization {
    /// Descriptor serializable compatible con contratos de realizaciones.
    pub descriptor: ConditionalRealization,
    /// Valores simulados en cada target del soporte.
    pub values: Vec<SimulatedNodeValue>,
    /// Resumen mínimo para QA rápido.
    pub summary: SequentialSimulationSummary,
}

/// Ensemble pequeño de prototipos secuenciales reproducibles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequentialSimulationEnsemble {
    /// Contrato serializable del ensemble.
    pub descriptor: ConditionalRealizationSet,
    /// Realizaciones materializadas.
    pub realizations: Vec<SequentialSimulationRealization>,
}

/// Opciones explícitas para SGS prototipo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequentialGaussianSimulationOptions {
    known_mean: f64,
}

impl SequentialGaussianSimulationOptions {
    /// Construye opciones validadas para SGS prototipo.
    pub fn new(known_mean: f64) -> Result<Self, MineError> {
        if !known_mean.is_finite() {
            return Err(MineError::invalid_parameter(
                "known_mean",
                "sequential gaussian simulation known mean must be finite",
            ));
        }

        Ok(Self { known_mean })
    }

    /// Media conocida usada por simple kriging.
    #[must_use]
    pub const fn known_mean(&self) -> f64 {
        self.known_mean
    }
}

/// Opciones explícitas para SIS prototipo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequentialIndicatorSimulationOptions {
    cutoff: f64,
}

impl SequentialIndicatorSimulationOptions {
    /// Construye opciones validadas para SIS prototipo.
    pub fn new(cutoff: f64) -> Result<Self, MineError> {
        if !cutoff.is_finite() {
            return Err(MineError::invalid_parameter(
                "cutoff",
                "sequential indicator simulation cutoff must be finite",
            ));
        }

        Ok(Self { cutoff })
    }

    /// Cutoff binario usado para transformar muestras en indicadores.
    #[must_use]
    pub const fn cutoff(&self) -> f64 {
        self.cutoff
    }
}

/// Genera un ensemble pequeño de SGS prototipo usando simple kriging y seeds explícitos.
#[allow(clippy::too_many_arguments)]
pub fn generate_sequential_gaussian_ensemble(
    ensemble_id: ArtifactId,
    base_model_id: ModelId,
    column_id: ColumnId,
    grid_artifact_id: ArtifactId,
    conditioning_artifact_ids: Vec<ArtifactId>,
    conditioning_samples: &[SpatialSample],
    targets: &[SimulationTarget],
    passes: &[EstimationPass],
    variogram_model: &VariogramModel,
    random_seeds: &[u64],
    options: &SequentialGaussianSimulationOptions,
) -> Result<SequentialSimulationEnsemble, MineError> {
    let realization_method = "sgs-prototype-simple-kriging";
    let simple_kriging = SimpleKrigingOptions::new(options.known_mean())?;

    generate_sequential_ensemble(
        ensemble_id,
        base_model_id,
        column_id,
        grid_artifact_id,
        conditioning_artifact_ids,
        conditioning_samples,
        targets,
        passes,
        variogram_model,
        random_seeds,
        realization_method,
        |estimate, rng| estimate.estimate + rng.sample_standard_normal() * kriging_stddev(estimate),
        |metadata| {
            metadata.insert("known_mean", MetadataValue::Float(options.known_mean()))?;
            Ok(())
        },
        validate_conditioning_samples,
        0.0,
        |samples, _, _| Ok(samples.to_vec()),
        &simple_kriging,
    )
}

/// Genera un ensemble pequeño de SIS prototipo usando simple kriging sobre indicadores y seeds explícitos.
#[allow(clippy::too_many_arguments)]
pub fn generate_sequential_indicator_ensemble(
    ensemble_id: ArtifactId,
    base_model_id: ModelId,
    column_id: ColumnId,
    grid_artifact_id: ArtifactId,
    conditioning_artifact_ids: Vec<ArtifactId>,
    conditioning_samples: &[SpatialSample],
    targets: &[SimulationTarget],
    passes: &[EstimationPass],
    variogram_model: &VariogramModel,
    random_seeds: &[u64],
    options: &SequentialIndicatorSimulationOptions,
) -> Result<SequentialSimulationEnsemble, MineError> {
    let realization_method = "sis-prototype-simple-kriging";
    let indicator_mean = build_indicator_mean(conditioning_samples, &column_id, options.cutoff())?;
    let simple_kriging = SimpleKrigingOptions::new(indicator_mean)?;

    generate_sequential_ensemble(
        ensemble_id,
        base_model_id,
        column_id,
        grid_artifact_id,
        conditioning_artifact_ids,
        conditioning_samples,
        targets,
        passes,
        variogram_model,
        random_seeds,
        realization_method,
        |estimate, rng| {
            let probability = estimate.estimate.clamp(0.0, 1.0);
            if rng.next_unit_f64() <= probability {
                1.0
            } else {
                0.0
            }
        },
        |metadata| {
            metadata.insert("cutoff", MetadataValue::Float(options.cutoff()))?;
            metadata.insert("indicator_mean", MetadataValue::Float(indicator_mean))?;
            Ok(())
        },
        validate_conditioning_samples,
        options.cutoff(),
        build_indicator_samples,
        &simple_kriging,
    )
}

#[allow(clippy::too_many_arguments)]
fn generate_sequential_ensemble<SimulateValue, AddMethodMetadata, PrepareSamples>(
    ensemble_id: ArtifactId,
    base_model_id: ModelId,
    column_id: ColumnId,
    grid_artifact_id: ArtifactId,
    conditioning_artifact_ids: Vec<ArtifactId>,
    conditioning_samples: &[SpatialSample],
    targets: &[SimulationTarget],
    passes: &[EstimationPass],
    variogram_model: &VariogramModel,
    random_seeds: &[u64],
    realization_method: &str,
    simulate_value: SimulateValue,
    add_method_metadata: AddMethodMetadata,
    validate_samples: fn(&[SpatialSample], &ColumnId) -> Result<(), MineError>,
    sample_transform_parameter: f64,
    prepare_conditioning_samples: PrepareSamples,
    simple_kriging: &SimpleKrigingOptions,
) -> Result<SequentialSimulationEnsemble, MineError>
where
    SimulateValue: Fn(&KrigingEstimate, &mut DeterministicRng) -> f64,
    AddMethodMetadata: Fn(&mut Metadata) -> Result<(), MineError>,
    PrepareSamples: Fn(&[SpatialSample], &ColumnId, f64) -> Result<Vec<SpatialSample>, MineError>,
{
    validate_targets(targets)?;
    validate_passes(passes)?;
    validate_random_seeds(random_seeds)?;
    validate_samples(conditioning_samples, &column_id)?;

    let support = RealizationSupport::new(targets.len(), true, grid_artifact_id)?;
    let lineage = ConditionalRealizationLineage::new(conditioning_artifact_ids, None)?;
    let sampled_columns = vec![column_id.clone()];

    let transformed_conditioning_samples =
        prepare_conditioning_samples(conditioning_samples, &column_id, sample_transform_parameter)?;

    let mut descriptors = Vec::with_capacity(random_seeds.len());
    let mut realizations = Vec::with_capacity(random_seeds.len());
    for (realization_index, random_seed) in random_seeds.iter().copied().enumerate() {
        let realization = simulate_single_realization(
            realization_index,
            random_seed,
            &base_model_id,
            &column_id,
            targets,
            passes,
            variogram_model,
            &support,
            &lineage,
            realization_method,
            &transformed_conditioning_samples,
            &simulate_value,
            &add_method_metadata,
            simple_kriging,
        )?;
        descriptors.push(realization.descriptor.clone());
        realizations.push(realization);
    }

    let mut ensemble_metadata = Metadata::new();
    ensemble_metadata.insert(
        "simulation_method",
        MetadataValue::Text(realization_method.to_owned()),
    )?;
    ensemble_metadata.insert(
        "random_seed_count",
        MetadataValue::Integer(i64::try_from(random_seeds.len()).map_err(|_| {
            MineError::validation("random seed count exceeds supported metadata range")
        })?),
    )?;

    let descriptor = ConditionalRealizationSet::new(
        ensemble_id,
        base_model_id,
        sampled_columns,
        support,
        descriptors,
        ensemble_metadata,
    )?;

    Ok(SequentialSimulationEnsemble {
        descriptor,
        realizations,
    })
}

#[allow(clippy::too_many_arguments)]
fn simulate_single_realization<SimulateValue, AddMethodMetadata>(
    realization_index: usize,
    random_seed: u64,
    base_model_id: &ModelId,
    column_id: &ColumnId,
    targets: &[SimulationTarget],
    passes: &[EstimationPass],
    variogram_model: &VariogramModel,
    support: &RealizationSupport,
    lineage: &ConditionalRealizationLineage,
    realization_method: &str,
    conditioning_samples: &[SpatialSample],
    simulate_value: &SimulateValue,
    add_method_metadata: &AddMethodMetadata,
    simple_kriging: &SimpleKrigingOptions,
) -> Result<SequentialSimulationRealization, MineError>
where
    SimulateValue: Fn(&KrigingEstimate, &mut DeterministicRng) -> f64,
    AddMethodMetadata: Fn(&mut Metadata) -> Result<(), MineError>,
{
    let mut rng = DeterministicRng::new(random_seed);
    let order = shuffled_indices(targets.len(), &mut rng);
    let mut working_samples = conditioning_samples.to_vec();
    let mut realized_values = vec![0.0; targets.len()];

    for target_index in order {
        let target = &targets[target_index];
        let estimate = estimate_simple_kriging(
            target.location,
            &working_samples,
            column_id,
            passes,
            variogram_model,
            simple_kriging,
        )?;
        let simulated_value = simulate_value(&estimate, &mut rng);
        realized_values[target_index] = simulated_value;
        working_samples.push(build_simulated_sample(
            random_seed,
            target,
            column_id,
            simulated_value,
        )?);
    }

    let values = targets
        .iter()
        .zip(realized_values.iter().copied())
        .map(|(target, value)| SimulatedNodeValue {
            target_id: target.target_id.clone(),
            location: target.location,
            domain: target.domain.clone(),
            value,
        })
        .collect::<Vec<_>>();
    let summary = build_summary(&values)?;
    let descriptor = build_realization_descriptor(
        realization_index,
        random_seed,
        base_model_id,
        column_id,
        support,
        lineage,
        realization_method,
        &summary,
        add_method_metadata,
    )?;

    Ok(SequentialSimulationRealization {
        descriptor,
        values,
        summary,
    })
}

fn build_summary(values: &[SimulatedNodeValue]) -> Result<SequentialSimulationSummary, MineError> {
    if values.is_empty() {
        return Err(MineError::invalid_parameter(
            "values",
            "simulation realization must contain at least one node",
        ));
    }

    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;
    let mut sum = 0.0;
    for value in values {
        min_value = min_value.min(value.value);
        max_value = max_value.max(value.value);
        sum += value.value;
    }

    Ok(SequentialSimulationSummary {
        node_count: values.len(),
        min_value,
        max_value,
        mean_value: sum / values.len() as f64,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_realization_descriptor<AddMethodMetadata>(
    realization_index: usize,
    random_seed: u64,
    base_model_id: &ModelId,
    column_id: &ColumnId,
    support: &RealizationSupport,
    lineage: &ConditionalRealizationLineage,
    realization_method: &str,
    summary: &SequentialSimulationSummary,
    add_method_metadata: &AddMethodMetadata,
) -> Result<ConditionalRealization, MineError>
where
    AddMethodMetadata: Fn(&mut Metadata) -> Result<(), MineError>,
{
    let realization_id = ArtifactId::new(format!(
        "{realization_method}:realization:{realization_index:03}"
    ))?;
    let storage_artifact_id = ArtifactId::new(format!(
        "{realization_method}:storage:{realization_index:03}"
    ))?;
    let mut metadata = Metadata::new();
    metadata.insert(
        "node_count",
        MetadataValue::Integer(summary.node_count as i64),
    )?;
    metadata.insert("min_value", MetadataValue::Float(summary.min_value))?;
    metadata.insert("max_value", MetadataValue::Float(summary.max_value))?;
    metadata.insert("mean_value", MetadataValue::Float(summary.mean_value))?;
    add_method_metadata(&mut metadata)?;

    ConditionalRealization::new(
        realization_id,
        realization_index,
        base_model_id.clone(),
        vec![column_id.clone()],
        storage_artifact_id,
        RealizationStorageFormat::Json,
        realization_method,
        random_seed,
        support.clone(),
        lineage.clone(),
        metadata,
    )
}

fn build_simulated_sample(
    random_seed: u64,
    target: &SimulationTarget,
    column_id: &ColumnId,
    value: f64,
) -> Result<SpatialSample, MineError> {
    SpatialSample::new(
        format!("sim-{random_seed}-{}", target.target_id),
        target.location,
        target.domain.clone(),
        BTreeMap::from([(column_id.clone(), value)]),
    )
}

fn validate_targets(targets: &[SimulationTarget]) -> Result<(), MineError> {
    if targets.is_empty() {
        return Err(MineError::invalid_parameter(
            "targets",
            "sequential simulation requires at least one target node",
        ));
    }
    let mut seen_ids = BTreeSet::new();
    for target in targets {
        if !seen_ids.insert(target.target_id.clone()) {
            return Err(MineError::validation(format!(
                "duplicate simulation target id `{}`",
                target.target_id
            )));
        }
    }
    Ok(())
}

fn validate_passes(passes: &[EstimationPass]) -> Result<(), MineError> {
    if passes.is_empty() {
        return Err(MineError::invalid_parameter(
            "passes",
            "sequential simulation requires at least one estimation pass",
        ));
    }
    Ok(())
}

fn validate_random_seeds(random_seeds: &[u64]) -> Result<(), MineError> {
    if random_seeds.is_empty() {
        return Err(MineError::invalid_parameter(
            "random_seeds",
            "sequential simulation requires at least one explicit random seed",
        ));
    }
    Ok(())
}

fn validate_conditioning_samples(
    conditioning_samples: &[SpatialSample],
    column_id: &ColumnId,
) -> Result<(), MineError> {
    if conditioning_samples.is_empty() {
        return Err(MineError::invalid_parameter(
            "conditioning_samples",
            "sequential simulation requires at least one conditioning sample",
        ));
    }
    for sample in conditioning_samples {
        if !sample.values.contains_key(column_id) {
            return Err(MineError::validation(format!(
                "conditioning sample `{}` does not contain column `{column_id}`",
                sample.sample_id
            )));
        }
    }
    Ok(())
}

fn build_indicator_mean(
    conditioning_samples: &[SpatialSample],
    column_id: &ColumnId,
    cutoff: f64,
) -> Result<f64, MineError> {
    validate_conditioning_samples(conditioning_samples, column_id)?;

    let indicator_sum = conditioning_samples
        .iter()
        .map(|sample| {
            if sample.values[column_id] >= cutoff {
                1.0
            } else {
                0.0
            }
        })
        .sum::<f64>();

    Ok(indicator_sum / conditioning_samples.len() as f64)
}

fn build_indicator_samples(
    conditioning_samples: &[SpatialSample],
    column_id: &ColumnId,
    cutoff: f64,
) -> Result<Vec<SpatialSample>, MineError> {
    validate_conditioning_samples(conditioning_samples, column_id)?;

    conditioning_samples
        .iter()
        .map(|sample| {
            SpatialSample::new(
                sample.sample_id.clone(),
                sample.location,
                sample.domain.clone(),
                BTreeMap::from([(
                    column_id.clone(),
                    if sample.values[column_id] >= cutoff {
                        1.0
                    } else {
                        0.0
                    },
                )]),
            )
        })
        .collect()
}

fn shuffled_indices(size: usize, rng: &mut DeterministicRng) -> Vec<usize> {
    let mut indices = (0..size).collect::<Vec<_>>();
    for index in (1..indices.len()).rev() {
        let selected = rng.next_bounded_usize(index + 1);
        indices.swap(index, selected);
    }
    indices
}

fn kriging_stddev(estimate: &KrigingEstimate) -> f64 {
    estimate.kriging_variance.max(0.0).sqrt()
}

#[derive(Debug, Clone)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_unit_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        bits as f64 / ((1u64 << 53) as f64)
    }

    fn next_bounded_usize(&mut self, upper_exclusive: usize) -> usize {
        (self.next_u64() % upper_exclusive as u64) as usize
    }

    fn sample_standard_normal(&mut self) -> f64 {
        let u1 = (1.0 - self.next_unit_f64()).max(f64::MIN_POSITIVE);
        let u2 = self.next_unit_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}
