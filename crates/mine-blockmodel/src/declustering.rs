use std::collections::{BTreeMap, BTreeSet};

use mine_core::{BlockDimensions, ColumnId, Coordinate3D, MineError};
use serde::{Deserialize, Serialize};

/// Sample espacial mínimo para workflows de declustering, variografía y estimación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialSample {
    /// Identificador estable del sample.
    pub sample_id: String,
    /// Coordenada puntual usada para agrupar y ponderar.
    pub location: Coordinate3D,
    /// Dominio opcional del sample.
    pub domain: Option<String>,
    /// Variables numéricas asociadas al sample.
    pub values: BTreeMap<ColumnId, f64>,
}

impl SpatialSample {
    /// Construye un sample espacial validando identificador, dominio y valores numéricos.
    pub fn new(
        sample_id: impl Into<String>,
        location: Coordinate3D,
        domain: Option<String>,
        values: BTreeMap<ColumnId, f64>,
    ) -> Result<Self, MineError> {
        let sample_id = sample_id.into();
        if sample_id.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "sample_id",
                "spatial sample id must not be empty",
            ));
        }
        if let Some(domain) = &domain
            && domain.trim().is_empty()
        {
            return Err(MineError::invalid_parameter(
                "domain",
                "spatial sample domain must not be empty when provided",
            ));
        }
        for (column_id, value) in &values {
            if !value.is_finite() {
                return Err(MineError::invalid_parameter(
                    "values",
                    format!("spatial sample value for `{column_id}` must be finite"),
                ));
            }
        }

        Ok(Self {
            sample_id,
            location,
            domain,
            values,
        })
    }
}

/// Offset fraccional del origen de la grilla de declustering respecto del tamaño de celda.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CellOriginOffset {
    x_fraction: f64,
    y_fraction: f64,
    z_fraction: f64,
}

impl CellOriginOffset {
    /// Construye un offset fraccional validado dentro del rango `[0, 1)`.
    pub fn new(x_fraction: f64, y_fraction: f64, z_fraction: f64) -> Result<Self, MineError> {
        validate_origin_fraction("x_fraction", x_fraction)?;
        validate_origin_fraction("y_fraction", y_fraction)?;
        validate_origin_fraction("z_fraction", z_fraction)?;

        Ok(Self {
            x_fraction,
            y_fraction,
            z_fraction,
        })
    }

    /// Fracción del tamaño de celda aplicada en `x`.
    #[must_use]
    pub const fn x_fraction(&self) -> f64 {
        self.x_fraction
    }

    /// Fracción del tamaño de celda aplicada en `y`.
    #[must_use]
    pub const fn y_fraction(&self) -> f64 {
        self.y_fraction
    }

    /// Fracción del tamaño de celda aplicada en `z`.
    #[must_use]
    pub const fn z_fraction(&self) -> f64 {
        self.z_fraction
    }
}

/// Configuración explícita del cell declustering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellDeclusteringOptions {
    cell_size: BlockDimensions,
    origins: Vec<CellOriginOffset>,
}

impl CellDeclusteringOptions {
    /// Construye opciones de declustering validando tamaño de celda y orígenes.
    pub fn new(
        cell_size: BlockDimensions,
        origins: Vec<CellOriginOffset>,
    ) -> Result<Self, MineError> {
        if origins.is_empty() {
            return Err(MineError::invalid_parameter(
                "origins",
                "cell declustering requires at least one origin offset",
            ));
        }

        Ok(Self { cell_size, origins })
    }

    /// Tamaño de celda usado para agrupar samples.
    #[must_use]
    pub const fn cell_size(&self) -> BlockDimensions {
        self.cell_size
    }

    /// Orígenes fraccionales usados en el sweep multi-origin.
    #[must_use]
    pub fn origins(&self) -> &[CellOriginOffset] {
        &self.origins
    }
}

