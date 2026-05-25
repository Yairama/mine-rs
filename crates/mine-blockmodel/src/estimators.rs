use mine_core::{ColumnId, Coordinate3D, MineError};
use serde::{Deserialize, Serialize};

use crate::{
    EstimationPass, EstimationPassEvaluation, NeighborhoodSample, SpatialSample,
    select_samples_by_estimation_passes,
};

/// Métodos deterministas de estimación disponibles en esta etapa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeterministicEstimatorKind {
    /// Usa el sample más cercano según el orden reproducible del neighborhood.
    NearestNeighbor,
    /// Promedia usando pesos inversos a la distancia con potencia explícita.
    InverseDistanceWeighting,
}

/// Opciones explícitas para inverse distance weighting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InverseDistanceWeightingOptions {
    power: f64,
}

impl InverseDistanceWeightingOptions {
    /// Construye opciones validadas para IDW.
    pub fn new(power: f64) -> Result<Self, MineError> {
        if !power.is_finite() || power <= 0.0 {
            return Err(MineError::invalid_parameter(
                "power",
                "inverse distance weighting power must be finite and greater than zero",
            ));
        }

        Ok(Self { power })
    }

    /// Potencia aplicada a la distancia.
    #[must_use]
    pub const fn power(&self) -> f64 {
        self.power
    }
}

impl Default for InverseDistanceWeightingOptions {
    fn default() -> Self {
        Self { power: 2.0 }
    }
}

/// Contribución individual usada por un estimador.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimateContribution {
    /// Índice del sample dentro de la entrada original.
    pub sample_index: usize,
    /// Identificador del sample.
    pub sample_id: String,
    /// Dominio del sample cuando existe.
    pub domain: Option<String>,
    /// Valor de la variable estimada en el sample.
    pub value: f64,
    /// Distancia euclidiana al target.
    pub euclidean_distance: f64,
    /// Distancia anisotrópica normalizada al target.
    pub anisotropic_distance: f64,
    /// Peso normalizado aportado al estimador.
    pub weight: f64,
}

/// Resultado serializable de una estimación puntual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointEstimate {
    /// Método usado para la estimación.
    pub estimator_kind: DeterministicEstimatorKind,
    /// Variable estimada.
    pub column_id: ColumnId,
    /// Pass seleccionado para construir la vecindad.
    pub selected_pass_id: String,
    /// Valor estimado.
    pub estimate: f64,
    /// Indica si se usó una coincidencia exacta a distancia cero.
    pub exact_match: bool,
    /// Cantidad de contribuciones finales.
    pub contribution_count: usize,
    /// Evaluaciones de passes en orden de prioridad.
    pub pass_evaluations: Vec<EstimationPassEvaluation>,
    /// Contribuciones usadas en el cálculo final.
    pub contributions: Vec<EstimateContribution>,
}

/// Estima una variable usando nearest neighbour sobre passes de búsqueda.
pub fn estimate_nearest_neighbor(
    target: Coordinate3D,
    samples: &[SpatialSample],
    column_id: &ColumnId,
    passes: &[EstimationPass],
) -> Result<PointEstimate, MineError> {
    let selection = resolve_estimation_selection(target, samples, passes)?;
    let selected_sample = selection.samples.first().ok_or_else(|| {
        MineError::validation("nearest neighbor selection must contain at least one sample")
    })?;
    let value = sample_value(samples, selected_sample.sample_index, column_id)?;

    Ok(PointEstimate {
        estimator_kind: DeterministicEstimatorKind::NearestNeighbor,
        column_id: column_id.clone(),
        selected_pass_id: selection.selected_pass_id,
        estimate: value,
        exact_match: selected_sample.euclidean_distance == 0.0,
        contribution_count: 1,
        pass_evaluations: selection.pass_evaluations,
        contributions: vec![EstimateContribution {
            sample_index: selected_sample.sample_index,
            sample_id: selected_sample.sample_id.clone(),
            domain: selected_sample.domain.clone(),
            value,
            euclidean_distance: selected_sample.euclidean_distance,
            anisotropic_distance: selected_sample.anisotropic_distance,
            weight: 1.0,
        }],
    })
}

