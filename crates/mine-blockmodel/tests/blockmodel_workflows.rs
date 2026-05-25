//! Tests de integración para workflows públicos de `mine-blockmodel`.

use std::collections::BTreeMap;

use mine_blockmodel::{
    BlockModel, CellDeclusteringOptions, CellOriginOffset, ColumnData,
    CompositeDomainAuditIssueCode, CompositeResidualPolicy, CompositingOptions, DomainMask,
    ExperimentalVariogramLag, IntervalSample, SpatialSample, VariogramDirection,
    VariogramLagConfig, audit_composite_domains, build_experimental_variogram,
    build_weighted_histogram, build_weighted_statistics_report, composite_intervals,
    compute_cell_declustering_weights, experimental_variogram_from_lag_rows,
    filter_interval_samples_by_domain_mask,
};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, MetadataValue, MineError,
};

fn sample_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid")
}

fn sample_schema() -> ColumnSchemaSet {
    ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
        ColumnSchema::new(
            ColumnId::new("domain").expect("column id should be valid"),
            ColumnLogicalType::Text,
            None,
            false,
            ColumnMiningRole::Domain,
        ),
    ])
    .expect("schema should be valid")
}

fn sample_columns() -> BTreeMap<ColumnId, ColumnData> {
    BTreeMap::from([
        (
            ColumnId::new("cu").expect("column id should be valid"),
            ColumnData::Floats(vec![0.8, 1.1]),
        ),
        (
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnData::Floats(vec![12.0, 15.0]),
        ),
        (
            ColumnId::new("domain").expect("column id should be valid"),
            ColumnData::Texts(vec!["waste".to_owned(), "ore".to_owned()]),
        ),
    ])
}

fn sparse_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(3, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid")
}

fn sparse_columns() -> BTreeMap<ColumnId, ColumnData> {
    sample_columns()
}

fn assert_close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("statistic should be present");
    assert!((actual - expected).abs() < 1e-12);
}

fn assert_close_value(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12);
}

#[test]
fn select_subset_of_columns() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::from_entries(vec![(
            "source".to_owned(),
            MetadataValue::Text("synthetic".to_owned()),
        )])
        .expect("metadata should be valid"),
        sample_columns(),
    )
    .expect("block model should be valid");

    let selected = model
        .select_columns(&[ColumnId::new("cu").expect("column id should be valid")])
        .expect("selection should be valid");

    assert_eq!(selected.block_count(), 2);
    assert!(
        selected
            .column(&ColumnId::new("cu").expect("column id should be valid"))
            .is_some()
    );
    assert!(
        selected
            .column(&ColumnId::new("tonnes").expect("column id should be valid"))
            .is_none()
    );
}

#[test]
fn preserve_sparse_layout_when_selecting_columns() {
    let model = BlockModel::new_sparse(
        sparse_grid(),
        sample_schema(),
        Metadata::new(),
        vec![0, 2],
        sparse_columns(),
    )
    .expect("sparse block model should be valid");

    let selected = model
        .select_columns(&[ColumnId::new("cu").expect("column id should be valid")])
        .expect("selection should be valid");

    assert!(selected.is_sparse());
    assert_eq!(selected.linear_index_at(1).expect("row should exist"), 2);
}

#[test]
fn build_serializable_model_summary() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::from_entries(vec![(
            "source".to_owned(),
            MetadataValue::Text("synthetic".to_owned()),
        )])
        .expect("metadata should be valid"),
        sample_columns(),
    )
    .expect("block model should be valid");

    let summary = model.summary().expect("summary should be available");

    assert_eq!(summary.block_count, 2);
    assert_eq!(summary.column_count, 3);
    assert_eq!(summary.approximate_memory_bytes, 40);
    assert_eq!(summary.metadata_keys, vec!["source".to_owned()]);
    assert_eq!(summary.extent.maximum.x(), 20.0);
    assert_eq!(
        summary.columns[0].name,
        ColumnId::new("cu").expect("column id should be valid")
    );
}

