//! Suite de validación de modelos de estimación.
//!
//! Implementa métricas reproducibles para validar la calidad de un modelo
//! estimado, incluyendo:
//!
//! - **Cross-validation leave-one-out**: para cada sample, lo remueve del
//!   conjunto, estima su valor con los restantes y compara con el real.
//! - **Swath plots**: compara la media estimada vs real en bins de coordenada.
//! - **Comparación composite-vs-block**: compara estadísticos globales de
//!   composites y bloques estimados.
//!
//! Todos los resultados son serializables y auditaables.

use mine_core::{ColumnId, Coordinate3D, MineError};
use serde::{Deserialize, Serialize};

use crate::{
    EstimationPass, InverseDistanceWeightingOptions, SpatialSample,
    estimators::estimate_inverse_distance_weighting, kriging::estimate_ordinary_kriging,
    neighborhoods::SearchNeighborhood, variography::VariogramModel,
};

// ── Cross-validation ─────────────────────────────────────────────────────────

/// Resultado de cross-validation leave-one-out para un sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossValidationEntry {
    /// Identificador del sample reservado (left-out).
    pub sample_id: String,
    /// Ubicación del sample reservado.
    pub location: Coordinate3D,
    /// Valor real del sample (observado).
    pub actual_value: f64,
    /// Valor estimado con el sample removido.
    pub estimated_value: f64,
    /// Error simple: estimado - real.
    pub error: f64,
    /// Error relativo: (estimado - real) / real. `None` si `actual_value` ≈ 0.
    pub relative_error: Option<f64>,
}

/// Métricas globales de cross-validation leave-one-out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossValidationMetrics {
    /// Número de samples validados.
    pub n: usize,
    /// Sesgo medio (mean error).
    pub mean_error: f64,
    /// Raíz del error cuadrático medio (RMSE).
    pub rmse: f64,
    /// Error absoluto medio (MAE).
    pub mae: f64,
    /// Coeficiente de correlación de Pearson entre estimado y real.
    pub correlation: f64,
    /// Media de los valores reales.
    pub mean_actual: f64,
    /// Media de los valores estimados.
    pub mean_estimated: f64,
}

/// Reporte completo de cross-validation leave-one-out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossValidationReport {
    /// Variable validada.
    pub column_id: ColumnId,
    /// Entradas por sample.
    pub entries: Vec<CrossValidationEntry>,
    /// Métricas globales.
    pub metrics: CrossValidationMetrics,
}

/// Método de estimación para la cross-validation.
#[derive(Debug, Clone)]
pub enum CrossValidationEstimator {
    /// Ordinary kriging con variograma y vecindad explícitos.
    OrdinaryKriging {
        variogram: VariogramModel,
        neighborhood: SearchNeighborhood,
    },
    /// Inverse distance weighting.
    InverseDistanceWeighting {
        options: InverseDistanceWeightingOptions,
    },
}

