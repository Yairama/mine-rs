//! Motor configurable de metricas para evidencia de clasificacion de recursos.
//!
//! El objetivo es producir evidencia estructurada y auditable a partir de:
//! - sample spacing;
//! - informedness del esquema de estimacion;
//! - continuidad implicita en variografia y cross-validation.
//!
//! Este modulo no automatiza compliance ni reemplaza el juicio profesional.

use std::collections::{BTreeMap, BTreeSet};

use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

use crate::{
    CrossValidationReport, EstimationPass, SpatialSample, VariogramModel,
    neighborhoods::select_samples_by_estimation_passes,
};

/// Umbrales configurables para un nivel de evidencia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationThreshold {
    label: String,
    max_p90_sample_spacing: Option<f64>,
    max_mean_sample_spacing: Option<f64>,
    min_p10_informing_samples: Option<usize>,
    min_support_coverage_ratio: Option<f64>,
    min_variogram_range: Option<f64>,
    min_cross_validation_correlation: Option<f64>,
    max_variogram_fit_rmse: Option<f64>,
}

impl ClassificationThreshold {
    /// Construye un umbral configurable para un nivel de evidencia.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        label: impl Into<String>,
        max_p90_sample_spacing: Option<f64>,
        max_mean_sample_spacing: Option<f64>,
        min_p10_informing_samples: Option<usize>,
        min_support_coverage_ratio: Option<f64>,
        min_variogram_range: Option<f64>,
        min_cross_validation_correlation: Option<f64>,
        max_variogram_fit_rmse: Option<f64>,
    ) -> Result<Self, MineError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "label",
                "classification threshold label must not be empty",
            ));
        }

        validate_optional_positive("max_p90_sample_spacing", max_p90_sample_spacing)?;
        validate_optional_positive("max_mean_sample_spacing", max_mean_sample_spacing)?;
        validate_optional_ratio("min_support_coverage_ratio", min_support_coverage_ratio)?;
        validate_optional_positive("min_variogram_range", min_variogram_range)?;
        validate_optional_ratio(
            "min_cross_validation_correlation",
            min_cross_validation_correlation,
        )?;
        validate_optional_positive("max_variogram_fit_rmse", max_variogram_fit_rmse)?;

        if max_p90_sample_spacing.is_none()
            && max_mean_sample_spacing.is_none()
            && min_p10_informing_samples.is_none()
            && min_support_coverage_ratio.is_none()
            && min_variogram_range.is_none()
            && min_cross_validation_correlation.is_none()
            && max_variogram_fit_rmse.is_none()
        {
            return Err(MineError::invalid_parameter(
                "threshold",
                "classification threshold must define at least one metric criterion",
            ));
        }

        Ok(Self {
            label,
            max_p90_sample_spacing,
            max_mean_sample_spacing,
            min_p10_informing_samples,
            min_support_coverage_ratio,
            min_variogram_range,
            min_cross_validation_correlation,
            max_variogram_fit_rmse,
        })
    }

    /// Etiqueta estable del nivel de evidencia.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    fn evaluate(
        &self,
        spacing: &SampleSpacingMetrics,
        informedness: &InformednessMetrics,
        continuity: &ContinuityMetrics,
    ) -> ClassificationLevelAssessment {
        let mut spacing_failures = Vec::new();
        let mut informedness_failures = Vec::new();
        let mut continuity_failures = Vec::new();

        if let Some(max_spacing) = self.max_p90_sample_spacing
            && spacing.p90_nearest_neighbor_distance > max_spacing
        {
            spacing_failures.push(format!(
                "p90 sample spacing {:.4} exceeds threshold {:.4}",
                spacing.p90_nearest_neighbor_distance, max_spacing
            ));
        }
        if let Some(max_spacing) = self.max_mean_sample_spacing
            && spacing.mean_nearest_neighbor_distance > max_spacing
        {
            spacing_failures.push(format!(
                "mean sample spacing {:.4} exceeds threshold {:.4}",
                spacing.mean_nearest_neighbor_distance, max_spacing
            ));
        }

        if let Some(min_samples) = self.min_p10_informing_samples
            && informedness.p10_informing_sample_count < min_samples
        {
            informedness_failures.push(format!(
                "p10 informing sample count {} is below threshold {}",
                informedness.p10_informing_sample_count, min_samples
            ));
        }
        if let Some(min_ratio) = self.min_support_coverage_ratio
            && informedness.support_coverage_ratio < min_ratio
        {
            informedness_failures.push(format!(
                "support coverage ratio {:.4} is below threshold {:.4}",
                informedness.support_coverage_ratio, min_ratio
            ));
        }

        if let Some(min_range) = self.min_variogram_range {
            match continuity.variogram_range {
                Some(range) if range >= min_range => {}
                Some(range) => continuity_failures.push(format!(
                    "variogram range {:.4} is below threshold {:.4}",
                    range, min_range
                )),
                None => continuity_failures.push(format!(
                    "variogram range is unavailable but threshold {:.4} was required",
                    min_range
                )),
            }
        }
        if let Some(min_corr) = self.min_cross_validation_correlation
            && continuity.cross_validation_correlation < min_corr
        {
            continuity_failures.push(format!(
                "cross-validation correlation {:.4} is below threshold {:.4}",
                continuity.cross_validation_correlation, min_corr
            ));
        }
        if let Some(max_rmse) = self.max_variogram_fit_rmse
            && continuity.variogram_fit_rmse > max_rmse
        {
            continuity_failures.push(format!(
                "variogram fit rmse {:.4} exceeds threshold {:.4}",
                continuity.variogram_fit_rmse, max_rmse
            ));
        }

        let spacing_passed = spacing_failures.is_empty();
        let informedness_passed = informedness_failures.is_empty();
        let continuity_passed = continuity_failures.is_empty();

        let mut failed_checks = Vec::new();
        failed_checks.extend(spacing_failures);
        failed_checks.extend(informedness_failures);
        failed_checks.extend(continuity_failures);

        ClassificationLevelAssessment {
            label: self.label.clone(),
            spacing_passed,
            informedness_passed,
            continuity_passed,
            overall_passed: failed_checks.is_empty(),
            failed_checks,
        }
    }
}

