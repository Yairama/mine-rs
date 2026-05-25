use std::collections::BTreeMap;

use mine_blockmodel::{
    EstimationPass, ExperimentalVariogram, ExperimentalVariogramLag, SampleCountLimits,
    SearchAnisotropy, SearchNeighborhood, SequentialGaussianSimulationOptions,
    SequentialIndicatorSimulationOptions, SimulationTarget, SpatialSample, VariogramFitSummary,
    VariogramLagConfig, VariogramModel, VariogramModelKind, generate_sequential_gaussian_ensemble,
    generate_sequential_indicator_ensemble,
};
use mine_core::{ArtifactId, ColumnId, Coordinate3D, MetadataValue, ModelId};

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

fn target(target_id: &str, x: f64) -> SimulationTarget {
    SimulationTarget::new(
        target_id,
        Coordinate3D::new(x, 0.0, 0.0).expect("coordinate should be valid"),
        Some("ore".to_owned()),
    )
    .expect("target should be valid")
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
fn sequential_gaussian_ensemble_is_reproducible_for_same_seeds() {
    let base_model_id = ModelId::new("synthetic-model").expect("model id should be valid");
    let samples = vec![
        sample("s1", 0.0, 0.9),
        sample("s2", 1.0, 1.3),
        sample("s3", 2.0, 1.8),
    ];
    let targets = vec![target("t1", 0.5), target("t2", 1.5)];
    let passes = vec![pass(3.0, 2, 3)];
    let variogram = variogram_model();
    let seeds = vec![7, 17];
    let options = SequentialGaussianSimulationOptions::new(1.2).expect("options should be valid");

    let first = generate_sequential_gaussian_ensemble(
        ArtifactId::new("sgs-ensemble").expect("artifact id should be valid"),
        base_model_id.clone(),
        ColumnId::new("cu").expect("column id should be valid"),
        ArtifactId::new("grid.synthetic").expect("artifact id should be valid"),
        vec![ArtifactId::new("samples.synthetic").expect("artifact id should be valid")],
        &samples,
        &targets,
        &passes,
        &variogram,
        &seeds,
        &options,
    )
    .expect("sgs should work");

    let second = generate_sequential_gaussian_ensemble(
        ArtifactId::new("sgs-ensemble").expect("artifact id should be valid"),
        base_model_id,
        ColumnId::new("cu").expect("column id should be valid"),
        ArtifactId::new("grid.synthetic").expect("artifact id should be valid"),
        vec![ArtifactId::new("samples.synthetic").expect("artifact id should be valid")],
        &samples,
        &targets,
        &passes,
        &variogram,
        &seeds,
        &options,
    )
    .expect("sgs should work");

    assert_eq!(first, second);
    assert_eq!(first.descriptor.realization_count(), 2);
    assert_eq!(first.realizations[0].descriptor.random_seed(), 7);
    assert_eq!(first.realizations[1].descriptor.random_seed(), 17);
    for realization in &first.realizations {
        assert!(realization.summary.min_value <= realization.summary.mean_value);
        assert!(realization.summary.mean_value <= realization.summary.max_value);
        assert_eq!(realization.summary.node_count, targets.len());
    }
}

#[test]
fn sequential_indicator_ensemble_emits_binary_values_and_tracks_metadata() {
    let ensemble = generate_sequential_indicator_ensemble(
        ArtifactId::new("sis-ensemble").expect("artifact id should be valid"),
        ModelId::new("synthetic-model").expect("model id should be valid"),
        ColumnId::new("cu").expect("column id should be valid"),
        ArtifactId::new("grid.synthetic").expect("artifact id should be valid"),
        vec![ArtifactId::new("samples.synthetic").expect("artifact id should be valid")],
        &[
            sample("s1", 0.0, 0.4),
            sample("s2", 1.0, 1.2),
            sample("s3", 2.0, 1.8),
        ],
        &[target("t1", 0.5), target("t2", 1.5)],
        &[pass(3.0, 2, 3)],
        &variogram_model(),
        &[11],
        &SequentialIndicatorSimulationOptions::new(1.0).expect("options should be valid"),
    )
    .expect("sis should work");

    let realization = &ensemble.realizations[0];
    let average = realization
        .values
        .iter()
        .map(|value| value.value)
        .sum::<f64>()
        / realization.values.len() as f64;

    assert_eq!(ensemble.descriptor.realization_count(), 1);
    assert_eq!(
        realization.descriptor.method(),
        "sis-prototype-simple-kriging"
    );
    assert!(
        realization
            .values
            .iter()
            .all(|value| value.value == 0.0 || value.value == 1.0)
    );
    assert_eq!(realization.summary.mean_value, average);
    assert!(matches!(
        realization.descriptor.metadata().get("cutoff"),
        Some(MetadataValue::Float(value)) if (*value - 1.0).abs() < 1.0e-12
    ));
    assert!(matches!(
        realization.descriptor.metadata().get("indicator_mean"),
        Some(MetadataValue::Float(value)) if (*value - (2.0 / 3.0)).abs() < 1.0e-12
    ));
}
