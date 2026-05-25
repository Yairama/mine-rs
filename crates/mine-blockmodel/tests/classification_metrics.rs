//! Tests de integracion para el motor de metricas de clasificacion.

use std::collections::BTreeMap;

use mine_blockmodel::{
    ClassificationMetricConfig, ClassificationThreshold, CrossValidationEntry,
    CrossValidationMetrics, CrossValidationReport, EstimationPass, SampleCountLimits,
    SearchAnisotropy, SearchNeighborhood, SpatialSample, VariogramFitSummary, VariogramModel,
    VariogramModelKind, evaluate_classification_metrics,
};
use mine_core::{ColumnId, Coordinate3D, MineError};

#[test]
fn evaluate_classification_metrics_with_explicit_thresholds() {
    let column_id = ColumnId::new("cu").expect("column should be valid");
    let samples = samples(&column_id);
    let passes = passes();
    let variogram = VariogramModel {
        column_id: column_id.clone(),
        domain: None,
        direction: None,
        model_kind: VariogramModelKind::Spherical,
        nugget: 0.05,
        partial_sill: 0.95,
        range: Some(40.0),
        fit_summary: VariogramFitSummary {
            observed_lag_count: 6,
            total_pair_count: 18,
            weighted_sse: 0.04,
            rmse: 0.15,
            mean_absolute_error: 0.12,
        },
    };
    let cross_validation = sample_cross_validation_report(&column_id);
    let config = ClassificationMetricConfig::new(vec![
        ClassificationThreshold::new(
            "measured",
            Some(12.0),
            Some(11.0),
            Some(2),
            Some(1.0),
            Some(35.0),
            Some(0.9),
            Some(0.2),
        )
        .expect("threshold should be valid"),
        ClassificationThreshold::new(
            "indicated",
            Some(20.0),
            Some(15.0),
            Some(1),
            Some(1.0),
            Some(25.0),
            Some(0.8),
            Some(0.3),
        )
        .expect("threshold should be valid"),
    ])
    .expect("config should be valid");

    let report =
        evaluate_classification_metrics(&samples, &passes, &variogram, &cross_validation, &config)
            .expect("classification metrics should evaluate");

    assert_eq!(report.column_id, column_id);
    assert_eq!(report.spacing.sample_count, 4);
    assert_eq!(report.informedness.supported_target_count, 4);
    assert_eq!(report.assessments.len(), 2);
    assert!(report.assessments[0].overall_passed);
    assert!(report.assessments[1].overall_passed);
    assert!(
        report
            .advisory_note
            .contains("do not declare compliance classes")
    );
}

#[test]
fn report_failed_checks_when_threshold_is_too_strict() {
    let column_id = ColumnId::new("cu").expect("column should be valid");
    let report = evaluate_classification_metrics(
        &samples(&column_id),
        &passes(),
        &VariogramModel {
            column_id: column_id.clone(),
            domain: None,
            direction: None,
            model_kind: VariogramModelKind::Spherical,
            nugget: 0.05,
            partial_sill: 0.95,
            range: Some(20.0),
            fit_summary: VariogramFitSummary {
                observed_lag_count: 6,
                total_pair_count: 18,
                weighted_sse: 0.2,
                rmse: 0.4,
                mean_absolute_error: 0.25,
            },
        },
        &sample_cross_validation_report(&column_id),
        &ClassificationMetricConfig::new(vec![
            ClassificationThreshold::new(
                "tight",
                Some(5.0),
                Some(5.0),
                Some(3),
                Some(1.0),
                Some(35.0),
                Some(0.98),
                Some(0.1),
            )
            .expect("threshold should be valid"),
        ])
        .expect("config should be valid"),
    )
    .expect("classification metrics should evaluate");

    assert!(!report.assessments[0].overall_passed);
    assert!(
        report.assessments[0]
            .failed_checks
            .iter()
            .any(|failure| failure.contains("p90 sample spacing"))
    );
    assert!(
        report.assessments[0]
            .failed_checks
            .iter()
            .any(|failure| failure.contains("cross-validation correlation"))
    );
}

#[test]
fn reject_thresholds_without_any_metric_criteria() {
    let error = ClassificationThreshold::new("empty", None, None, None, None, None, None, None)
        .expect_err("empty threshold should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "threshold",
            "classification threshold must define at least one metric criterion"
        )
    );
}

fn samples(column_id: &ColumnId) -> Vec<SpatialSample> {
    vec![
        sample("s1", 0.0, column_id, 1.0),
        sample("s2", 10.0, column_id, 1.1),
        sample("s3", 20.0, column_id, 0.9),
        sample("s4", 30.0, column_id, 1.2),
    ]
}

fn sample(sample_id: &str, x: f64, column_id: &ColumnId, value: f64) -> SpatialSample {
    SpatialSample::new(
        sample_id,
        Coordinate3D::new(x, 0.0, 0.0).expect("coordinate should be valid"),
        None,
        BTreeMap::from([(column_id.clone(), value)]),
    )
    .expect("sample should be valid")
}

fn passes() -> Vec<EstimationPass> {
    vec![
        EstimationPass::new(
            "primary",
            SearchNeighborhood::new(
                SearchAnisotropy::new(25.0, 25.0, 25.0, 0.0, 0.0, 0.0)
                    .expect("anisotropy should be valid"),
                None,
            )
            .expect("neighborhood should be valid"),
            SampleCountLimits::new(2, 4).expect("limits should be valid"),
        )
        .expect("pass should be valid"),
    ]
}

fn sample_cross_validation_report(column_id: &ColumnId) -> CrossValidationReport {
    CrossValidationReport {
        column_id: column_id.clone(),
        entries: vec![
            CrossValidationEntry {
                sample_id: "s1".to_owned(),
                location: Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
                actual_value: 1.0,
                estimated_value: 0.95,
                error: -0.05,
                relative_error: Some(-0.05),
            },
            CrossValidationEntry {
                sample_id: "s2".to_owned(),
                location: Coordinate3D::new(10.0, 0.0, 0.0).expect("coordinate should be valid"),
                actual_value: 1.1,
                estimated_value: 1.05,
                error: -0.05,
                relative_error: Some(-0.045454545454545456),
            },
            CrossValidationEntry {
                sample_id: "s3".to_owned(),
                location: Coordinate3D::new(20.0, 0.0, 0.0).expect("coordinate should be valid"),
                actual_value: 0.9,
                estimated_value: 0.92,
                error: 0.02,
                relative_error: Some(0.022222222222222223),
            },
            CrossValidationEntry {
                sample_id: "s4".to_owned(),
                location: Coordinate3D::new(30.0, 0.0, 0.0).expect("coordinate should be valid"),
                actual_value: 1.2,
                estimated_value: 1.15,
                error: -0.05,
                relative_error: Some(-0.04166666666666667),
            },
        ],
        metrics: CrossValidationMetrics {
            n: 4,
            mean_error: -0.0325,
            rmse: 0.05,
            mae: 0.0425,
            correlation: 0.96,
            mean_actual: 1.05,
            mean_estimated: 1.0175,
        },
    }
}