/// Configuracion del motor de metricas de clasificacion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationMetricConfig {
    thresholds: Vec<ClassificationThreshold>,
}

impl ClassificationMetricConfig {
    /// Construye una configuracion validando labels unicos.
    pub fn new(thresholds: Vec<ClassificationThreshold>) -> Result<Self, MineError> {
        if thresholds.is_empty() {
            return Err(MineError::invalid_parameter(
                "thresholds",
                "classification metrics require at least one threshold definition",
            ));
        }

        let mut seen = BTreeSet::new();
        for threshold in &thresholds {
            if !seen.insert(threshold.label().to_owned()) {
                return Err(MineError::validation(format!(
                    "duplicate classification threshold label `{}`",
                    threshold.label()
                )));
            }
        }

        Ok(Self { thresholds })
    }

    /// Niveles configurados para evaluar la evidencia.
    #[must_use]
    pub fn thresholds(&self) -> &[ClassificationThreshold] {
        &self.thresholds
    }
}

/// Metricas de sample spacing derivadas del set de muestras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSpacingMetrics {
    /// Cantidad de muestras evaluadas.
    pub sample_count: usize,
    /// Distancia media al vecino mas cercano.
    pub mean_nearest_neighbor_distance: f64,
    /// Percentil 50 de distancia al vecino mas cercano.
    pub p50_nearest_neighbor_distance: f64,
    /// Percentil 90 de distancia al vecino mas cercano.
    pub p90_nearest_neighbor_distance: f64,
    /// Distancia maxima al vecino mas cercano.
    pub max_nearest_neighbor_distance: f64,
}

/// Uso de passes como evidencia de informedness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassUsageMetric {
    /// Pass seleccionado como primero que cumple el minimo.
    pub pass_id: String,
    /// Cantidad de targets que se resolvieron con este pass.
    pub selection_count: usize,
}

