use mine_core::{ColumnId, Coordinate3D, MineError};
use serde::{Deserialize, Serialize};

use crate::declustering::SpatialSample;

/// Configuración explícita del lagging para un variograma experimental.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariogramLagConfig {
    lag_size: f64,
    lag_count: usize,
    lag_tolerance: f64,
}

impl VariogramLagConfig {
    /// Construye una configuración validada de lagging.
    pub fn new(lag_size: f64, lag_count: usize, lag_tolerance: f64) -> Result<Self, MineError> {
        if !lag_size.is_finite() || lag_size <= 0.0 {
            return Err(MineError::invalid_parameter(
                "lag_size",
                "variogram lag size must be finite and greater than zero",
            ));
        }
        if lag_count == 0 {
            return Err(MineError::invalid_parameter(
                "lag_count",
                "variogram lag count must be greater than zero",
            ));
        }
        if !lag_tolerance.is_finite() || lag_tolerance < 0.0 {
            return Err(MineError::invalid_parameter(
                "lag_tolerance",
                "variogram lag tolerance must be finite and greater than or equal to zero",
            ));
        }

        Ok(Self {
            lag_size,
            lag_count,
            lag_tolerance,
        })
    }

    /// Tamaño del lag.
    #[must_use]
    pub const fn lag_size(&self) -> f64 {
        self.lag_size
    }

    /// Cantidad de lags.
    #[must_use]
    pub const fn lag_count(&self) -> usize {
        self.lag_count
    }

    /// Tolerancia radial permitida alrededor del centro del lag.
    #[must_use]
    pub const fn lag_tolerance(&self) -> f64 {
        self.lag_tolerance
    }
}

/// Restricción direccional explícita para variografía experimental.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariogramDirection {
    vector: Coordinate3D,
    angular_tolerance_degrees: f64,
    bandwidth: Option<f64>,
}

impl VariogramDirection {
    /// Construye una restricción direccional validando vector, tolerancia angular y bandwidth.
    pub fn new(
        vector: Coordinate3D,
        angular_tolerance_degrees: f64,
        bandwidth: Option<f64>,
    ) -> Result<Self, MineError> {
        let norm =
            (vector.x() * vector.x() + vector.y() * vector.y() + vector.z() * vector.z()).sqrt();
        if norm <= 0.0 {
            return Err(MineError::invalid_parameter(
                "vector",
                "variogram direction vector must have non-zero length",
            ));
        }
        if !angular_tolerance_degrees.is_finite()
            || !(0.0..=90.0).contains(&angular_tolerance_degrees)
        {
            return Err(MineError::invalid_parameter(
                "angular_tolerance_degrees",
                "variogram angular tolerance must be finite and within [0, 90]",
            ));
        }
        if let Some(bandwidth) = bandwidth
            && (!bandwidth.is_finite() || bandwidth < 0.0)
        {
            return Err(MineError::invalid_parameter(
                "bandwidth",
                "variogram bandwidth must be finite and greater than or equal to zero",
            ));
        }

        Ok(Self {
            vector,
            angular_tolerance_degrees,
            bandwidth,
        })
    }

    /// Vector bruto de la dirección.
    #[must_use]
    pub const fn vector(&self) -> Coordinate3D {
        self.vector
    }

    /// Tolerancia angular en grados.
    #[must_use]
    pub const fn angular_tolerance_degrees(&self) -> f64 {
        self.angular_tolerance_degrees
    }

    /// Bandwidth opcional perpendicular a la dirección.
    #[must_use]
    pub const fn bandwidth(&self) -> Option<f64> {
        self.bandwidth
    }
}

/// Bin serializable de un variograma experimental.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentalVariogramLag {
    /// Índice basado en 1 del lag.
    pub lag_index: usize,
    /// Distancia objetivo del lag.
    pub lag_center: f64,
    /// Cantidad de pares asignados.
    pub pair_count: usize,
    /// Distancia media observada en los pares del lag.
    pub average_distance: Option<f64>,
    /// Semivarianza experimental del lag.
    pub semivariance: Option<f64>,
}

/// Artefacto serializable de variografía experimental.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentalVariogram {
    /// Variable analizada.
    pub column_id: ColumnId,
    /// Dominio filtrado; `None` indica selección global.
    pub domain: Option<String>,
    /// Restricción direccional aplicada; `None` indica variograma omni.
    pub direction: Option<VariogramDirection>,
    /// Configuración de lagging usada.
    pub lag_config: VariogramLagConfig,
    /// Cantidad de samples considerados.
    pub sample_count: usize,
    /// Lags resultantes.
    pub lags: Vec<ExperimentalVariogramLag>,
}