/// Estima una variable usando inverse distance weighting sobre passes de búsqueda.
pub fn estimate_inverse_distance_weighting(
    target: Coordinate3D,
    samples: &[SpatialSample],
    column_id: &ColumnId,
    passes: &[EstimationPass],
    options: &InverseDistanceWeightingOptions,
) -> Result<PointEstimate, MineError> {
    let selection = resolve_estimation_selection(target, samples, passes)?;

    let exact_matches = selection
        .samples
        .iter()
        .filter(|sample| sample.euclidean_distance == 0.0)
        .cloned()
        .collect::<Vec<_>>();

    let weighted_samples = if exact_matches.is_empty() {
        let raw_weights = selection
            .samples
            .iter()
            .map(|sample| {
                (
                    sample.clone(),
                    1.0 / sample.euclidean_distance.powf(options.power()),
                )
            })
            .collect::<Vec<_>>();
        normalize_contributions(samples, column_id, &raw_weights)?
    } else {
        let exact_weight = 1.0 / exact_matches.len() as f64;
        let raw_weights = exact_matches
            .into_iter()
            .map(|sample| (sample, exact_weight))
            .collect::<Vec<_>>();
        normalize_contributions(samples, column_id, &raw_weights)?
    };

    let estimate = weighted_samples
        .iter()
        .map(|contribution| contribution.value * contribution.weight)
        .sum::<f64>();

    Ok(PointEstimate {
        estimator_kind: DeterministicEstimatorKind::InverseDistanceWeighting,
        column_id: column_id.clone(),
        selected_pass_id: selection.selected_pass_id,
        estimate,
        exact_match: weighted_samples
            .iter()
            .any(|contribution| contribution.euclidean_distance == 0.0),
        contribution_count: weighted_samples.len(),
        pass_evaluations: selection.pass_evaluations,
        contributions: weighted_samples,
    })
}

pub(crate) fn resolve_estimation_selection(
    target: Coordinate3D,
    samples: &[SpatialSample],
    passes: &[EstimationPass],
) -> Result<ResolvedEstimationSelection, MineError> {
    let selection = select_samples_by_estimation_passes(target, samples, passes)?;
    let Some(selected_pass_id) = selection.selected_pass_id else {
        return Err(MineError::validation(
            "estimation passes did not produce a neighborhood satisfying minimum sample requirements",
        ));
    };

    Ok(ResolvedEstimationSelection {
        selected_pass_id,
        pass_evaluations: selection.evaluations,
        samples: selection.samples,
    })
}

fn normalize_contributions(
    samples: &[SpatialSample],
    column_id: &ColumnId,
    weighted_samples: &[(NeighborhoodSample, f64)],
) -> Result<Vec<EstimateContribution>, MineError> {
    let total_raw_weight = weighted_samples
        .iter()
        .map(|(_, weight)| weight)
        .sum::<f64>();
    if !total_raw_weight.is_finite() || total_raw_weight <= 0.0 {
        return Err(MineError::validation(
            "estimator raw weights must sum to a finite positive value",
        ));
    }

    weighted_samples
        .iter()
        .map(|(sample, raw_weight)| {
            let value = sample_value(samples, sample.sample_index, column_id)?;
            Ok(EstimateContribution {
                sample_index: sample.sample_index,
                sample_id: sample.sample_id.clone(),
                domain: sample.domain.clone(),
                value,
                euclidean_distance: sample.euclidean_distance,
                anisotropic_distance: sample.anisotropic_distance,
                weight: raw_weight / total_raw_weight,
            })
        })
        .collect()
}

pub(crate) fn sample_value(
    samples: &[SpatialSample],
    sample_index: usize,
    column_id: &ColumnId,
) -> Result<f64, MineError> {
    let sample = samples.get(sample_index).ok_or_else(|| {
        MineError::numeric("selected sample index must remain within source slice bounds")
    })?;

    sample.values.get(column_id).copied().ok_or_else(|| {
        MineError::schema(format!(
            "spatial sample `{}` is missing value column `{column_id}`",
            sample.sample_id
        ))
    })
}

pub(crate) struct ResolvedEstimationSelection {
    pub(crate) selected_pass_id: String,
    pub(crate) pass_evaluations: Vec<EstimationPassEvaluation>,
    pub(crate) samples: Vec<NeighborhoodSample>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_core::Coordinate3D;

    use super::*;
    use crate::{SampleCountLimits, SearchAnisotropy, SearchNeighborhood};