/// Metricas de informedness derivadas de los passes y la densidad efectiva.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InformednessMetrics {
    /// Cantidad de targets evaluados.
    pub target_count: usize,
    /// Cantidad de targets que cumplieron algun pass.
    pub supported_target_count: usize,
    /// Ratio de targets que cumplieron el minimo de soporte.
    pub support_coverage_ratio: f64,
    /// Numero medio de muestras informantes seleccionadas.
    pub mean_informing_sample_count: f64,
    /// Percentil 10 del conteo de muestras informantes.
    pub p10_informing_sample_count: usize,
    /// Histograma de uso por pass.
    pub pass_usage: Vec<PassUsageMetric>,
}

/// Metricas de continuidad derivadas de variografia y cross-validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContinuityMetrics {
    /// Columna evaluada.
    pub column_id: ColumnId,
    /// Rango practico del modelo variografico, cuando existe.
    pub variogram_range: Option<f64>,
    /// RMSE del fitting variografico.
    pub variogram_fit_rmse: f64,
    /// Correlacion de cross-validation.
    pub cross_validation_correlation: f64,
    /// RMSE de cross-validation.
    pub cross_validation_rmse: f64,
}

/// Evaluacion de un nivel configurable sobre las metricas calculadas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationLevelAssessment {
    /// Etiqueta del nivel evaluado.
    pub label: String,
    /// Resultado agregado del bloque de sample spacing.
    pub spacing_passed: bool,
    /// Resultado agregado del bloque de informedness.
    pub informedness_passed: bool,
    /// Resultado agregado del bloque de continuidad.
    pub continuity_passed: bool,
    /// Resultado total del nivel configurado.
    pub overall_passed: bool,
    /// Reglas incumplidas para este nivel.
    pub failed_checks: Vec<String>,
}

/// Reporte completo de evidencia para clasificacion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationMetricsReport {
    /// Variable evaluada.
    pub column_id: ColumnId,
    /// Evidencia de sample spacing.
    pub spacing: SampleSpacingMetrics,
    /// Evidencia de informedness.
    pub informedness: InformednessMetrics,
    /// Evidencia de continuidad.
    pub continuity: ContinuityMetrics,
    /// Evaluaciones por nivel configurado.
    pub assessments: Vec<ClassificationLevelAssessment>,
    /// Nota explicita para evitar automatizar compliance.
    pub advisory_note: String,
}

/// Evalua evidencia cuantitativa para clasificacion de recursos.
pub fn evaluate_classification_metrics(
    samples: &[SpatialSample],
    passes: &[EstimationPass],
    variogram_model: &VariogramModel,
    cross_validation_report: &CrossValidationReport,
    config: &ClassificationMetricConfig,
) -> Result<ClassificationMetricsReport, MineError> {
    if samples.len() < 2 {
        return Err(MineError::invalid_parameter(
            "samples",
            "classification metrics require at least two samples",
        ));
    }
    if passes.is_empty() {
        return Err(MineError::invalid_parameter(
            "passes",
            "classification metrics require at least one estimation pass",
        ));
    }
    if cross_validation_report.entries.is_empty() {
        return Err(MineError::invalid_parameter(
            "cross_validation_report",
            "classification metrics require a non-empty cross-validation report",
        ));
    }
    if variogram_model.column_id != cross_validation_report.column_id {
        return Err(MineError::validation(format!(
            "variogram column `{}` does not match cross-validation column `{}`",
            variogram_model.column_id, cross_validation_report.column_id
        )));
    }

    let spacing = compute_spacing_metrics(samples);
    let informedness = compute_informedness_metrics(samples, passes)?;
    let continuity = ContinuityMetrics {
        column_id: cross_validation_report.column_id.clone(),
        variogram_range: variogram_model.range,
        variogram_fit_rmse: variogram_model.fit_summary.rmse,
        cross_validation_correlation: cross_validation_report.metrics.correlation,
        cross_validation_rmse: cross_validation_report.metrics.rmse,
    };

    let assessments = config
        .thresholds()
        .iter()
        .map(|threshold| threshold.evaluate(&spacing, &informedness, &continuity))
        .collect();

    Ok(ClassificationMetricsReport {
        column_id: cross_validation_report.column_id.clone(),
        spacing,
        informedness,
        continuity,
        assessments,
        advisory_note:
            "These metrics provide auditable evidence only; they do not declare compliance classes or replace professional judgement.".to_owned(),
    })
}