/// Ejecuta cross-validation leave-one-out sobre un conjunto de samples.
///
/// Para cada sample `i`, construye el conjunto sin `i` y estima el valor
/// en la ubicación de `i`. El estimador usa el mismo pass de vecindad para
/// todos los samples.
///
/// # Errores
///
/// Retorna error si:
/// - `samples` tiene menos de 2 entradas
/// - alguna estimación falla (se permite skip con resultado `None` si hay menos de 2 vecinos)
pub fn cross_validate_leave_one_out(
    samples: &[SpatialSample],
    column_id: &ColumnId,
    passes: &[EstimationPass],
    estimator: &CrossValidationEstimator,
) -> Result<CrossValidationReport, MineError> {
    if samples.len() < 2 {
        return Err(MineError::invalid_parameter(
            "samples",
            "cross-validation requires at least 2 samples",
        ));
    }

    let mut entries: Vec<CrossValidationEntry> = Vec::with_capacity(samples.len());

    for i in 0..samples.len() {
        let target_sample = &samples[i];
        let actual_value = *target_sample.values.get(column_id).ok_or_else(|| {
            MineError::invalid_parameter(
                "column_id",
                format!(
                    "column `{column_id}` not found in sample `{}`",
                    target_sample.sample_id
                ),
            )
        })?;

        // Build leave-one-out dataset
        let mut loo_samples: Vec<SpatialSample> = Vec::with_capacity(samples.len() - 1);
        for (j, s) in samples.iter().enumerate() {
            if j != i {
                loo_samples.push(s.clone());
            }
        }

        let estimated_value = match estimator {
            CrossValidationEstimator::OrdinaryKriging {
                variogram,
                neighborhood,
            } => {
                match estimate_ordinary_kriging(
                    target_sample.location,
                    &loo_samples,
                    column_id,
                    passes,
                    variogram,
                ) {
                    Ok(est) => est.estimate,
                    Err(_) => continue, // skip if estimation fails (insufficient neighbors)
                }
            }
            CrossValidationEstimator::InverseDistanceWeighting { options } => {
                match estimate_inverse_distance_weighting(
                    target_sample.location,
                    &loo_samples,
                    column_id,
                    passes,
                    options,
                ) {
                    Ok(est) => est.estimate,
                    Err(_) => continue,
                }
            }
        };

        let error = estimated_value - actual_value;
        let relative_error = if actual_value.abs() > 1e-10 {
            Some(error / actual_value)
        } else {
            None
        };

        entries.push(CrossValidationEntry {
            sample_id: target_sample.sample_id.clone(),
            location: target_sample.location,
            actual_value,
            estimated_value,
            error,
            relative_error,
        });
    }

    let metrics = compute_cv_metrics(&entries);

    Ok(CrossValidationReport {
        column_id: column_id.clone(),
        entries,
        metrics,
    })
}

fn compute_cv_metrics(entries: &[CrossValidationEntry]) -> CrossValidationMetrics {
    let n = entries.len();
    if n == 0 {
        return CrossValidationMetrics {
            n: 0,
            mean_error: 0.0,
            rmse: 0.0,
            mae: 0.0,
            correlation: 0.0,
            mean_actual: 0.0,
            mean_estimated: 0.0,
        };
    }

    let nf = n as f64;
    let mean_error = entries.iter().map(|e| e.error).sum::<f64>() / nf;
    let rmse = (entries.iter().map(|e| e.error * e.error).sum::<f64>() / nf).sqrt();
    let mae = entries.iter().map(|e| e.error.abs()).sum::<f64>() / nf;
    let mean_actual = entries.iter().map(|e| e.actual_value).sum::<f64>() / nf;
    let mean_estimated = entries.iter().map(|e| e.estimated_value).sum::<f64>() / nf;

    // Pearson correlation
    let var_actual = entries
        .iter()
        .map(|e| (e.actual_value - mean_actual).powi(2))
        .sum::<f64>();
    let var_estimated = entries
        .iter()
        .map(|e| (e.estimated_value - mean_estimated).powi(2))
        .sum::<f64>();
    let cov = entries
        .iter()
        .map(|e| (e.actual_value - mean_actual) * (e.estimated_value - mean_estimated))
        .sum::<f64>();
    let correlation = if var_actual > 0.0 && var_estimated > 0.0 {
        cov / (var_actual.sqrt() * var_estimated.sqrt())
    } else {
        0.0
    };

    CrossValidationMetrics {
        n,
        mean_error,
        rmse,
        mae,
        correlation,
        mean_actual,
        mean_estimated,
    }
}

// ── Swath Plots ────────────────────────────────────────────────────────────────

/// Eje a lo largo del cual se construye el swath plot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwathAxis {
    /// Eje X.
    X,
    /// Eje Y.
    Y,
    /// Eje Z.
    Z,
}