impl ExperimentalVariogram {
    /// Materializa el variograma como filas planas por lag para IO tabular.
    #[must_use]
    pub fn lag_rows(&self) -> Vec<ExperimentalVariogramLagRow> {
        self.lags
            .iter()
            .map(|lag| ExperimentalVariogramLagRow {
                column_id: self.column_id.clone(),
                domain: self.domain.clone(),
                direction_x: self
                    .direction
                    .as_ref()
                    .map(|direction| direction.vector().x()),
                direction_y: self
                    .direction
                    .as_ref()
                    .map(|direction| direction.vector().y()),
                direction_z: self
                    .direction
                    .as_ref()
                    .map(|direction| direction.vector().z()),
                angular_tolerance_degrees: self
                    .direction
                    .as_ref()
                    .map(VariogramDirection::angular_tolerance_degrees),
                bandwidth: self
                    .direction
                    .as_ref()
                    .and_then(VariogramDirection::bandwidth),
                lag_size: self.lag_config.lag_size(),
                lag_count: self.lag_config.lag_count(),
                lag_tolerance: self.lag_config.lag_tolerance(),
                sample_count: self.sample_count,
                lag_index: lag.lag_index,
                lag_center: lag.lag_center,
                pair_count: lag.pair_count,
                average_distance: lag.average_distance,
                semivariance: lag.semivariance,
            })
            .collect()
    }
}

/// Fila plana serializable de un variograma experimental por lag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentalVariogramLagRow {
    /// Variable analizada.
    pub column_id: ColumnId,
    /// Dominio filtrado.
    pub domain: Option<String>,
    /// Componente `x` de la dirección, cuando aplica.
    pub direction_x: Option<f64>,
    /// Componente `y` de la dirección, cuando aplica.
    pub direction_y: Option<f64>,
    /// Componente `z` de la dirección, cuando aplica.
    pub direction_z: Option<f64>,
    /// Tolerancia angular en grados, cuando aplica.
    pub angular_tolerance_degrees: Option<f64>,
    /// Bandwidth explícito, cuando aplica.
    pub bandwidth: Option<f64>,
    /// Tamaño base del lag.
    pub lag_size: f64,
    /// Cantidad total de lags declarados.
    pub lag_count: usize,
    /// Tolerancia radial del lag.
    pub lag_tolerance: f64,
    /// Cantidad de samples considerados.
    pub sample_count: usize,
    /// Índice del lag.
    pub lag_index: usize,
    /// Centro del lag.
    pub lag_center: f64,
    /// Cantidad de pares del lag.
    pub pair_count: usize,
    /// Distancia media del lag.
    pub average_distance: Option<f64>,
    /// Semivarianza del lag.
    pub semivariance: Option<f64>,
}

/// Modelos variográficos autorizados en esta etapa del SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariogramModelKind {
    /// Efecto pepita puro.
    Nugget,
    /// Modelo esférico con rango práctico explícito.
    Spherical,
    /// Modelo exponencial usando el rango práctico `a` en `1 - exp(-3h/a)`.
    Exponential,
    /// Modelo gaussiano usando el rango práctico `a` en `1 - exp(-3h^2/a^2)`.
    Gaussian,
}

impl VariogramModelKind {
    #[must_use]
    const fn requires_range(self) -> bool {
        !matches!(self, Self::Nugget)
    }
}

/// Opciones explícitas para el fitting determinista de un modelo variográfico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariogramFitOptions {
    include_nugget: bool,
    nugget_steps: usize,
    sill_steps: usize,
    range_steps: usize,
    max_range: Option<f64>,
}

impl VariogramFitOptions {
    /// Construye opciones validadas para el fitting por búsqueda en grilla.
    pub fn new(
        include_nugget: bool,
        nugget_steps: usize,
        sill_steps: usize,
        range_steps: usize,
        max_range: Option<f64>,
    ) -> Result<Self, MineError> {
        if nugget_steps == 0 {
            return Err(MineError::invalid_parameter(
                "nugget_steps",
                "variogram nugget_steps must be greater than zero",
            ));
        }
        if sill_steps == 0 {
            return Err(MineError::invalid_parameter(
                "sill_steps",
                "variogram sill_steps must be greater than zero",
            ));
        }
        if range_steps == 0 {
            return Err(MineError::invalid_parameter(
                "range_steps",
                "variogram range_steps must be greater than zero",
            ));
        }
        if let Some(max_range) = max_range
            && (!max_range.is_finite() || max_range <= 0.0)
        {
            return Err(MineError::invalid_parameter(
                "max_range",
                "variogram max_range must be finite and greater than zero",
            ));
        }

        Ok(Self {
            include_nugget,
            nugget_steps,
            sill_steps,
            range_steps,
            max_range,
        })
    }

    /// Indica si el ajuste debe explorar un término nugget explícito.
    #[must_use]
    pub const fn include_nugget(&self) -> bool {
        self.include_nugget
    }

    /// Cantidad de candidatos para nugget.
    #[must_use]
    pub const fn nugget_steps(&self) -> usize {
        self.nugget_steps
    }

    /// Cantidad de candidatos para sill parcial.
    #[must_use]
    pub const fn sill_steps(&self) -> usize {
        self.sill_steps
    }

    /// Cantidad de candidatos para rango.
    #[must_use]
    pub const fn range_steps(&self) -> usize {
        self.range_steps
    }

    /// Rango máximo opcional a explorar; `None` deriva un límite desde el variograma experimental.
    #[must_use]
    pub const fn max_range(&self) -> Option<f64> {
        self.max_range
    }
}

impl Default for VariogramFitOptions {
    fn default() -> Self {
        Self {
            include_nugget: true,
            nugget_steps: 11,
            sill_steps: 21,
            range_steps: 21,
            max_range: None,
        }
    }
}