    fn sample(point_id: &str, x: f64, value: f64, domain: Option<&str>) -> SpatialSample {
        SpatialSample::new(
            point_id,
            Coordinate3D::new(x, 0.0, 0.0).expect("coordinate should be valid"),
            domain.map(str::to_owned),
            BTreeMap::from([(
                ColumnId::new("cu").expect("column id should be valid"),
                value,
            )]),
        )
        .expect("sample should be valid")
    }

    fn pass(pass_id: &str, range: f64, min_samples: usize, max_samples: usize) -> EstimationPass {
        EstimationPass::new(
            pass_id,
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

    #[test]
    fn nearest_neighbor_uses_first_selected_sample() {
        let samples = vec![
            sample("s1", 1.0, 1.2, Some("ore")),
            sample("s2", 2.0, 2.4, Some("ore")),
        ];

        let estimate = estimate_nearest_neighbor(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &samples,
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass("primary", 3.0, 1, 2)],
        )
        .expect("estimate should work");

        assert_eq!(
            estimate.estimator_kind,
            DeterministicEstimatorKind::NearestNeighbor
        );
        assert_eq!(estimate.selected_pass_id, "primary");
        assert_eq!(estimate.contribution_count, 1);
        assert_eq!(estimate.contributions[0].sample_id, "s1");
        assert_eq!(estimate.estimate, 1.2);
    }

    #[test]
    fn inverse_distance_weighting_matches_manual_value() {
        let samples = vec![
            sample("s1", 1.0, 10.0, Some("ore")),
            sample("s2", 2.0, 22.0, Some("ore")),
        ];

        let estimate = estimate_inverse_distance_weighting(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &samples,
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass("primary", 3.0, 2, 2)],
            &InverseDistanceWeightingOptions::new(2.0).expect("options should be valid"),
        )
        .expect("estimate should work");

        assert_eq!(
            estimate.estimator_kind,
            DeterministicEstimatorKind::InverseDistanceWeighting
        );
        assert!(!estimate.exact_match);
        assert_eq!(estimate.contribution_count, 2);
        assert!((estimate.contributions[0].weight - 0.8).abs() < 1.0e-9);
        assert!((estimate.contributions[1].weight - 0.2).abs() < 1.0e-9);
        assert!((estimate.estimate - 12.4).abs() < 1.0e-9);
    }

    #[test]
    fn inverse_distance_weighting_uses_exact_matches_without_infinite_weights() {
        let samples = vec![
            sample("exact", 0.0, 5.0, Some("ore")),
            sample("far", 2.0, 50.0, Some("ore")),
        ];

        let estimate = estimate_inverse_distance_weighting(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &samples,
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass("primary", 3.0, 1, 2)],
            &InverseDistanceWeightingOptions::default(),
        )
        .expect("estimate should work");

        assert!(estimate.exact_match);
        assert_eq!(estimate.contribution_count, 1);
        assert_eq!(estimate.contributions[0].sample_id, "exact");
        assert_eq!(estimate.contributions[0].weight, 1.0);
        assert_eq!(estimate.estimate, 5.0);
    }

    #[test]
    fn estimators_error_when_no_pass_satisfies_minimum() {
        let error = estimate_nearest_neighbor(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &[sample("s1", 1.0, 1.0, Some("ore"))],
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass("primary", 1.0, 2, 2)],
        )
        .expect_err("estimate should reject missing neighborhood");

        assert_eq!(
            error,
            MineError::validation(
                "estimation passes did not produce a neighborhood satisfying minimum sample requirements"
            )
        );
    }

    #[test]
    fn estimators_require_selected_column_in_source_sample() {
        let samples = vec![
            SpatialSample::new(
                "s1",
                Coordinate3D::new(0.5, 0.0, 0.0).expect("coordinate should be valid"),
                Some("ore".to_owned()),
                BTreeMap::new(),
            )
            .expect("sample should be valid"),
        ];

        let error = estimate_nearest_neighbor(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &samples,
            &ColumnId::new("cu").expect("column id should be valid"),
            &[pass("primary", 1.0, 1, 1)],
        )
        .expect_err("estimate should reject missing value");

        assert_eq!(
            error,
            MineError::schema("spatial sample `s1` is missing value column `cu`")
        );
    }

    #[test]
    fn idw_options_reject_non_positive_power() {
        let error = InverseDistanceWeightingOptions::new(0.0)
            .expect_err("options should reject zero power");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "power",
                "inverse distance weighting power must be finite and greater than zero"
            )
        );
    }
}