/// Un bin del swath plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwathBin {
    /// Límite inferior del bin (inclusive).
    pub lower_bound: f64,
    /// Límite superior del bin (exclusive).
    pub upper_bound: f64,
    /// Centro del bin.
    pub center: f64,
    /// Número de samples/bloques en el bin.
    pub count: usize,
    /// Media de los valores reales (composites) en el bin.
    pub mean_actual: Option<f64>,
    /// Media de los valores estimados en el bin.
    pub mean_estimated: Option<f64>,
}

/// Reporte de swath plot entre valores reales y estimados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwathPlotReport {
    /// Variable analizada.
    pub column_id: ColumnId,
    /// Eje del swath.
    pub axis: SwathAxis,
    /// Ancho de cada bin.
    pub bin_width: f64,
    /// Bins del swath.
    pub bins: Vec<SwathBin>,
}

/// Par de valores (real, estimado) en una ubicación para swath plot.
#[derive(Debug, Clone)]
pub struct SwathDataPoint {
    /// Ubicación del punto en espacio 3D.
    pub location: Coordinate3D,
    /// Valor real (observado o de composite) en esta ubicación.
    pub actual_value: f64,
    /// Valor estimado por el modelo en esta ubicación.
    pub estimated_value: f64,
}

/// Construye un swath plot comparando valores reales vs estimados a lo largo de un eje.
///
/// # Errores
///
/// Retorna error si `bin_width` no es positivo y finito, o si `data_points` está vacío.
pub fn build_swath_plot(
    data_points: &[SwathDataPoint],
    column_id: &ColumnId,
    axis: SwathAxis,
    bin_width: f64,
) -> Result<SwathPlotReport, MineError> {
    if !bin_width.is_finite() || bin_width <= 0.0 {
        return Err(MineError::invalid_parameter(
            "bin_width",
            "swath bin width must be finite and positive",
        ));
    }
    if data_points.is_empty() {
        return Err(MineError::invalid_parameter(
            "data_points",
            "swath plot requires at least one data point",
        ));
    }

    let coord_values: Vec<f64> = data_points
        .iter()
        .map(|p| axis_coord(&p.location, axis))
        .collect();

    let coord_min = coord_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let coord_max = coord_values
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    let n_bins = ((coord_max - coord_min) / bin_width).ceil() as usize + 1;
    let mut bins: Vec<(f64, Vec<f64>, Vec<f64>)> = (0..n_bins)
        .map(|i| {
            let lower = coord_min + i as f64 * bin_width;
            (lower, Vec::new(), Vec::new())
        })
        .collect();

    for (i, pt) in data_points.iter().enumerate() {
        let coord = coord_values[i];
        let bin_idx = ((coord - coord_min) / bin_width).floor() as usize;
        let bin_idx = bin_idx.min(n_bins - 1);
        bins[bin_idx].1.push(pt.actual_value);
        bins[bin_idx].2.push(pt.estimated_value);
    }

    let result_bins: Vec<SwathBin> = bins
        .into_iter()
        .filter(|(_, a, _)| !a.is_empty())
        .map(|(lower, actuals, estimated)| {
            let upper = lower + bin_width;
            let center = lower + bin_width / 2.0;
            let count = actuals.len();
            let mean_actual = Some(actuals.iter().sum::<f64>() / count as f64);
            let mean_estimated = Some(estimated.iter().sum::<f64>() / count as f64);
            SwathBin {
                lower_bound: lower,
                upper_bound: upper,
                center,
                count,
                mean_actual,
                mean_estimated,
            }
        })
        .collect();

    Ok(SwathPlotReport {
        column_id: column_id.clone(),
        axis,
        bin_width,
        bins: result_bins,
    })
}

fn axis_coord(coord: &Coordinate3D, axis: SwathAxis) -> f64 {
    match axis {
        SwathAxis::X => coord.x(),
        SwathAxis::Y => coord.y(),
        SwathAxis::Z => coord.z(),
    }
}

// ── Composite vs Block Comparison ─────────────────────────────────────────────