#[test]
fn build_basic_statistics() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let statistics = model
        .basic_statistics(&ColumnId::new("tonnes").expect("column id should be valid"))
        .expect("statistics should be available");

    assert_eq!(statistics.block_count, 2);
    assert_eq!(statistics.total_tonnage, 27.0);
    assert_eq!(
        statistics.tonnage_column,
        ColumnId::new("tonnes").expect("column id should be valid")
    );
    assert_eq!(statistics.null_counts.len(), 3);
    assert_eq!(statistics.null_counts[0].null_count, 0);
    assert_eq!(statistics.grade_statistics.len(), 1);
    assert_close(statistics.grade_statistics[0].average_grade, 26.1 / 27.0);
    assert_close(statistics.grade_statistics[0].contained_metal, 0.261);
}

#[test]
fn build_grouped_statistics() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let statistics = model
        .grouped_statistics(
            &ColumnId::new("domain").expect("column id should be valid"),
            &ColumnId::new("tonnes").expect("column id should be valid"),
        )
        .expect("grouped statistics should be available");

    assert_eq!(statistics.len(), 2);
    assert_eq!(statistics[0].group_value, "ore");
    assert_eq!(statistics[0].block_count, 1);
    assert_eq!(statistics[0].total_tonnage, 15.0);
    assert_close(statistics[0].grade_statistics[0].average_grade, 1.1);
    assert_close(statistics[0].grade_statistics[0].contained_metal, 0.165);
    assert_eq!(statistics[1].group_value, "waste");
    assert_eq!(statistics[1].total_tonnage, 12.0);
    assert_close(statistics[1].grade_statistics[0].average_grade, 0.8);
    assert_close(statistics[1].grade_statistics[0].contained_metal, 0.096);
}

#[test]
fn reject_grouped_statistics_for_float_column() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let error = model
        .grouped_statistics(
            &ColumnId::new("cu").expect("column id should be valid"),
            &ColumnId::new("tonnes").expect("column id should be valid"),
        )
        .expect_err("float group column should fail");

    assert_eq!(
        error,
        MineError::schema("group column `cu` must be categorical (text, integer or boolean)")
    );
}

#[test]
fn build_grade_tonnage_curve() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let curve = model
        .grade_tonnage_curve(
            &ColumnId::new("cu").expect("column id should be valid"),
            &ColumnId::new("tonnes").expect("column id should be valid"),
            &[1.0, 0.0, 1.2],
        )
        .expect("curve should be available");

    assert_eq!(curve.len(), 3);
    assert_eq!(curve[0].cutoff, 0.0);
    assert_eq!(curve[0].block_count, 2);
    assert_eq!(curve[0].tonnage, 27.0);
    assert_close(curve[0].average_grade, 26.1 / 27.0);
    assert_close(curve[0].contained_metal, 0.261);
    assert_close(curve[0].tonnage_percentage, 100.0);
    assert_eq!(curve[1].cutoff, 1.0);
    assert_eq!(curve[1].block_count, 1);
    assert_eq!(curve[1].tonnage, 15.0);
    assert_close(curve[1].average_grade, 1.1);
    assert_close(curve[1].contained_metal, 0.165);
    assert_close(curve[1].tonnage_percentage, 15.0 / 27.0 * 100.0);
    assert_eq!(curve[2].cutoff, 1.2);
    assert_eq!(curve[2].block_count, 0);
    assert_eq!(curve[2].tonnage, 0.0);
    assert_eq!(curve[2].average_grade, None);
    assert_close(curve[2].contained_metal, 0.0);
    assert_close(curve[2].tonnage_percentage, 0.0);
}

#[test]
fn reject_non_finite_grade_tonnage_cutoff() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let error = model
        .grade_tonnage_curve(
            &ColumnId::new("cu").expect("column id should be valid"),
            &ColumnId::new("tonnes").expect("column id should be valid"),
            &[f64::NAN],
        )
        .expect_err("non-finite cutoff should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "cutoffs",
            "grade-tonnage cutoffs must be finite numeric values"
        )
    );
}

#[test]
fn filter_by_grade_minimum() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let selection = model
        .filter_by_float_min(
            &ColumnId::new("cu").expect("column id should be valid"),
            1.0,
        )
        .expect("grade filter should be valid");

    assert_eq!(selection.indices(), &[1]);
}

#[test]
fn filter_by_domain_text() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let selection = model
        .filter_by_text_match(
            &ColumnId::new("domain").expect("column id should be valid"),
            "ore",
        )
        .expect("domain filter should be valid");

    assert_eq!(selection.indices(), &[1]);
}