/// Resumen serializable del ajuste variográfico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariogramFitSummary {
    /// Cantidad de lags con observación efectiva usados en el fitting.
    pub observed_lag_count: usize,
    /// Cantidad total de pares usados como peso.
    pub total_pair_count: usize,
    /// Suma de errores cuadrados ponderada por cantidad de pares.
    pub weighted_sse: f64,
    /// Error cuadrático medio no ponderado por lag.
    pub rmse: f64,
    /// Error absoluto medio no ponderado por lag.
    pub mean_absolute_error: f64,
}

/// Modelo variográfico serializable y reusable para estimación posterior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariogramModel {
    /// Variable analizada.
    pub column_id: ColumnId,
    /// Dominio filtrado; `None` indica selección global.
    pub domain: Option<String>,
    /// Restricción direccional aplicada; `None` indica variograma omni.
    pub direction: Option<VariogramDirection>,
    /// Tipo de modelo autorizado.
    pub model_kind: VariogramModelKind,
    /// Componente nugget explícita.
    pub nugget: f64,
    /// Sill parcial estructurado; para `Nugget` debe ser `0.0`.
    pub partial_sill: f64,
    /// Rango práctico del modelo estructurado; `None` para `Nugget`.
    pub range: Option<f64>,
    /// Métricas del fitting que originó el modelo.
    pub fit_summary: VariogramFitSummary,
}

impl VariogramModel {
    /// Construye un modelo variográfico desde el contexto de un variograma experimental.
    pub fn from_variogram(
        variogram: &ExperimentalVariogram,
        model_kind: VariogramModelKind,
        nugget: f64,
        partial_sill: f64,
        range: Option<f64>,
        fit_summary: VariogramFitSummary,
    ) -> Result<Self, MineError> {
        if !nugget.is_finite() || nugget < 0.0 {
            return Err(MineError::invalid_parameter(
                "nugget",
                "variogram nugget must be finite and greater than or equal to zero",
            ));
        }
        if !partial_sill.is_finite() || partial_sill < 0.0 {
            return Err(MineError::invalid_parameter(
                "partial_sill",
                "variogram partial sill must be finite and greater than or equal to zero",
            ));
        }
        if !fit_summary.weighted_sse.is_finite()
            || !fit_summary.rmse.is_finite()
            || !fit_summary.mean_absolute_error.is_finite()
        {
            return Err(MineError::invalid_parameter(
                "fit_summary",
                "variogram fit summary metrics must be finite",
            ));
        }
        if fit_summary.observed_lag_count == 0 || fit_summary.total_pair_count == 0 {
            return Err(MineError::invalid_parameter(
                "fit_summary",
                "variogram fit summary must report at least one observed lag and one pair",
            ));
        }

        match model_kind {
            VariogramModelKind::Nugget => {
                if partial_sill != 0.0 {
                    return Err(MineError::validation(
                        "nugget variogram model must not carry structured sill",
                    ));
                }
                if range.is_some() {
                    return Err(MineError::validation(
                        "nugget variogram model must not define a range",
                    ));
                }
            }
            _ => {
                if partial_sill <= 0.0 {
                    return Err(MineError::validation(
                        "structured variogram model must have a strictly positive partial sill",
                    ));
                }
                let Some(range) = range else {
                    return Err(MineError::validation(
                        "structured variogram model requires an explicit range",
                    ));
                };
                if !range.is_finite() || range <= 0.0 {
                    return Err(MineError::invalid_parameter(
                        "range",
                        "variogram range must be finite and greater than zero",
                    ));
                }
            }
        }

        Ok(Self {
            column_id: variogram.column_id.clone(),
            domain: variogram.domain.clone(),
            direction: variogram.direction.clone(),
            model_kind,
            nugget,
            partial_sill,
            range,
            fit_summary,
        })
    }

    /// Devuelve el sill total (`nugget + partial_sill`).
    #[must_use]
    pub fn total_sill(&self) -> f64 {
        self.nugget + self.partial_sill
    }

    /// Evalúa la semivarianza para una distancia dada usando el modelo actual.
    pub fn semivariance(&self, distance: f64) -> Result<f64, MineError> {
        if !distance.is_finite() || distance < 0.0 {
            return Err(MineError::invalid_parameter(
                "distance",
                "variogram distance must be finite and greater than or equal to zero",
            ));
        }

        let nugget_component = if distance > 0.0 { self.nugget } else { 0.0 };
        let structured_component = match self.model_kind {
            VariogramModelKind::Nugget => 0.0,
            VariogramModelKind::Spherical => {
                let range = self
                    .range
                    .expect("validated spherical variogram model should carry range");
                let ratio = (distance / range).min(1.0);
                if ratio >= 1.0 {
                    self.partial_sill
                } else {
                    self.partial_sill * (1.5 * ratio - 0.5 * ratio.powi(3))
                }
            }
            VariogramModelKind::Exponential => {
                let range = self
                    .range
                    .expect("validated exponential variogram model should carry range");
                self.partial_sill * (1.0 - (-3.0 * distance / range).exp())
            }
            VariogramModelKind::Gaussian => {
                let range = self
                    .range
                    .expect("validated gaussian variogram model should carry range");
                self.partial_sill * (1.0 - (-3.0 * distance.powi(2) / range.powi(2)).exp())
            }
        };

        Ok(nugget_component + structured_component)
    }
}