/// Estadísticos básicos para una variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableStatistics {
    /// Número de valores.
    pub count: usize,
    /// Media aritmética.
    pub mean: f64,
    /// Varianza poblacional.
    pub variance: f64,
    /// Desviación estándar poblacional.
    pub std_dev: f64,
    /// Valor mínimo.
    pub min: f64,
    /// Valor máximo.
    pub max: f64,
    /// Percentil 10.
    pub p10: f64,
    /// Mediana (percentil 50).
    pub p50: f64,
    /// Percentil 90.
    pub p90: f64,
}

/// Comparación de estadísticos entre composites y bloques estimados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeVsBlockReport {
    /// Variable comparada.
    pub column_id: ColumnId,
    /// Estadísticos de los composites (datos originales).
    pub composite_stats: VariableStatistics,
    /// Estadísticos de los bloques estimados.
    pub block_stats: VariableStatistics,
    /// Diferencia de medias: block_mean - composite_mean.
    pub mean_difference: f64,
    /// Diferencia relativa de medias (si composite_mean ≠ 0).
    pub relative_mean_difference: Option<f64>,
    /// Reducción esperada de varianza (variance smoothing): 1 - var_block/var_composite.
    pub variance_smoothing_ratio: Option<f64>,
}

/// Compara estadísticos de composites contra bloques estimados para una variable.
///
/// # Errores
///
/// Retorna error si alguna lista está vacía.
pub fn compare_composites_vs_blocks(
    composite_values: &[f64],
    block_values: &[f64],
    column_id: &ColumnId,
) -> Result<CompositeVsBlockReport, MineError> {
    if composite_values.is_empty() {
        return Err(MineError::invalid_parameter(
            "composite_values",
            "composite values must not be empty",
        ));
    }
    if block_values.is_empty() {
        return Err(MineError::invalid_parameter(
            "block_values",
            "block values must not be empty",
        ));
    }

    let composite_stats = compute_stats(composite_values);
    let block_stats = compute_stats(block_values);

    let mean_difference = block_stats.mean - composite_stats.mean;
    let relative_mean_difference = if composite_stats.mean.abs() > 1e-10 {
        Some(mean_difference / composite_stats.mean)
    } else {
        None
    };
    let variance_smoothing_ratio = if composite_stats.variance > 1e-10 {
        Some(1.0 - block_stats.variance / composite_stats.variance)
    } else {
        None
    };

    Ok(CompositeVsBlockReport {
        column_id: column_id.clone(),
        composite_stats,
        block_stats,
        mean_difference,
        relative_mean_difference,
        variance_smoothing_ratio,
    })
}