fn compute_spacing_metrics(samples: &[SpatialSample]) -> SampleSpacingMetrics {
    let mut nearest_distances = Vec::with_capacity(samples.len());
    for (index, sample) in samples.iter().enumerate() {
        let nearest = samples
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .map(|(_, other)| euclidean_distance(sample, other))
            .fold(f64::INFINITY, f64::min);
        nearest_distances.push(nearest);
    }

    nearest_distances.sort_by(f64::total_cmp);
    let sample_count = nearest_distances.len();
    let mean = nearest_distances.iter().sum::<f64>() / sample_count as f64;

    SampleSpacingMetrics {
        sample_count,
        mean_nearest_neighbor_distance: mean,
        p50_nearest_neighbor_distance: percentile(&nearest_distances, 0.5),
        p90_nearest_neighbor_distance: percentile(&nearest_distances, 0.9),
        max_nearest_neighbor_distance: *nearest_distances
            .last()
            .expect("nearest distance list must not be empty"),
    }
}

fn compute_informedness_metrics(
    samples: &[SpatialSample],
    passes: &[EstimationPass],
) -> Result<InformednessMetrics, MineError> {
    let mut selected_counts = Vec::with_capacity(samples.len());
    let mut supported_target_count = 0usize;
    let mut pass_usage = BTreeMap::<String, usize>::new();

    for index in 0..samples.len() {
        let target = &samples[index];
        let mut remaining = Vec::with_capacity(samples.len() - 1);
        for (other_index, sample) in samples.iter().enumerate() {
            if other_index != index {
                remaining.push(sample.clone());
            }
        }

        let selection = select_samples_by_estimation_passes(target.location, &remaining, passes)?;
        selected_counts.push(selection.samples.len());
        if let Some(pass_id) = selection.selected_pass_id {
            supported_target_count += 1;
            *pass_usage.entry(pass_id).or_insert(0) += 1;
        }
    }

    selected_counts.sort_unstable();
    let target_count = selected_counts.len();
    let mean_informing_sample_count =
        selected_counts.iter().sum::<usize>() as f64 / target_count as f64;

    Ok(InformednessMetrics {
        target_count,
        supported_target_count,
        support_coverage_ratio: supported_target_count as f64 / target_count as f64,
        mean_informing_sample_count,
        p10_informing_sample_count: percentile_usize(&selected_counts, 0.1),
        pass_usage: pass_usage
            .into_iter()
            .map(|(pass_id, selection_count)| PassUsageMetric {
                pass_id,
                selection_count,
            })
            .collect(),
    })
}

fn percentile(sorted_values: &[f64], quantile: f64) -> f64 {
    let index = percentile_index(sorted_values.len(), quantile);
    sorted_values[index]
}

fn percentile_usize(sorted_values: &[usize], quantile: f64) -> usize {
    let index = percentile_index(sorted_values.len(), quantile);
    sorted_values[index]
}

fn percentile_index(length: usize, quantile: f64) -> usize {
    ((length.saturating_sub(1)) as f64 * quantile).round() as usize
}

fn euclidean_distance(left: &SpatialSample, right: &SpatialSample) -> f64 {
    let dx = left.location.x() - right.location.x();
    let dy = left.location.y() - right.location.y();
    let dz = left.location.z() - right.location.z();
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn validate_optional_positive(
    parameter: &'static str,
    value: Option<f64>,
) -> Result<(), MineError> {
    if let Some(value) = value
        && (!value.is_finite() || value <= 0.0)
    {
        return Err(MineError::invalid_parameter(
            parameter,
            "must be finite and greater than zero when provided",
        ));
    }
    Ok(())
}

fn validate_optional_ratio(parameter: &'static str, value: Option<f64>) -> Result<(), MineError> {
    if let Some(value) = value
        && (!value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        return Err(MineError::invalid_parameter(
            parameter,
            "must be finite and between 0.0 and 1.0 when provided",
        ));
    }
    Ok(())
}