/// Ajusta un modelo variográfico autorizado sobre un variograma experimental.
pub fn fit_variogram_model(
    variogram: &ExperimentalVariogram,
    model_kind: VariogramModelKind,
    options: &VariogramFitOptions,
) -> Result<VariogramModel, MineError> {
    let observed_lags = collect_observed_lags(variogram)?;

    match model_kind {
        VariogramModelKind::Nugget => fit_nugget_model(variogram, &observed_lags),
        _ => fit_structured_model(variogram, model_kind, options, &observed_lags),
    }
}

/// Construye un variograma experimental omni o direccional para una variable y dominio.
pub fn build_experimental_variogram(
    samples: &[SpatialSample],
    column_id: &ColumnId,
    lag_config: &VariogramLagConfig,
    direction: Option<&VariogramDirection>,
    domain: Option<&str>,
) -> Result<ExperimentalVariogram, MineError> {
    if samples.is_empty() {
        return Err(MineError::invalid_parameter(
            "samples",
            "experimental variography requires at least one spatial sample",
        ));
    }

    validate_unique_sample_ids(samples)?;

    let selected_samples = samples
        .iter()
        .filter(|sample| match domain {
            Some(domain) => sample.domain.as_deref() == Some(domain),
            None => true,
        })
        .collect::<Vec<_>>();

    if selected_samples.len() < 2 {
        return Err(MineError::validation(
            "experimental variography requires at least two spatial samples in the selection",
        ));
    }

    for sample in &selected_samples {
        if !sample.values.contains_key(column_id) {
            return Err(MineError::schema(format!(
                "spatial sample `{}` is missing value column `{column_id}`",
                sample.sample_id
            )));
        }
    }

    let mut accumulators = (0..lag_config.lag_count())
        .map(|index| LagAccumulator {
            lag_index: index + 1,
            lag_center: (index + 1) as f64 * lag_config.lag_size(),
            pair_count: 0,
            distance_sum: 0.0,
            semivariance_sum: 0.0,
        })
        .collect::<Vec<_>>();

    for left_index in 0..selected_samples.len() {
        let left = selected_samples[left_index];
        let left_value = *left
            .values
            .get(column_id)
            .expect("selected sample should contain requested column");
        for right in &selected_samples[left_index + 1..] {
            let right_value = *right
                .values
                .get(column_id)
                .expect("selected sample should contain requested column");
            let pair = build_pair_geometry(left.location, right.location);
            if pair.distance <= 0.0 {
                continue;
            }
            if !direction_matches(direction, &pair)? {
                continue;
            }

            let lag_number = (pair.distance / lag_config.lag_size()).round() as isize;
            if lag_number < 1 || lag_number > lag_config.lag_count() as isize {
                continue;
            }

            let accumulator = &mut accumulators[lag_number as usize - 1];
            if (pair.distance - accumulator.lag_center).abs() > lag_config.lag_tolerance() {
                continue;
            }

            accumulator.pair_count += 1;
            accumulator.distance_sum += pair.distance;
            accumulator.semivariance_sum += 0.5 * (left_value - right_value).powi(2);
        }
    }

    let lags = accumulators
        .into_iter()
        .map(|accumulator| ExperimentalVariogramLag {
            lag_index: accumulator.lag_index,
            lag_center: accumulator.lag_center,
            pair_count: accumulator.pair_count,
            average_distance: (accumulator.pair_count > 0)
                .then_some(accumulator.distance_sum / accumulator.pair_count as f64),
            semivariance: (accumulator.pair_count > 0)
                .then_some(accumulator.semivariance_sum / accumulator.pair_count as f64),
        })
        .collect();

    Ok(ExperimentalVariogram {
        column_id: column_id.clone(),
        domain: domain.map(ToOwned::to_owned),
        direction: direction.cloned(),
        lag_config: lag_config.clone(),
        sample_count: selected_samples.len(),
        lags,
    })
}