/// Peso serializable de un sample luego del sweep multi-origin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclusteredSampleWeight {
    /// Sample al que pertenece el peso.
    pub sample_id: String,
    /// Peso promedio no normalizado a través de todos los orígenes.
    pub average_cell_weight: f64,
    /// Peso final normalizado para estadísticas e histogramas.
    pub normalized_weight: f64,
}

/// Resultado serializable del cell declustering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellDeclusteringResult {
    /// Configuración aplicada.
    pub options: CellDeclusteringOptions,
    /// Cantidad de celdas ocupadas observadas por origen.
    pub occupied_cell_counts: Vec<usize>,
    /// Factor usado para normalizar los pesos finales.
    pub normalization_factor: f64,
    /// Pesos por sample en el mismo orden de entrada.
    pub sample_weights: Vec<DeclusteredSampleWeight>,
}

/// Resumen ponderado de una variable numérica.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedVariableSummary {
    /// Variable resumida.
    pub column_id: ColumnId,
    /// Peso total acumulado en la selección.
    pub total_weight: f64,
    /// Media ponderada cuando el denominador es positivo.
    pub weighted_mean: Option<f64>,
    /// Valor mínimo observado.
    pub minimum: f64,
    /// Valor máximo observado.
    pub maximum: f64,
}

/// Resumen ponderado de un subconjunto de samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedSampleStatistics {
    /// Cantidad de samples incluidos en la selección.
    pub sample_count: usize,
    /// Peso total de la selección.
    pub total_weight: f64,
    /// Resúmenes por variable.
    pub variables: Vec<WeightedVariableSummary>,
}

/// Resumen ponderado por dominio explícito.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainWeightedStatistics {
    /// Dominio auditado.
    pub domain: String,
    /// Cantidad de samples del dominio.
    pub sample_count: usize,
    /// Peso total del dominio.
    pub total_weight: f64,
    /// Resúmenes por variable.
    pub variables: Vec<WeightedVariableSummary>,
}

/// Reporte serializable de estadísticas ponderadas overall y por dominio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedStatisticsReport {
    /// Resumen ponderado overall.
    pub overall: WeightedSampleStatistics,
    /// Resúmenes ponderados por dominio etiquetado.
    pub domains: Vec<DomainWeightedStatistics>,
    /// Samples sin dominio explícito que quedaron fuera del grouping por dominio.
    pub untagged_sample_ids: Vec<String>,
}

/// Bin ponderado de un histograma numérico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedHistogramBin {
    /// Límite inferior del bin.
    pub from: f64,
    /// Límite superior del bin.
    pub to: f64,
    /// Cantidad de samples que cayeron en el bin.
    pub sample_count: usize,
    /// Peso acumulado dentro del bin.
    pub total_weight: f64,
}

/// Histograma ponderado serializable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedHistogram {
    /// Variable usada para el histograma.
    pub column_id: ColumnId,
    /// Dominio filtrado; `None` indica histograma overall.
    pub domain: Option<String>,
    /// Peso total considerado en la selección.
    pub total_weight: f64,
    /// Peso por debajo del primer límite.
    pub underflow_weight: f64,
    /// Peso por encima del último límite.
    pub overflow_weight: f64,
    /// Bins explícitos del histograma.
    pub bins: Vec<WeightedHistogramBin>,
}