#[test]
fn filter_by_coordinate_range() {
    let model = BlockModel::new(
        sample_grid(),
        sample_schema(),
        Metadata::new(),
        sample_columns(),
    )
    .expect("block model should be valid");

    let selection = model
        .filter_by_coordinate_range(
            Coordinate3D::new(10.0, 0.0, 0.0).expect("minimum should be valid"),
            Coordinate3D::new(20.0, 10.0, 10.0).expect("maximum should be valid"),
        )
        .expect("coordinate filter should be valid");

    assert_eq!(selection.indices(), &[1]);
}

#[test]
fn filter_sparse_model_by_coordinate_range_uses_materialized_rows() {
    let model = BlockModel::new_sparse(
        sparse_grid(),
        sample_schema(),
        Metadata::new(),
        vec![0, 2],
        sparse_columns(),
    )
    .expect("sparse block model should be valid");

    let selection = model
        .filter_by_coordinate_range(
            Coordinate3D::new(20.0, 0.0, 0.0).expect("minimum should be valid"),
            Coordinate3D::new(30.0, 10.0, 10.0).expect("maximum should be valid"),
        )
        .expect("sparse coordinate filter should succeed");

    assert_eq!(selection.indices(), &[1]);
}

#[test]
fn composite_intervals_with_length_weighted_values() {
    let samples = vec![
        IntervalSample::new(
            "sample-01",
            0.0,
            1.0,
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        IntervalSample::new(
            "sample-02",
            1.0,
            2.0,
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 3.0)]),
        )
        .expect("sample should be valid"),
    ];
    let composites = composite_intervals(
        &samples,
        &CompositingOptions::new(1.5, CompositeResidualPolicy::Keep, true)
            .expect("options should be valid"),
    )
    .expect("compositing should work");

    assert_eq!(composites.len(), 2);
    assert_eq!(composites[0].from, 0.0);
    assert_eq!(composites[0].to, 1.5);
    assert!(
        (composites[0].values[&ColumnId::new("cu").expect("column id should be valid")]
            - (5.0 / 3.0))
            .abs()
            < 1e-12
    );
    assert_eq!(composites[0].contributions.len(), 2);
    assert!((composites[0].contributions[0].weight - (2.0 / 3.0)).abs() < 1e-12);
    assert_eq!(composites[1].length, 0.5);
    assert!(
        (composites[1].values[&ColumnId::new("cu").expect("column id should be valid")] - 3.0)
            .abs()
            < 1e-12
    );
}

#[test]
fn split_composites_on_domain_change() {
    let samples = vec![
        IntervalSample::new(
            "sample-01",
            0.0,
            1.0,
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        IntervalSample::new(
            "sample-02",
            1.0,
            2.0,
            Some("waste".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 3.0)]),
        )
        .expect("sample should be valid"),
    ];
    let composites = composite_intervals(
        &samples,
        &CompositingOptions::new(2.0, CompositeResidualPolicy::Keep, true)
            .expect("options should be valid"),
    )
    .expect("compositing should work");

    assert_eq!(composites.len(), 2);
    assert_eq!(composites[0].domain.as_deref(), Some("ore"));
    assert_eq!(composites[1].domain.as_deref(), Some("waste"));
}

#[test]
fn drop_residual_composite_when_policy_requires_it() {
    let samples = vec![
        IntervalSample::new(
            "sample-01",
            0.0,
            2.0,
            None,
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 2.0)]),
        )
        .expect("sample should be valid"),
    ];
    let composites = composite_intervals(
        &samples,
        &CompositingOptions::new(1.5, CompositeResidualPolicy::Drop, false)
            .expect("options should be valid"),
    )
    .expect("compositing should work");

    assert_eq!(composites.len(), 1);
    assert_eq!(composites[0].from, 0.0);
    assert_eq!(composites[0].to, 1.5);
}

#[test]
fn reject_overlapping_interval_samples() {
    let samples = vec![
        IntervalSample::new(
            "sample-01",
            0.0,
            1.0,
            None,
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        IntervalSample::new(
            "sample-02",
            0.5,
            1.5,
            None,
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 2.0)]),
        )
        .expect("sample should be valid"),
    ];
    let error = composite_intervals(
        &samples,
        &CompositingOptions::new(1.0, CompositeResidualPolicy::Keep, false)
            .expect("options should be valid"),
    )
    .expect_err("overlap should fail");

    assert_eq!(
        error,
        MineError::validation(
            "interval samples must be non-overlapping after sorting by start coordinate"
        )
    );
}