/// Reconstruye un variograma experimental desde filas tabulares por lag.
pub fn experimental_variogram_from_lag_rows(
    rows: &[ExperimentalVariogramLagRow],
) -> Result<ExperimentalVariogram, MineError> {
    if rows.is_empty() {
        return Err(MineError::invalid_parameter(
            "rows",
            "experimental variogram rows must not be empty",
        ));
    }

    let first = &rows[0];
    let lag_config = VariogramLagConfig::new(first.lag_size, first.lag_count, first.lag_tolerance)?;
    let direction = reconstruct_direction(first)?;
    let mut sorted_rows = rows.to_vec();
    sorted_rows.sort_by_key(|row| row.lag_index);

    for row in &sorted_rows {
        if row.column_id != first.column_id
            || row.domain != first.domain
            || row.lag_size != first.lag_size
            || row.lag_count != first.lag_count
            || row.lag_tolerance != first.lag_tolerance
            || row.sample_count != first.sample_count
            || row.direction_x != first.direction_x
            || row.direction_y != first.direction_y
            || row.direction_z != first.direction_z
            || row.angular_tolerance_degrees != first.angular_tolerance_degrees
            || row.bandwidth != first.bandwidth
        {
            return Err(MineError::validation(
                "experimental variogram rows must share the same metadata across all lags",
            ));
        }
    }

    if sorted_rows.len() != first.lag_count {
        return Err(MineError::validation(
            "experimental variogram rows must contain one row per declared lag",
        ));
    }

    let mut lags = Vec::with_capacity(sorted_rows.len());
    for (expected_index, row) in sorted_rows.into_iter().enumerate() {
        let lag_index = expected_index + 1;
        if row.lag_index != lag_index {
            return Err(MineError::validation(
                "experimental variogram lag rows must be contiguous starting at 1",
            ));
        }
        lags.push(ExperimentalVariogramLag {
            lag_index: row.lag_index,
            lag_center: row.lag_center,
            pair_count: row.pair_count,
            average_distance: row.average_distance,
            semivariance: row.semivariance,
        });
    }

    Ok(ExperimentalVariogram {
        column_id: first.column_id.clone(),
        domain: first.domain.clone(),
        direction,
        lag_config,
        sample_count: first.sample_count,
        lags,
    })
}

fn validate_unique_sample_ids(samples: &[SpatialSample]) -> Result<(), MineError> {
    let mut seen = std::collections::BTreeSet::new();
    for sample in samples {
        if !seen.insert(sample.sample_id.clone()) {
            return Err(MineError::validation(format!(
                "duplicate spatial sample id `{}` found while building experimental variogram",
                sample.sample_id
            )));
        }
    }
    Ok(())
}

fn collect_observed_lags(
    variogram: &ExperimentalVariogram,
) -> Result<Vec<ObservedVariogramLag>, MineError> {
    let mut observed = Vec::new();

    for lag in &variogram.lags {
        if lag.pair_count == 0 {
            continue;
        }

        let Some(distance) = lag.average_distance else {
            return Err(MineError::validation(
                "experimental variogram lags with pairs must include average distance",
            ));
        };
        let Some(semivariance) = lag.semivariance else {
            return Err(MineError::validation(
                "experimental variogram lags with pairs must include semivariance",
            ));
        };
        if !distance.is_finite() || distance <= 0.0 {
            return Err(MineError::validation(
                "experimental variogram average distances must be finite and greater than zero",
            ));
        }
        if !semivariance.is_finite() || semivariance < 0.0 {
            return Err(MineError::validation(
                "experimental variogram semivariances must be finite and greater than or equal to zero",
            ));
        }

        observed.push(ObservedVariogramLag {
            distance,
            semivariance,
            pair_count: lag.pair_count,
        });
    }

    if observed.is_empty() {
        return Err(MineError::validation(
            "variogram fitting requires at least one lag with observed pairs",
        ));
    }

    observed.sort_by(|left, right| left.distance.total_cmp(&right.distance));
    Ok(observed)
}

fn fit_nugget_model(
    variogram: &ExperimentalVariogram,
    observed_lags: &[ObservedVariogramLag],
) -> Result<VariogramModel, MineError> {
    let total_pair_count = observed_lags
        .iter()
        .map(|lag| lag.pair_count)
        .sum::<usize>();
    let weighted_sum = observed_lags
        .iter()
        .map(|lag| lag.semivariance * lag.pair_count as f64)
        .sum::<f64>();
    let nugget = weighted_sum / total_pair_count as f64;
    let fit_summary =
        compute_fit_summary(VariogramModelKind::Nugget, nugget, 0.0, None, observed_lags)?;

    VariogramModel::from_variogram(
        variogram,
        VariogramModelKind::Nugget,
        nugget,
        0.0,
        None,
        fit_summary,
    )
}