/// Calcula pesos de cell declustering promediando múltiples orígenes.
pub fn compute_cell_declustering_weights(
    samples: &[SpatialSample],
    options: &CellDeclusteringOptions,
) -> Result<CellDeclusteringResult, MineError> {
    if samples.is_empty() {
        return Err(MineError::invalid_parameter(
            "samples",
            "cell declustering requires at least one spatial sample",
        ));
    }

    let sample_ids = validate_spatial_samples(samples)?;
    let mut accumulated_weights = vec![0.0; samples.len()];
    let mut occupied_cell_counts = Vec::with_capacity(options.origins.len());

    for origin in options.origins() {
        let mut cell_counts = BTreeMap::<(i64, i64, i64), usize>::new();
        let mut sample_cells = Vec::with_capacity(samples.len());

        for sample in samples {
            let cell = spatial_cell_key(sample.location, options.cell_size(), origin)?;
            *cell_counts.entry(cell).or_insert(0) += 1;
            sample_cells.push(cell);
        }

        occupied_cell_counts.push(cell_counts.len());

        for (index, cell) in sample_cells.into_iter().enumerate() {
            let population = *cell_counts
                .get(&cell)
                .expect("cell count should exist for every sample cell");
            accumulated_weights[index] += 1.0 / population as f64;
        }
    }

    let origin_count = options.origins().len() as f64;
    for weight in &mut accumulated_weights {
        *weight /= origin_count;
    }

    let normalization_factor = accumulated_weights.iter().sum::<f64>();
    if normalization_factor <= 0.0 || !normalization_factor.is_finite() {
        return Err(MineError::numeric(
            "cell declustering produced a non-positive normalization factor",
        ));
    }

    let sample_weights = sample_ids
        .into_iter()
        .zip(accumulated_weights)
        .map(|(sample_id, average_cell_weight)| DeclusteredSampleWeight {
            sample_id,
            average_cell_weight,
            normalized_weight: average_cell_weight / normalization_factor,
        })
        .collect();

    Ok(CellDeclusteringResult {
        options: options.clone(),
        occupied_cell_counts,
        normalization_factor,
        sample_weights,
    })
}

/// Construye estadísticas ponderadas overall y por dominio a partir de pesos explícitos.
pub fn build_weighted_statistics_report(
    samples: &[SpatialSample],
    weights: &[DeclusteredSampleWeight],
) -> Result<WeightedStatisticsReport, MineError> {
    if samples.is_empty() {
        return Err(MineError::invalid_parameter(
            "samples",
            "weighted statistics require at least one spatial sample",
        ));
    }

    let weight_map = build_weight_map(samples, weights)?;
    let variable_keys = validate_value_columns(samples)?;
    let overall_samples = samples.iter().collect::<Vec<_>>();
    let overall = summarize_sample_group(&overall_samples, &weight_map, &variable_keys)?;
    let mut domain_groups = BTreeMap::<String, Vec<&SpatialSample>>::new();
    let mut untagged_sample_ids = Vec::new();

    for sample in samples {
        match &sample.domain {
            Some(domain) => domain_groups
                .entry(domain.clone())
                .or_default()
                .push(sample),
            None => untagged_sample_ids.push(sample.sample_id.clone()),
        }
    }

    let mut domains = Vec::with_capacity(domain_groups.len());
    for (domain, domain_samples) in domain_groups {
        let summary = summarize_sample_group(&domain_samples, &weight_map, &variable_keys)?;
        domains.push(DomainWeightedStatistics {
            domain,
            sample_count: summary.sample_count,
            total_weight: summary.total_weight,
            variables: summary.variables,
        });
    }

    Ok(WeightedStatisticsReport {
        overall,
        domains,
        untagged_sample_ids,
    })
}

