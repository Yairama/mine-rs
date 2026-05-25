use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

use mine_core::{ColumnId, Coordinate3D, MineError};

use crate::{
    EstimateContribution, EstimationPass, SpatialSample, VariogramModel,
    estimators::{ResolvedEstimationSelection, resolve_estimation_selection, sample_value},
};

/// Métodos de kriging soportados en esta etapa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KrigingEstimatorKind {
    /// Ordinary kriging con restricción de suma de pesos igual a uno.
    Ordinary,
    /// Simple kriging con media conocida explícita.
    Simple,
}

/// Opciones explícitas para simple kriging.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimpleKrigingOptions {
    known_mean: f64,
}

impl SimpleKrigingOptions {
    /// Construye opciones validadas de simple kriging.
    pub fn new(known_mean: f64) -> Result<Self, MineError> {
        if !known_mean.is_finite() {
            return Err(MineError::invalid_parameter(
                "known_mean",
                "simple kriging known_mean must be finite",
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

/// Resultado serializable de kriging puntual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KrigingEstimate {
    /// Método de kriging usado.
    pub estimator_kind: KrigingEstimatorKind,
    /// Variable estimada.
    pub column_id: ColumnId,
    /// Pass seleccionado.
    pub selected_pass_id: String,
    /// Valor estimado.
    pub estimate: f64,
    /// Varianza de kriging.
    pub kriging_variance: f64,
    /// Indica si se resolvió por coincidencia exacta.
    pub exact_match: bool,
    /// Media conocida cuando aplica a simple kriging.
    pub known_mean: Option<f64>,
    /// Multiplicador de Lagrange cuando aplica a ordinary kriging.
    pub lagrange_multiplier: Option<f64>,
    /// Cantidad de contribuciones finales.
    pub contribution_count: usize,
    /// Evaluaciones de passes en orden de prioridad.
    pub pass_evaluations: Vec<crate::EstimationPassEvaluation>,
    /// Contribuciones y pesos finales.
    pub contributions: Vec<EstimateContribution>,
}

/// Estima un target usando ordinary kriging.
pub fn estimate_ordinary_kriging(
    target: Coordinate3D,
    samples: &[SpatialSample],
    column_id: &ColumnId,
    passes: &[EstimationPass],
    variogram_model: &VariogramModel,
) -> Result<KrigingEstimate, MineError> {
    validate_kriging_model(column_id, variogram_model)?;
    let selection = resolve_estimation_selection(target, samples, passes)?;

    let exact_matches = selection
        .samples
        .iter()
        .filter(|sample| sample.euclidean_distance == 0.0)
        .cloned()
        .collect::<Vec<_>>();
    if !exact_matches.is_empty() {
        return build_exact_match_estimate(
            KrigingEstimatorKind::Ordinary,
            column_id,
            &selection,
            samples,
            &exact_matches,
            None,
        );
    }

    let mut point_system =
        build_point_kriging_data(samples, column_id, variogram_model, &selection)?;
    let sample_count = point_system.contributions.len();
    let mut matrix = DMatrix::<f64>::zeros(sample_count + 1, sample_count + 1);
    let mut rhs = DVector::<f64>::zeros(sample_count + 1);

    for row in 0..sample_count {
        rhs[row] = point_system.target_covariances[row];
        matrix[(row, sample_count)] = 1.0;
        matrix[(sample_count, row)] = 1.0;
        for column in 0..sample_count {
            matrix[(row, column)] = point_system.sample_covariances[row][column];
        }
    }
    rhs[sample_count] = 1.0;

    let solution = matrix.lu().solve(&rhs).ok_or_else(|| {
        MineError::validation("ordinary kriging system is singular for the selected neighborhood")
    })?;
    let lagrange_multiplier = solution[sample_count];
    let mut estimate = 0.0;
    let mut covariance_term = 0.0;
    for index in 0..sample_count {
        let weight = solution[index];
        point_system.contributions[index].weight = weight;
        estimate += weight * point_system.contributions[index].value;
        covariance_term += weight * point_system.target_covariances[index];
    }

    Ok(KrigingEstimate {
        estimator_kind: KrigingEstimatorKind::Ordinary,
        column_id: column_id.clone(),
        selected_pass_id: selection.selected_pass_id,
        estimate,
        kriging_variance: normalize_variance(
            point_system.total_sill - covariance_term - lagrange_multiplier,
        )?,
        exact_match: false,
        known_mean: None,
        lagrange_multiplier: Some(lagrange_multiplier),
        contribution_count: point_system.contributions.len(),
        pass_evaluations: selection.pass_evaluations,
        contributions: point_system.contributions,
    })
}

/// Estima un target usando simple kriging con media conocida explícita.
pub fn estimate_simple_kriging(
    target: Coordinate3D,
    samples: &[SpatialSample],
    column_id: &ColumnId,
    passes: &[EstimationPass],
    variogram_model: &VariogramModel,
    options: &SimpleKrigingOptions,
) -> Result<KrigingEstimate, MineError> {
    validate_kriging_model(column_id, variogram_model)?;
    let selection = resolve_estimation_selection(target, samples, passes)?;

    let exact_matches = selection
        .samples
        .iter()
        .filter(|sample| sample.euclidean_distance == 0.0)
        .cloned()
        .collect::<Vec<_>>();
    if !exact_matches.is_empty() {
        return build_exact_match_estimate(
            KrigingEstimatorKind::Simple,
            column_id,
            &selection,
            samples,
            &exact_matches,
            Some(options.known_mean()),
        );
    }

    let mut point_system =
        build_point_kriging_data(samples, column_id, variogram_model, &selection)?;
    let sample_count = point_system.contributions.len();
    let mut matrix = DMatrix::<f64>::zeros(sample_count, sample_count);
    let mut rhs = DVector::<f64>::zeros(sample_count);

    for row in 0..sample_count {
        rhs[row] = point_system.target_covariances[row];
        for column in 0..sample_count {
            matrix[(row, column)] = point_system.sample_covariances[row][column];
        }
    }

    let solution = matrix.lu().solve(&rhs).ok_or_else(|| {
        MineError::validation("simple kriging system is singular for the selected neighborhood")
    })?;
    let mut estimate = options.known_mean();
    let mut covariance_term = 0.0;
    for index in 0..sample_count {
        let weight = solution[index];
        point_system.contributions[index].weight = weight;
        estimate += weight * (point_system.contributions[index].value - options.known_mean());
        covariance_term += weight * point_system.target_covariances[index];
    }

    Ok(KrigingEstimate {
        estimator_kind: KrigingEstimatorKind::Simple,
        column_id: column_id.clone(),
        selected_pass_id: selection.selected_pass_id,
        estimate,
        kriging_variance: normalize_variance(point_system.total_sill - covariance_term)?,
        exact_match: false,
        known_mean: Some(options.known_mean()),
        lagrange_multiplier: None,
        contribution_count: point_system.contributions.len(),
        pass_evaluations: selection.pass_evaluations,
        contributions: point_system.contributions,
    })
}

fn build_exact_match_estimate(
    estimator_kind: KrigingEstimatorKind,
    column_id: &ColumnId,
    selection: &ResolvedEstimationSelection,
    samples: &[SpatialSample],
    exact_matches: &[crate::NeighborhoodSample],
    known_mean: Option<f64>,
) -> Result<KrigingEstimate, MineError> {
    let weight = 1.0 / exact_matches.len() as f64;
    let contributions = exact_matches
        .iter()
        .map(|sample| {
            Ok(EstimateContribution {
                sample_index: sample.sample_index,
                sample_id: sample.sample_id.clone(),
                domain: sample.domain.clone(),
                value: sample_value(samples, sample.sample_index, column_id)?,
                euclidean_distance: sample.euclidean_distance,
                anisotropic_distance: sample.anisotropic_distance,
                weight,
            })
        })
        .collect::<Result<Vec<_>, MineError>>()?;
    let estimate = contributions
        .iter()
        .map(|contribution| contribution.value * contribution.weight)
        .sum::<f64>();

    Ok(KrigingEstimate {
        estimator_kind,
        column_id: column_id.clone(),
        selected_pass_id: selection.selected_pass_id.clone(),
        estimate,
        kriging_variance: 0.0,
        exact_match: true,
        known_mean,
        lagrange_multiplier: None,
        contribution_count: contributions.len(),
        pass_evaluations: selection.pass_evaluations.clone(),
        contributions,
    })
}

fn build_point_kriging_data(
    samples: &[SpatialSample],
    column_id: &ColumnId,
    variogram_model: &VariogramModel,
    selection: &ResolvedEstimationSelection,
) -> Result<PointKrigingData, MineError> {
    let mut contributions = Vec::with_capacity(selection.samples.len());
    let mut target_covariances = Vec::with_capacity(selection.samples.len());
    let mut sample_covariances = vec![vec![0.0; selection.samples.len()]; selection.samples.len()];

    for (row, selected) in selection.samples.iter().enumerate() {
        contributions.push(EstimateContribution {
            sample_index: selected.sample_index,
            sample_id: selected.sample_id.clone(),
            domain: selected.domain.clone(),
            value: sample_value(samples, selected.sample_index, column_id)?,
            euclidean_distance: selected.euclidean_distance,
            anisotropic_distance: selected.anisotropic_distance,
            weight: 0.0,
        });
        target_covariances.push(covariance_from_model(
            variogram_model,
            selected.euclidean_distance,
        )?);

        for (column, other) in selection.samples.iter().enumerate() {
            let left = samples.get(selected.sample_index).ok_or_else(|| {
                MineError::numeric("selected sample index must remain within source slice bounds")
            })?;
            let right = samples.get(other.sample_index).ok_or_else(|| {
                MineError::numeric("selected sample index must remain within source slice bounds")
            })?;
            sample_covariances[row][column] = covariance_from_model(
                variogram_model,
                euclidean_distance(left.location, right.location),
            )?;
        }
    }

    Ok(PointKrigingData {
        total_sill: variogram_model.total_sill(),
        contributions,
        target_covariances,
        sample_covariances,
    })
}

fn covariance_from_model(
    variogram_model: &VariogramModel,
    distance: f64,
) -> Result<f64, MineError> {
    Ok(variogram_model.total_sill() - variogram_model.semivariance(distance)?)
}

fn euclidean_distance(left: Coordinate3D, right: Coordinate3D) -> f64 {
    let dx = right.x() - left.x();
    let dy = right.y() - left.y();
    let dz = right.z() - left.z();
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn normalize_variance(variance: f64) -> Result<f64, MineError> {
    if !variance.is_finite() {
        return Err(MineError::numeric(
            "kriging variance must stay finite for the selected neighborhood",
        ));
    }
    if variance < -1.0e-9 {
        return Err(MineError::numeric(
            "kriging variance became negative beyond numeric tolerance",
        ));
    }

    Ok(variance.max(0.0))
}

fn validate_kriging_model(
    column_id: &ColumnId,
    variogram_model: &VariogramModel,
) -> Result<(), MineError> {
    if &variogram_model.column_id != column_id {
        return Err(MineError::validation(format!(
            "kriging variogram model column `{}` does not match requested column `{column_id}`",
            variogram_model.column_id
        )));
    }

    Ok(())
}

struct PointKrigingData {
    total_sill: f64,
    contributions: Vec<EstimateContribution>,
    target_covariances: Vec<f64>,
    sample_covariances: Vec<Vec<f64>>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_core::Coordinate3D;

    use super::*;
    use crate::{
        ExperimentalVariogram, ExperimentalVariogramLag, SampleCountLimits, SearchAnisotropy,
        SearchNeighborhood, VariogramFitSummary, VariogramLagConfig, VariogramModelKind,
    };

    fn sample(point_id: &str, x: f64, value: f64) -> SpatialSample {
        SpatialSample::new(
            point_id,
            Coordinate3D::new(x, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(
                ColumnId::new("cu").expect("column id should be valid"),
                value,
            )]),
        )
        .expect("sample should be valid")
    }

    fn pass(range: f64, min_samples: usize, max_samples: usize) -> EstimationPass {
        EstimationPass::new(
            "primary",
            SearchNeighborhood::new(
                SearchAnisotropy::new(range, range, range, 0.0, 0.0, 0.0)
                    .expect("anisotropy should be valid"),
                Some(vec!["ore".to_owned()]),
            )
            .expect("neighborhood should be valid"),
            SampleCountLimits::new(min_samples, max_samples).expect("limits should be valid"),
        )
        .expect("pass should be valid")
    }

    fn variogram_model() -> VariogramModel {
        let variogram = ExperimentalVariogram {
            column_id: ColumnId::new("cu").expect("column id should be valid"),
            domain: Some("ore".to_owned()),
            direction: None,
            lag_config: VariogramLagConfig::new(1.0, 2, 0.1).expect("lag config should be valid"),
            sample_count: 3,
            lags: vec![
                ExperimentalVariogramLag {
                    lag_index: 1,
                    lag_center: 1.0,
                    pair_count: 2,
                    average_distance: Some(1.0),
                    semivariance: Some(0.3671875),
                },
                ExperimentalVariogramLag {
                    lag_index: 2,
                    lag_center: 2.0,
                    pair_count: 1,
                    average_distance: Some(2.0),
                    semivariance: Some(0.6875),
                },
            ],
        };

        VariogramModel::from_variogram(
            &variogram,
            VariogramModelKind::Spherical,
            0.0,
            1.0,
            Some(4.0),
            VariogramFitSummary {
                observed_lag_count: 2,
                total_pair_count: 3,
                weighted_sse: 0.0,
                rmse: 0.0,
                mean_absolute_error: 0.0,
            },
        )
        .expect("variogram model should be valid")
    }

    #[test]
    fn simple_kriging_with_single_sample_matches_manual_solution() {
        let estimate = estimate_simple_kriging(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &[sample("s1", 1.0, 10.0)],
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass(2.0, 1, 1)],
            &variogram_model(),
            &SimpleKrigingOptions::new(0.0).expect("options should be valid"),
        )
        .expect("kriging should work");

        assert_eq!(estimate.estimator_kind, KrigingEstimatorKind::Simple);
        assert!((estimate.contributions[0].weight - 0.6328125).abs() < 1.0e-9);
        assert!((estimate.estimate - 6.328125).abs() < 1.0e-9);
        assert!((estimate.kriging_variance - 0.59954833984375).abs() < 1.0e-9);
        assert_eq!(estimate.known_mean, Some(0.0));
    }

    #[test]
    fn ordinary_kriging_returns_symmetric_average() {
        let estimate = estimate_ordinary_kriging(
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            &[sample("left", 0.0, 1.0), sample("right", 2.0, 3.0)],
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass(2.0, 2, 2)],
            &variogram_model(),
        )
        .expect("kriging should work");

        assert_eq!(estimate.estimator_kind, KrigingEstimatorKind::Ordinary);
        assert!((estimate.contributions[0].weight - 0.5).abs() < 1.0e-9);
        assert!((estimate.contributions[1].weight - 0.5).abs() < 1.0e-9);
        assert!((estimate.estimate - 2.0).abs() < 1.0e-9);
        assert!(estimate.lagrange_multiplier.is_some());
        assert!(estimate.kriging_variance >= 0.0);
    }

    #[test]
    fn ordinary_kriging_cross_validation_reproduces_middle_sample_on_symmetric_fixture() {
        let estimate = estimate_ordinary_kriging(
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            &[sample("left", 0.0, 1.0), sample("right", 2.0, 3.0)],
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass(2.0, 2, 2)],
            &variogram_model(),
        )
        .expect("kriging should work");

        assert!((estimate.estimate - 2.0).abs() < 1.0e-9);
    }

    #[test]
    fn kriging_uses_exact_match_path_when_target_coincides_with_sample() {
        let estimate = estimate_ordinary_kriging(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &[sample("exact", 0.0, 7.5), sample("far", 2.0, 10.0)],
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass(2.0, 1, 2)],
            &variogram_model(),
        )
        .expect("kriging should work");

        assert!(estimate.exact_match);
        assert_eq!(estimate.kriging_variance, 0.0);
        assert_eq!(estimate.contribution_count, 1);
        assert_eq!(estimate.estimate, 7.5);
    }

    #[test]
    fn kriging_rejects_variogram_model_for_different_column() {
        let error = estimate_ordinary_kriging(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &[sample("s1", 1.0, 1.0)],
            &ColumnId::new("au").expect("column id should be valid"),
            &[pass(2.0, 1, 1)],
            &variogram_model(),
        )
        .expect_err("kriging should reject model mismatch");

        assert_eq!(
            error,
            MineError::validation(
                "kriging variogram model column `cu` does not match requested column `au`"
            )
        );
    }

    #[test]
    fn simple_kriging_requires_finite_mean() {
        let error = SimpleKrigingOptions::new(f64::NAN)
            .expect_err("simple kriging should reject non-finite mean");

        assert_eq!(
            error,
            MineError::invalid_parameter("known_mean", "simple kriging known_mean must be finite")
        );
    }
}