fn fit_structured_model(
    variogram: &ExperimentalVariogram,
    model_kind: VariogramModelKind,
    options: &VariogramFitOptions,
    observed_lags: &[ObservedVariogramLag],
) -> Result<VariogramModel, MineError> {
    if !model_kind.requires_range() {
        return Err(MineError::validation(
            "structured variogram fitting requires a non-nugget model kind",
        ));
    }
    if observed_lags.len() < 2 {
        return Err(MineError::validation(
            "structured variogram fitting requires at least two observed lags",
        ));
    }

    let min_distance = observed_lags
        .iter()
        .map(|lag| lag.distance)
        .min_by(|left, right| left.total_cmp(right))
        .expect("observed lags should not be empty");
    let max_distance = observed_lags
        .iter()
        .map(|lag| lag.distance)
        .max_by(|left, right| left.total_cmp(right))
        .expect("observed lags should not be empty");
    let max_semivariance = observed_lags
        .iter()
        .map(|lag| lag.semivariance)
        .fold(0.0, f64::max);

    if max_semivariance <= 0.0 {
        return Err(MineError::validation(
            "structured variogram fitting requires positive observed semivariance",
        ));
    }

    let max_range = options
        .max_range()
        .unwrap_or(max_distance * 3.0)
        .max(min_distance);
    let partial_sill_upper = (max_semivariance * 2.0).max(f64::EPSILON);
    let nugget_candidates = if options.include_nugget() {
        inclusive_grid(0.0, max_semivariance, options.nugget_steps())
    } else {
        vec![0.0]
    };
    let partial_sill_candidates = inclusive_grid(0.0, partial_sill_upper, options.sill_steps())
        .into_iter()
        .filter(|candidate| *candidate > 0.0)
        .collect::<Vec<_>>();
    let range_candidates = inclusive_grid(min_distance, max_range, options.range_steps());

    let mut best_candidate: Option<(f64, f64, f64, VariogramFitSummary)> = None;
    for nugget in nugget_candidates {
        for partial_sill in &partial_sill_candidates {
            for range in &range_candidates {
                let fit_summary = compute_fit_summary(
                    model_kind,
                    nugget,
                    *partial_sill,
                    Some(*range),
                    observed_lags,
                )?;
                let is_better = best_candidate
                    .as_ref()
                    .is_none_or(|(_, _, _, best_summary)| {
                        fit_summary.weighted_sse < best_summary.weighted_sse
                    });
                if is_better {
                    best_candidate = Some((nugget, *partial_sill, *range, fit_summary));
                }
            }
        }
    }

    let Some((nugget, partial_sill, range, fit_summary)) = best_candidate else {
        return Err(MineError::validation(
            "variogram fitting could not find any physically valid candidate",
        ));
    };

    VariogramModel::from_variogram(
        variogram,
        model_kind,
        nugget,
        partial_sill,
        Some(range),
        fit_summary,
    )
}

fn compute_fit_summary(
    model_kind: VariogramModelKind,
    nugget: f64,
    partial_sill: f64,
    range: Option<f64>,
    observed_lags: &[ObservedVariogramLag],
) -> Result<VariogramFitSummary, MineError> {
    let total_pair_count = observed_lags
        .iter()
        .map(|lag| lag.pair_count)
        .sum::<usize>();
    let mut weighted_sse = 0.0;
    let mut squared_error_sum = 0.0;
    let mut absolute_error_sum = 0.0;

    for lag in observed_lags {
        let fitted = evaluate_semivariance(model_kind, nugget, partial_sill, range, lag.distance)?;
        let residual = fitted - lag.semivariance;
        weighted_sse += residual.powi(2) * lag.pair_count as f64;
        squared_error_sum += residual.powi(2);
        absolute_error_sum += residual.abs();
    }

    Ok(VariogramFitSummary {
        observed_lag_count: observed_lags.len(),
        total_pair_count,
        weighted_sse,
        rmse: (squared_error_sum / observed_lags.len() as f64).sqrt(),
        mean_absolute_error: absolute_error_sum / observed_lags.len() as f64,
    })
}

fn evaluate_semivariance(
    model_kind: VariogramModelKind,
    nugget: f64,
    partial_sill: f64,
    range: Option<f64>,
    distance: f64,
) -> Result<f64, MineError> {
    if !distance.is_finite() || distance < 0.0 {
        return Err(MineError::invalid_parameter(
            "distance",
            "variogram distance must be finite and greater than or equal to zero",
        ));
    }
    if !nugget.is_finite() || nugget < 0.0 {
        return Err(MineError::invalid_parameter(
            "nugget",
            "variogram nugget must be finite and greater than or equal to zero",
        ));
    }
    if !partial_sill.is_finite() || partial_sill < 0.0 {
        return Err(MineError::invalid_parameter(
            "partial_sill",
            "variogram partial sill must be finite and greater than or equal to zero",
        ));
    }

    let nugget_component = if distance > 0.0 { nugget } else { 0.0 };
    let structured_component = match model_kind {
        VariogramModelKind::Nugget => {
            if partial_sill != 0.0 || range.is_some() {
                return Err(MineError::validation(
                    "nugget variogram candidates must not carry structured parameters",
                ));
            }
            0.0
        }
        VariogramModelKind::Spherical => {
            let Some(range) = range else {
                return Err(MineError::validation(
                    "structured variogram candidates require an explicit range",
                ));
            };
            if !range.is_finite() || range <= 0.0 {
                return Err(MineError::invalid_parameter(
                    "range",
                    "variogram range must be finite and greater than zero",
                ));
            }
            if partial_sill <= 0.0 {
                return Err(MineError::validation(
                    "structured variogram candidates require a positive partial sill",
                ));
            }
            let ratio = (distance / range).min(1.0);
            if ratio >= 1.0 {
                partial_sill
            } else {
                partial_sill * (1.5 * ratio - 0.5 * ratio.powi(3))
            }
        }
        VariogramModelKind::Exponential => {
            let Some(range) = range else {
                return Err(MineError::validation(
                    "structured variogram candidates require an explicit range",
                ));
            };
            if !range.is_finite() || range <= 0.0 {
                return Err(MineError::invalid_parameter(
                    "range",
                    "variogram range must be finite and greater than zero",
                ));
            }
            if partial_sill <= 0.0 {
                return Err(MineError::validation(
                    "structured variogram candidates require a positive partial sill",
                ));
            }
            partial_sill * (1.0 - (-3.0 * distance / range).exp())
        }
        VariogramModelKind::Gaussian => {
            let Some(range) = range else {
                return Err(MineError::validation(
                    "structured variogram candidates require an explicit range",
                ));
            };
            if !range.is_finite() || range <= 0.0 {
                return Err(MineError::invalid_parameter(
                    "range",
                    "variogram range must be finite and greater than zero",
                ));
            }
            if partial_sill <= 0.0 {
                return Err(MineError::validation(
                    "structured variogram candidates require a positive partial sill",
                ));
            }
            partial_sill * (1.0 - (-3.0 * distance.powi(2) / range.powi(2)).exp())
        }
    };

    Ok(nugget_component + structured_component)
}