#[test]
fn filter_interval_samples_by_domain_mask_keeps_only_allowed_domains() {
    let samples = vec![
        IntervalSample::new(
            "sample-01",
            0.0,
            1.0,
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        IntervalSample::new(
            "sample-02",
            1.0,
            2.0,
            Some("waste".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 2.0)]),
        )
        .expect("sample should be valid"),
    ];
    let report = filter_interval_samples_by_domain_mask(
        &samples,
        &DomainMask::new(vec!["ore".to_owned()], false).expect("mask should be valid"),
    );

    assert_eq!(report.selected_samples.len(), 1);
    assert_eq!(report.selected_samples[0].sample_id, "sample-01");
    assert_eq!(report.excluded_sample_ids, vec!["sample-02".to_owned()]);
    assert!(report.untagged_sample_ids.is_empty());
}

#[test]
fn audit_composite_domains_flags_mixed_and_out_of_mask_domains() {
    let samples = vec![
        IntervalSample::new(
            "sample-01",
            0.0,
            1.0,
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        IntervalSample::new(
            "sample-02",
            1.0,
            2.0,
            Some("waste".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 2.0)]),
        )
        .expect("sample should be valid"),
    ];
    let composites = composite_intervals(
        &samples,
        &CompositingOptions::new(2.0, CompositeResidualPolicy::Keep, false)
            .expect("options should be valid"),
    )
    .expect("compositing should work");
    let report = audit_composite_domains(
        &samples,
        &composites,
        &DomainMask::new(vec!["ore".to_owned()], false).expect("mask should be valid"),
    )
    .expect("audit should work");

    assert_eq!(report.issues.len(), 2);
    assert_eq!(
        report.issues[0].code,
        CompositeDomainAuditIssueCode::MixedDomains
    );
    assert_eq!(
        report.issues[1].code,
        CompositeDomainAuditIssueCode::OutOfMaskDomain
    );
}

#[test]
fn audit_composite_domains_flags_untagged_contributions() {
    let samples = vec![
        IntervalSample::new(
            "sample-01",
            0.0,
            1.0,
            None,
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
    ];
    let composites = composite_intervals(
        &samples,
        &CompositingOptions::new(1.0, CompositeResidualPolicy::Keep, false)
            .expect("options should be valid"),
    )
    .expect("compositing should work");
    let report = audit_composite_domains(
        &samples,
        &composites,
        &DomainMask::new(vec!["ore".to_owned()], false).expect("mask should be valid"),
    )
    .expect("audit should work");

    assert_eq!(report.issues.len(), 1);
    assert_eq!(
        report.issues[0].code,
        CompositeDomainAuditIssueCode::UntaggedContribution
    );
}

#[test]
fn compute_cell_declustering_weights_across_multiple_origins() {
    let samples = vec![
        SpatialSample::new(
            "sample-01",
            Coordinate3D::new(0.1, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-02",
            Coordinate3D::new(0.9, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.2)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-03",
            Coordinate3D::new(1.2, 0.0, 0.0).expect("coordinate should be valid"),
            Some("waste".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 0.2)]),
        )
        .expect("sample should be valid"),
    ];
    let result = compute_cell_declustering_weights(
        &samples,
        &CellDeclusteringOptions::new(
            BlockDimensions::new(1.0, 1.0, 1.0).expect("cell size should be valid"),
            vec![
                CellOriginOffset::new(0.0, 0.0, 0.0).expect("origin should be valid"),
                CellOriginOffset::new(0.5, 0.0, 0.0).expect("origin should be valid"),
            ],
        )
        .expect("options should be valid"),
    )
    .expect("declustering should work");

    assert_eq!(result.occupied_cell_counts, vec![2, 2]);
    assert_close_value(result.normalization_factor, 2.0);
    assert_eq!(result.sample_weights.len(), 3);
    assert_close_value(result.sample_weights[0].average_cell_weight, 0.75);
    assert_close_value(result.sample_weights[0].normalized_weight, 0.375);
    assert_close_value(result.sample_weights[1].average_cell_weight, 0.5);
    assert_close_value(result.sample_weights[1].normalized_weight, 0.25);
    assert_close_value(result.sample_weights[2].average_cell_weight, 0.75);
    assert_close_value(result.sample_weights[2].normalized_weight, 0.375);
    assert_close_value(
        result
            .sample_weights
            .iter()
            .map(|weight| weight.normalized_weight)
            .sum(),
        1.0,
    );
}