/// Construye un histograma ponderado overall o restringido a un dominio.
pub fn build_weighted_histogram(
    samples: &[SpatialSample],
    weights: &[DeclusteredSampleWeight],
    column_id: &ColumnId,
    boundaries: &[f64],
    domain: Option<&str>,
) -> Result<WeightedHistogram, MineError> {
    if samples.is_empty() {
        return Err(MineError::invalid_parameter(
            "samples",
            "weighted histogram requires at least one spatial sample",
        ));
    }

    let weight_map = build_weight_map(samples, weights)?;
    let boundaries = normalize_histogram_boundaries(boundaries)?;
    let selected_samples = samples
        .iter()
        .filter(|sample| match domain {
            Some(domain) => sample.domain.as_deref() == Some(domain),
            None => true,
        })
        .collect::<Vec<_>>();

    if selected_samples.is_empty() {
        return Err(MineError::validation(
            "weighted histogram selection is empty for the requested domain",
        ));
    }

    let mut bins = boundaries
        .windows(2)
        .map(|window| WeightedHistogramBin {
            from: window[0],
            to: window[1],
            sample_count: 0,
            total_weight: 0.0,
        })
        .collect::<Vec<_>>();
    let mut underflow_weight = 0.0;
    let mut overflow_weight = 0.0;
    let mut total_weight = 0.0;

    for sample in selected_samples {
        let value = *sample.values.get(column_id).ok_or_else(|| {
            MineError::schema(format!(
                "spatial sample `{}` is missing value column `{column_id}`",
                sample.sample_id
            ))
        })?;
        let weight = *weight_map
            .get(sample.sample_id.as_str())
            .expect("weight map should contain every sample");
        total_weight += weight;

        if value < boundaries[0] {
            underflow_weight += weight;
            continue;
        }
        if value > *boundaries.last().expect("boundaries should not be empty") {
            overflow_weight += weight;
            continue;
        }

        let mut assigned = false;
        let bin_count = bins.len();
        for (index, bin) in bins.iter_mut().enumerate() {
            let is_last_bin = index + 1 == bin_count;
            if (value >= bin.from && value < bin.to) || (is_last_bin && value == bin.to) {
                bin.sample_count += 1;
                bin.total_weight += weight;
                assigned = true;
                break;
            }
        }

        if !assigned {
            overflow_weight += weight;
        }
    }

    Ok(WeightedHistogram {
        column_id: column_id.clone(),
        domain: domain.map(ToOwned::to_owned),
        total_weight,
        underflow_weight,
        overflow_weight,
        bins,
    })
}

fn validate_spatial_samples(samples: &[SpatialSample]) -> Result<Vec<String>, MineError> {
    let mut sample_ids = Vec::with_capacity(samples.len());
    let mut seen = BTreeSet::new();

    for sample in samples {
        if !seen.insert(sample.sample_id.clone()) {
            return Err(MineError::validation(format!(
                "duplicate spatial sample id `{}` found while declustering",
                sample.sample_id
            )));
        }
        sample_ids.push(sample.sample_id.clone());
    }

    Ok(sample_ids)
}

fn spatial_cell_key(
    location: Coordinate3D,
    cell_size: BlockDimensions,
    origin: &CellOriginOffset,
) -> Result<(i64, i64, i64), MineError> {
    let x = floor_to_i64(
        (location.x() - origin.x_fraction() * cell_size.dx()) / cell_size.dx(),
        "x",
    )?;
    let y = floor_to_i64(
        (location.y() - origin.y_fraction() * cell_size.dy()) / cell_size.dy(),
        "y",
    )?;
    let z = floor_to_i64(
        (location.z() - origin.z_fraction() * cell_size.dz()) / cell_size.dz(),
        "z",
    )?;
    Ok((x, y, z))
}

fn floor_to_i64(value: f64, axis: &str) -> Result<i64, MineError> {
    if !value.is_finite() {
        return Err(MineError::numeric(format!(
            "declustering cell index for `{axis}` must be finite"
        )));
    }
    let floored = value.floor();
    if floored < i64::MIN as f64 || floored > i64::MAX as f64 {
        return Err(MineError::numeric(format!(
            "declustering cell index for `{axis}` is outside i64 range"
        )));
    }
    Ok(floored as i64)
}