fn inclusive_grid(start: f64, end: f64, steps: usize) -> Vec<f64> {
    if steps == 1 || (end - start).abs() <= f64::EPSILON {
        return vec![start];
    }

    let step = (end - start) / (steps - 1) as f64;
    (0..steps)
        .map(|index| start + step * index as f64)
        .collect()
}

fn reconstruct_direction(
    row: &ExperimentalVariogramLagRow,
) -> Result<Option<VariogramDirection>, MineError> {
    match (
        row.direction_x,
        row.direction_y,
        row.direction_z,
        row.angular_tolerance_degrees,
    ) {
        (Some(x), Some(y), Some(z), Some(angular_tolerance_degrees)) => {
            Ok(Some(VariogramDirection::new(
                Coordinate3D::new(x, y, z)?,
                angular_tolerance_degrees,
                row.bandwidth,
            )?))
        }
        (None, None, None, None) => Ok(None),
        _ => Err(MineError::validation(
            "experimental variogram rows contain a partial directional specification",
        )),
    }
}

fn direction_matches(
    direction: Option<&VariogramDirection>,
    pair: &PairGeometry,
) -> Result<bool, MineError> {
    let Some(direction) = direction else {
        return Ok(true);
    };

    let direction_norm = (direction.vector().x() * direction.vector().x()
        + direction.vector().y() * direction.vector().y()
        + direction.vector().z() * direction.vector().z())
    .sqrt();
    if direction_norm <= 0.0 {
        return Err(MineError::numeric(
            "variogram direction vector norm must stay positive",
        ));
    }

    let unit_x = direction.vector().x() / direction_norm;
    let unit_y = direction.vector().y() / direction_norm;
    let unit_z = direction.vector().z() / direction_norm;
    let parallel_projection = pair.dx * unit_x + pair.dy * unit_y + pair.dz * unit_z;
    let cosine = (parallel_projection.abs() / pair.distance).clamp(-1.0, 1.0);
    let angle_degrees = cosine.acos().to_degrees();
    if angle_degrees > direction.angular_tolerance_degrees() {
        return Ok(false);
    }

    if let Some(bandwidth) = direction.bandwidth() {
        let perpendicular_sq =
            (pair.distance * pair.distance) - (parallel_projection * parallel_projection);
        let perpendicular_distance = perpendicular_sq.max(0.0).sqrt();
        if perpendicular_distance > bandwidth {
            return Ok(false);
        }
    }

    Ok(true)
}

fn build_pair_geometry(left: Coordinate3D, right: Coordinate3D) -> PairGeometry {
    let dx = right.x() - left.x();
    let dy = right.y() - left.y();
    let dz = right.z() - left.z();
    PairGeometry {
        dx,
        dy,
        dz,
        distance: (dx * dx + dy * dy + dz * dz).sqrt(),
    }
}

#[derive(Debug)]
struct PairGeometry {
    dx: f64,
    dy: f64,
    dz: f64,
    distance: f64,
}

#[derive(Debug, Clone, Copy)]
struct ObservedVariogramLag {
    distance: f64,
    semivariance: f64,
    pair_count: usize,
}