#[test]
fn build_weighted_statistics_report_groups_by_domain() {
    let samples = vec![
        SpatialSample::new(
            "sample-01",
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([
                (ColumnId::new("cu").expect("column id should be valid"), 1.0),
                (ColumnId::new("au").expect("column id should be valid"), 0.2),
            ]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-02",
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([
                (ColumnId::new("cu").expect("column id should be valid"), 2.0),
                (ColumnId::new("au").expect("column id should be valid"), 0.4),
            ]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-03",
            Coordinate3D::new(2.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("waste".to_owned()),
            BTreeMap::from([
                (ColumnId::new("cu").expect("column id should be valid"), 0.1),
                (ColumnId::new("au").expect("column id should be valid"), 0.0),
            ]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-04",
            Coordinate3D::new(3.0, 0.0, 0.0).expect("coordinate should be valid"),
            None,
            BTreeMap::from([
                (ColumnId::new("cu").expect("column id should be valid"), 0.5),
                (ColumnId::new("au").expect("column id should be valid"), 0.1),
            ]),
        )
        .expect("sample should be valid"),
    ];
    let weights = vec![
        mine_blockmodel::DeclusteredSampleWeight {
            sample_id: "sample-01".to_owned(),
            average_cell_weight: 0.25,
            normalized_weight: 0.25,
        },
        mine_blockmodel::DeclusteredSampleWeight {
            sample_id: "sample-02".to_owned(),
            average_cell_weight: 0.35,
            normalized_weight: 0.35,
        },
        mine_blockmodel::DeclusteredSampleWeight {
            sample_id: "sample-03".to_owned(),
            average_cell_weight: 0.30,
            normalized_weight: 0.30,
        },
        mine_blockmodel::DeclusteredSampleWeight {
            sample_id: "sample-04".to_owned(),
            average_cell_weight: 0.10,
            normalized_weight: 0.10,
        },
    ];
    let report =
        build_weighted_statistics_report(&samples, &weights).expect("statistics should work");

    assert_eq!(report.overall.sample_count, 4);
    assert_close_value(report.overall.total_weight, 1.0);
    assert_eq!(report.domains.len(), 2);
    assert_eq!(report.domains[0].domain, "ore");
    assert_close_value(report.domains[0].total_weight, 0.60);
    let ore_cu = report.domains[0]
        .variables
        .iter()
        .find(|variable| {
            variable.column_id == ColumnId::new("cu").expect("column id should be valid")
        })
        .expect("cu summary should exist");
    assert_close_value(
        ore_cu.weighted_mean.expect("mean should exist"),
        (1.0 * 0.25 + 2.0 * 0.35) / 0.60,
    );
    assert_eq!(report.domains[1].domain, "waste");
    assert_close_value(report.domains[1].total_weight, 0.30);
    assert_eq!(report.untagged_sample_ids, vec!["sample-04".to_owned()]);
}

#[test]
fn build_weighted_histogram_for_domain_selection() {
    let samples = vec![
        SpatialSample::new(
            "sample-01",
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 0.6)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-02",
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.4)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-03",
            Coordinate3D::new(2.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("waste".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.8)]),
        )
        .expect("sample should be valid"),
    ];
    let weights = vec![
        mine_blockmodel::DeclusteredSampleWeight {
            sample_id: "sample-01".to_owned(),
            average_cell_weight: 0.2,
            normalized_weight: 0.2,
        },
        mine_blockmodel::DeclusteredSampleWeight {
            sample_id: "sample-02".to_owned(),
            average_cell_weight: 0.5,
            normalized_weight: 0.5,
        },
        mine_blockmodel::DeclusteredSampleWeight {
            sample_id: "sample-03".to_owned(),
            average_cell_weight: 0.3,
            normalized_weight: 0.3,
        },
    ];
    let histogram = build_weighted_histogram(
        &samples,
        &weights,
        &ColumnId::new("cu").expect("column id should be valid"),
        &[0.0, 1.0, 2.0],
        Some("ore"),
    )
    .expect("histogram should work");

    assert_eq!(histogram.domain, Some("ore".to_owned()));
    assert_close_value(histogram.total_weight, 0.7);
    assert_close_value(histogram.underflow_weight, 0.0);
    assert_close_value(histogram.overflow_weight, 0.0);
    assert_eq!(histogram.bins.len(), 2);
    assert_eq!(histogram.bins[0].sample_count, 1);
    assert_close_value(histogram.bins[0].total_weight, 0.2);
    assert_eq!(histogram.bins[1].sample_count, 1);
    assert_close_value(histogram.bins[1].total_weight, 0.5);
}

#[test]
fn build_experimental_variogram_bins_pairs_by_lag() {
    let samples = vec![
        SpatialSample::new(
            "sample-01",
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 0.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-02",
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-03",
            Coordinate3D::new(2.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 2.0)]),
        )
        .expect("sample should be valid"),
    ];
    let variogram = build_experimental_variogram(
        &samples,
        &ColumnId::new("cu").expect("column id should be valid"),
        &VariogramLagConfig::new(1.0, 2, 0.1).expect("lag config should be valid"),
        None,
        Some("ore"),
    )
    .expect("variogram should work");

    assert_eq!(variogram.sample_count, 3);
    assert_eq!(variogram.domain, Some("ore".to_owned()));
    assert_eq!(variogram.lags.len(), 2);
    assert_eq!(
        variogram.lags[0],
        ExperimentalVariogramLag {
            lag_index: 1,
            lag_center: 1.0,
            pair_count: 2,
            average_distance: Some(1.0),
            semivariance: Some(0.5),
        }
    );
    assert_eq!(
        variogram.lags[1],
        ExperimentalVariogramLag {
            lag_index: 2,
            lag_center: 2.0,
            pair_count: 1,
            average_distance: Some(2.0),
            semivariance: Some(2.0),
        }
    );
}