fn build_weight_map<'a>(
    samples: &[SpatialSample],
    weights: &'a [DeclusteredSampleWeight],
) -> Result<BTreeMap<&'a str, f64>, MineError> {
    let sample_ids = samples
        .iter()
        .map(|sample| sample.sample_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut weight_map = BTreeMap::new();

    for weight in weights {
        if !sample_ids.contains(weight.sample_id.as_str()) {
            return Err(MineError::validation(format!(
                "weight references unknown spatial sample `{}`",
                weight.sample_id
            )));
        }
        if !weight.normalized_weight.is_finite() || weight.normalized_weight < 0.0 {
            return Err(MineError::invalid_parameter(
                "weights",
                "normalized sample weights must be finite and greater than or equal to zero",
            ));
        }
        if weight_map
            .insert(weight.sample_id.as_str(), weight.normalized_weight)
            .is_some()
        {
            return Err(MineError::validation(format!(
                "duplicate weight found for spatial sample `{}`",
                weight.sample_id
            )));
        }
    }

    for sample in samples {
        if !weight_map.contains_key(sample.sample_id.as_str()) {
            return Err(MineError::validation(format!(
                "missing weight for spatial sample `{}`",
                sample.sample_id
            )));
        }
    }

    Ok(weight_map)
}

fn validate_value_columns(samples: &[SpatialSample]) -> Result<Vec<ColumnId>, MineError> {
    let variable_keys = samples[0].values.keys().cloned().collect::<Vec<_>>();
    for sample in &samples[1..] {
        let sample_keys = sample.values.keys().cloned().collect::<Vec<_>>();
        if sample_keys != variable_keys {
            return Err(MineError::schema(
                "all spatial samples must expose the same numeric value columns",
            ));
        }
    }
    Ok(variable_keys)
}

fn summarize_sample_group(
    samples: &[&SpatialSample],
    weight_map: &BTreeMap<&str, f64>,
    variable_keys: &[ColumnId],
) -> Result<WeightedSampleStatistics, MineError> {
    let sample_count = samples.len();
    let total_weight = samples
        .iter()
        .map(|sample| {
            *weight_map
                .get(sample.sample_id.as_str())
                .expect("weight map should contain every sample")
        })
        .sum::<f64>();
    let mut variables = Vec::with_capacity(variable_keys.len());

    for column_id in variable_keys {
        let mut weighted_sum = 0.0;
        let mut minimum = None::<f64>;
        let mut maximum = None::<f64>;

        for sample in samples {
            let value = *sample.values.get(column_id).ok_or_else(|| {
                MineError::schema(format!(
                    "spatial sample `{}` is missing value column `{column_id}`",
                    sample.sample_id
                ))
            })?;
            let weight = *weight_map
                .get(sample.sample_id.as_str())
                .expect("weight map should contain every sample");
            weighted_sum += value * weight;
            minimum = Some(match minimum {
                Some(current) => current.min(value),
                None => value,
            });
            maximum = Some(match maximum {
                Some(current) => current.max(value),
                None => value,
            });
        }

        variables.push(WeightedVariableSummary {
            column_id: column_id.clone(),
            total_weight,
            weighted_mean: (total_weight > 0.0).then_some(weighted_sum / total_weight),
            minimum: minimum.expect("sample group should not be empty"),
            maximum: maximum.expect("sample group should not be empty"),
        });
    }

    Ok(WeightedSampleStatistics {
        sample_count,
        total_weight,
        variables,
    })
}

fn normalize_histogram_boundaries(boundaries: &[f64]) -> Result<Vec<f64>, MineError> {
    if boundaries.iter().any(|boundary| !boundary.is_finite()) {
        return Err(MineError::invalid_parameter(
            "boundaries",
            "histogram boundaries must be finite numeric values",
        ));
    }

    let mut normalized = boundaries.to_vec();
    normalized.sort_by(f64::total_cmp);
    normalized.dedup_by(|left, right| left.total_cmp(right).is_eq());
    if normalized.len() < 2 {
        return Err(MineError::invalid_parameter(
            "boundaries",
            "weighted histogram requires at least two distinct boundaries",
        ));
    }

    Ok(normalized)
}

fn validate_origin_fraction(name: &str, value: f64) -> Result<(), MineError> {
    if !value.is_finite() || !(0.0..1.0).contains(&value) {
        return Err(MineError::invalid_parameter(
            "origin_fraction",
            format!("{name} must be finite and within [0, 1)"),
        ));
    }
    Ok(())
}