#[derive(Debug)]
struct LagAccumulator {
    lag_index: usize,
    lag_center: f64,
    pair_count: usize,
    distance_sum: f64,
    semivariance_sum: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64) {
        let difference = (left - right).abs();
        assert!(
            difference <= 1.0e-9,
            "expected {left} to be close to {right}, difference {difference}"
        );
    }

    fn sample_fit_summary() -> VariogramFitSummary {
        VariogramFitSummary {
            observed_lag_count: 2,
            total_pair_count: 10,
            weighted_sse: 0.0,
            rmse: 0.0,
            mean_absolute_error: 0.0,
        }
    }

    fn sample_variogram(lags: Vec<ExperimentalVariogramLag>) -> ExperimentalVariogram {
        ExperimentalVariogram {
            column_id: ColumnId::new("cu").expect("column id should be valid"),
            domain: Some("ore".to_owned()),
            direction: None,
            lag_config: VariogramLagConfig::new(4.0, lags.len(), 0.5)
                .expect("lag config should be valid"),
            sample_count: 12,
            lags,
        }
    }

    #[test]
    fn spherical_model_evaluates_known_points() {
        let variogram = sample_variogram(vec![
            ExperimentalVariogramLag {
                lag_index: 1,
                lag_center: 4.0,
                pair_count: 3,
                average_distance: Some(4.0),
                semivariance: Some(0.5),
            },
            ExperimentalVariogramLag {
                lag_index: 2,
                lag_center: 8.0,
                pair_count: 2,
                average_distance: Some(8.0),
                semivariance: Some(0.7),
            },
        ]);
        let model = VariogramModel::from_variogram(
            &variogram,
            VariogramModelKind::Spherical,
            0.1,
            0.9,
            Some(10.0),
            sample_fit_summary(),
        )
        .expect("model should be valid");

        assert_close(
            model.semivariance(0.0).expect("distance should be valid"),
            0.0,
        );
        assert_close(
            model.semivariance(5.0).expect("distance should be valid"),
            0.71875,
        );
        assert_close(
            model.semivariance(10.0).expect("distance should be valid"),
            1.0,
        );
        assert_close(model.total_sill(), 1.0);
    }

    #[test]
    fn nugget_model_rejects_structured_parameters() {
        let variogram = sample_variogram(vec![
            ExperimentalVariogramLag {
                lag_index: 1,
                lag_center: 4.0,
                pair_count: 2,
                average_distance: Some(4.0),
                semivariance: Some(0.4),
            },
            ExperimentalVariogramLag {
                lag_index: 2,
                lag_center: 8.0,
                pair_count: 2,
                average_distance: Some(8.0),
                semivariance: Some(0.6),
            },
        ]);
        let error = VariogramModel::from_variogram(
            &variogram,
            VariogramModelKind::Nugget,
            0.2,
            0.3,
            None,
            sample_fit_summary(),
        )
        .expect_err("nugget model should reject structured sill");

        assert_eq!(
            error,
            MineError::validation("nugget variogram model must not carry structured sill")
        );
    }

    #[test]
    fn fit_nugget_model_matches_weighted_average_semivariance() {
        let variogram = sample_variogram(vec![
            ExperimentalVariogramLag {
                lag_index: 1,
                lag_center: 4.0,
                pair_count: 10,
                average_distance: Some(4.0),
                semivariance: Some(0.2),
            },
            ExperimentalVariogramLag {
                lag_index: 2,
                lag_center: 8.0,
                pair_count: 20,
                average_distance: Some(8.0),
                semivariance: Some(0.8),
            },
        ]);

        let model = fit_variogram_model(
            &variogram,
            VariogramModelKind::Nugget,
            &VariogramFitOptions::default(),
        )
        .expect("fit should work");

        assert_eq!(model.model_kind, VariogramModelKind::Nugget);
        assert_close(model.nugget, 0.6);
        assert_eq!(model.range, None);
        assert_close(model.partial_sill, 0.0);
    }

    #[test]
    fn fit_spherical_model_recovers_fixture_parameters() {
        let variogram = sample_variogram(vec![
            ExperimentalVariogramLag {
                lag_index: 1,
                lag_center: 4.0,
                pair_count: 10,
                average_distance: Some(4.0),
                semivariance: Some(0.5851851851851851),
            },
            ExperimentalVariogramLag {
                lag_index: 2,
                lag_center: 8.0,
                pair_count: 8,
                average_distance: Some(8.0),
                semivariance: Some(0.8814814814814815),
            },
            ExperimentalVariogramLag {
                lag_index: 3,
                lag_center: 12.0,
                pair_count: 6,
                average_distance: Some(12.0),
                semivariance: Some(1.0),
            },
        ]);
        let options =
            VariogramFitOptions::new(true, 6, 11, 3, Some(12.0)).expect("options should be valid");

        let model = fit_variogram_model(&variogram, VariogramModelKind::Spherical, &options)
            .expect("fit should work");

        assert_eq!(model.model_kind, VariogramModelKind::Spherical);
        assert_close(model.nugget, 0.2);
        assert_close(model.partial_sill, 0.8);
        assert_close(model.range.expect("range should exist"), 12.0);
        assert_close(model.fit_summary.weighted_sse, 0.0);
    }

    #[test]
    fn fit_structured_model_requires_two_observed_lags() {
        let variogram = sample_variogram(vec![ExperimentalVariogramLag {
            lag_index: 1,
            lag_center: 4.0,
            pair_count: 4,
            average_distance: Some(4.0),
            semivariance: Some(0.5),
        }]);

        let error = fit_variogram_model(
            &variogram,
            VariogramModelKind::Gaussian,
            &VariogramFitOptions::default(),
        )
        .expect_err("fit should reject a single lag");

        assert_eq!(
            error,
            MineError::validation(
                "structured variogram fitting requires at least two observed lags"
            )
        );
    }

    #[test]
    fn collect_observed_lags_orders_by_distance() {
        let variogram = sample_variogram(vec![
            ExperimentalVariogramLag {
                lag_index: 2,
                lag_center: 8.0,
                pair_count: 2,
                average_distance: Some(8.0),
                semivariance: Some(0.7),
            },
            ExperimentalVariogramLag {
                lag_index: 1,
                lag_center: 4.0,
                pair_count: 4,
                average_distance: Some(4.0),
                semivariance: Some(0.4),
            },
        ]);

        let observed = collect_observed_lags(&variogram).expect("observed lags should work");

        assert_eq!(observed.len(), 2);
        assert_close(observed[0].distance, 4.0);
        assert_close(observed[1].distance, 8.0);
    }
}