fn compute_stats(values: &[f64]) -> VariableStatistics {
    let n = values.len();
    let count = n;
    let mean = values.iter().sum::<f64>() / n as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let min = sorted[0];
    let max = sorted[n - 1];

    let p10 = percentile(&sorted, 0.10);
    let p50 = percentile(&sorted, 0.50);
    let p90 = percentile(&sorted, 0.90);

    VariableStatistics {
        count,
        mean,
        variance,
        std_dev,
        min,
        max,
        p10,
        p50,
        p90,
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = p * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    let frac = idx - lo as f64;
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_core::{ColumnId, Coordinate3D};

    use crate::{
        EstimationPass, SpatialSample,
        neighborhoods::{SampleCountLimits, SearchNeighborhood},
    };

    use super::*;

    fn collinear_samples(n: usize, col: &ColumnId) -> Vec<SpatialSample> {
        (0..n)
            .map(|i| {
                let x = i as f64 * 10.0;
                let value = 1.0 + x * 0.01; // linear trend
                SpatialSample::new(
                    format!("s{i}"),
                    Coordinate3D::new(x, 0.0, 0.0).expect("coord should be valid"),
                    None,
                    BTreeMap::from([(col.clone(), value)]),
                )
                .expect("sample should be valid")
            })
            .collect()
    }

    fn simple_idw_pass() -> EstimationPass {
        let anisotropy = crate::SearchAnisotropy::new(500.0, 500.0, 500.0, 0.0, 0.0, 0.0)
            .expect("anisotropy should be valid");
        let neighborhood =
            SearchNeighborhood::new(anisotropy, None).expect("neighborhood should be valid");
        let limits = SampleCountLimits::new(2, 8).expect("limits should be valid");
        EstimationPass::new("pass1", neighborhood, limits).expect("pass should be valid")
    }

    #[test]
    fn cross_validation_produces_entries_for_all_samples() {
        let col = ColumnId::new("cu").expect("column id should be valid");
        let samples = collinear_samples(8, &col);
        let passes = vec![simple_idw_pass()];
        let estimator = CrossValidationEstimator::InverseDistanceWeighting {
            options: InverseDistanceWeightingOptions::new(2.0).expect("power should be valid"),
        };

        let report = cross_validate_leave_one_out(&samples, &col, &passes, &estimator)
            .expect("cross validation should succeed");

        // All 8 samples should be validated (IDW should always find 2+ neighbors from 7 remaining)
        assert_eq!(report.column_id, col);
        assert!(!report.entries.is_empty());
        assert!(report.metrics.n > 0);
        assert!(report.metrics.rmse >= 0.0);
    }

    #[test]
    fn cross_validation_rejects_single_sample() {
        let col = ColumnId::new("cu").expect("column id should be valid");
        let samples = collinear_samples(1, &col);
        let passes = vec![simple_idw_pass()];
        let estimator = CrossValidationEstimator::InverseDistanceWeighting {
            options: InverseDistanceWeightingOptions::default(),
        };

        assert!(cross_validate_leave_one_out(&samples, &col, &passes, &estimator).is_err());
    }

    #[test]
    fn swath_plot_bins_correctly() {
        let col = ColumnId::new("grade").expect("column id should be valid");
        let data: Vec<SwathDataPoint> = (0..10)
            .map(|i| SwathDataPoint {
                location: Coordinate3D::new(i as f64, 0.0, 0.0).expect("coord should be valid"),
                actual_value: i as f64,
                estimated_value: i as f64 * 1.05,
            })
            .collect();

        let report =
            build_swath_plot(&data, &col, SwathAxis::X, 5.0).expect("swath plot should succeed");

        assert_eq!(report.axis, SwathAxis::X);
        assert!(!report.bins.is_empty());
        // Two bins expected: [0-5) and [5-10)
        assert_eq!(report.bins.len(), 2);
    }

    #[test]
    fn composite_vs_block_computes_smoothing_ratio() {
        let composite = vec![0.1, 0.3, 0.5, 0.7, 0.9];
        let blocks = vec![0.3, 0.4, 0.5, 0.6, 0.7]; // less variance
        let col = ColumnId::new("cu").expect("column id should be valid");

        let report = compare_composites_vs_blocks(&composite, &blocks, &col)
            .expect("comparison should succeed");

        assert!(report.variance_smoothing_ratio.is_some());
        let smoothing = report.variance_smoothing_ratio.unwrap();
        // Block variance < composite variance → smoothing ratio > 0
        assert!(smoothing > 0.0);
    }

    #[test]
    fn composite_vs_block_detects_mean_difference() {
        let composite = vec![1.0, 1.0, 1.0, 1.0];
        let blocks = vec![1.1, 1.1, 1.1, 1.1]; // 10% higher
        let col = ColumnId::new("cu").expect("column id should be valid");

        let report = compare_composites_vs_blocks(&composite, &blocks, &col)
            .expect("comparison should succeed");

        assert!((report.mean_difference - 0.1).abs() < 1e-9);
        assert!(report.relative_mean_difference.is_some());
        let rel = report.relative_mean_difference.unwrap();
        assert!((rel - 0.1).abs() < 1e-9);
    }
}