#[test]
fn build_directional_experimental_variogram_filters_by_azimuth() {
    let samples = vec![
        SpatialSample::new(
            "sample-01",
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 0.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-02",
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-03",
            Coordinate3D::new(0.0, 1.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 3.0)]),
        )
        .expect("sample should be valid"),
    ];
    let variogram = build_experimental_variogram(
        &samples,
        &ColumnId::new("cu").expect("column id should be valid"),
        &VariogramLagConfig::new(1.0, 1, 0.1).expect("lag config should be valid"),
        Some(
            &VariogramDirection::new(
                Coordinate3D::new(1.0, 0.0, 0.0).expect("direction should be valid"),
                10.0,
                Some(0.25),
            )
            .expect("direction should be valid"),
        ),
        Some("ore"),
    )
    .expect("variogram should work");

    assert!(variogram.direction.is_some());
    assert_eq!(variogram.lags[0].pair_count, 1);
    assert_close(variogram.lags[0].average_distance, 1.0);
    assert_close(variogram.lags[0].semivariance, 0.5);
}

#[test]
fn roundtrip_experimental_variogram_through_lag_rows() {
    let samples = vec![
        SpatialSample::new(
            "sample-01",
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 0.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-02",
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-03",
            Coordinate3D::new(0.0, 1.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column id should be valid"), 3.0)]),
        )
        .expect("sample should be valid"),
    ];
    let variogram = build_experimental_variogram(
        &samples,
        &ColumnId::new("cu").expect("column id should be valid"),
        &VariogramLagConfig::new(1.0, 1, 0.1).expect("lag config should be valid"),
        Some(
            &VariogramDirection::new(
                Coordinate3D::new(1.0, 0.0, 0.0).expect("direction should be valid"),
                10.0,
                Some(0.25),
            )
            .expect("direction should be valid"),
        ),
        Some("ore"),
    )
    .expect("variogram should work");

    let roundtrip = experimental_variogram_from_lag_rows(&variogram.lag_rows())
        .expect("lag row roundtrip should work");

    assert_eq!(roundtrip, variogram);
}
