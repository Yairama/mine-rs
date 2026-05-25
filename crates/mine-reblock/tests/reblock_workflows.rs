//! Tests de integración para workflows públicos de `mine-reblock`.

use std::collections::BTreeMap;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, MineError,
};
use mine_reblock::{
    AdaptiveResolutionStrategy, AdaptiveZoneRule, AggregationRule, AggregationRules,
    DistributionRule, DistributionRules, ReconciliationTolerances,
    build_adaptive_reblock_prototype, reconcile_models, subblock, superblock,
};

#[test]
fn superblock_preserves_tonnage_and_contained_metal() {
    let rules = AggregationRules::new(vec![
        AggregationRule::sum(
            ColumnId::new("tonnes_total").expect("column should be valid"),
            ColumnId::new("tonnes").expect("column should be valid"),
        ),
        AggregationRule::weighted_average(
            ColumnId::new("cu_avg").expect("column should be valid"),
            ColumnId::new("cu").expect("column should be valid"),
            ColumnId::new("tonnes").expect("column should be valid"),
        ),
        AggregationRule::majority(
            ColumnId::new("domain_mode").expect("column should be valid"),
            ColumnId::new("domain").expect("column should be valid"),
        ),
    ])
    .expect("rules should be valid");
    let target_grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(20.0, 10.0, 20.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("target grid should be valid");

    let rebloqued = superblock(&sample_superblock_model(), target_grid, &rules)
        .expect("superblock should succeed");

    assert_eq!(rebloqued.block_count(), 1);
    assert!(!rebloqued.is_sparse());

    let tonnes = match rebloqued
        .column(&ColumnId::new("tonnes_total").expect("column should be valid"))
        .expect("tonnes column should exist")
    {
        ColumnData::Floats(values) => values[0],
        other => panic!("unexpected tonnes column type: {other:?}"),
    };
    let cu = match rebloqued
        .column(&ColumnId::new("cu_avg").expect("column should be valid"))
        .expect("grade column should exist")
    {
        ColumnData::Floats(values) => values[0],
        other => panic!("unexpected grade column type: {other:?}"),
    };
    let domain = match rebloqued
        .column(&ColumnId::new("domain_mode").expect("column should be valid"))
        .expect("domain column should exist")
    {
        ColumnData::Texts(values) => values[0].clone(),
        other => panic!("unexpected domain column type: {other:?}"),
    };

    assert_eq!(tonnes, 100.0);
    assert_eq!(cu, 3.0);
    assert_eq!(tonnes * cu, 300.0);
    assert_eq!(domain, "ore");
}

#[test]
fn superblock_emits_sparse_target_when_some_cells_are_empty() {
    let rules = AggregationRules::new(vec![AggregationRule::sum(
        ColumnId::new("tonnes_total").expect("column should be valid"),
        ColumnId::new("tonnes").expect("column should be valid"),
    )])
    .expect("rules should be valid");
    let target_grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(20.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("target grid should be valid");

    let rebloqued = superblock(&sample_sparse_superblock_model(), target_grid, &rules)
        .expect("superblock should succeed");

    assert!(rebloqued.is_sparse());
    assert_eq!(rebloqued.block_count(), 1);
    assert_eq!(rebloqued.linear_index_at(0).expect("row should exist"), 0);

    let tonnes = match rebloqued
        .column(&ColumnId::new("tonnes_total").expect("column should be valid"))
        .expect("tonnes column should exist")
    {
        ColumnData::Floats(values) => values[0],
        other => panic!("unexpected tonnes column type: {other:?}"),
    };

    assert_eq!(tonnes, 30.0);
}

#[test]
fn superblock_rejects_incompatible_target_grid() {
    let rules = AggregationRules::new(vec![AggregationRule::sum(
        ColumnId::new("tonnes_total").expect("column should be valid"),
        ColumnId::new("tonnes").expect("column should be valid"),
    )])
    .expect("rules should be valid");
    let incompatible_target_grid = GridDefinition::new(
        Coordinate3D::new(5.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(20.0, 10.0, 20.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("target grid should be valid");

    let error = superblock(&sample_superblock_model(), incompatible_target_grid, &rules)
        .expect_err("misaligned grid should fail");

    assert_eq!(
        error,
        MineError::grid("superblock requires source and target grids to share the same origin")
    );
}

#[test]
fn subblock_splits_conservative_values_and_replicates_attributes() {
    let rules = DistributionRules::new(vec![
        DistributionRule::split_equally(
            ColumnId::new("tonnes").expect("column should be valid"),
            ColumnId::new("tonnes").expect("column should be valid"),
        ),
        DistributionRule::replicate(
            ColumnId::new("cu").expect("column should be valid"),
            ColumnId::new("cu").expect("column should be valid"),
        ),
        DistributionRule::replicate(
            ColumnId::new("domain").expect("column should be valid"),
            ColumnId::new("domain").expect("column should be valid"),
        ),
    ])
    .expect("rules should be valid");
    let target_grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 2).expect("shape should be valid"),
        None,
    )
    .expect("target grid should be valid");

    let subblocked =
        subblock(&sample_subblock_model(), target_grid, &rules).expect("subblock should work");

    assert_eq!(subblocked.block_count(), 4);
    assert!(!subblocked.is_sparse());

    let tonnes = match subblocked
        .column(&ColumnId::new("tonnes").expect("column should be valid"))
        .expect("tonnes column should exist")
    {
        ColumnData::Floats(values) => values.clone(),
        other => panic!("unexpected tonnes column type: {other:?}"),
    };
    let cu = match subblocked
        .column(&ColumnId::new("cu").expect("column should be valid"))
        .expect("cu column should exist")
    {
        ColumnData::Floats(values) => values.clone(),
        other => panic!("unexpected cu column type: {other:?}"),
    };
    let domain = match subblocked
        .column(&ColumnId::new("domain").expect("column should be valid"))
        .expect("domain column should exist")
    {
        ColumnData::Texts(values) => values.clone(),
        other => panic!("unexpected domain column type: {other:?}"),
    };

    assert_eq!(tonnes, vec![25.0, 25.0, 25.0, 25.0]);
    assert_eq!(tonnes.iter().sum::<f64>(), 100.0);
    assert_eq!(cu, vec![2.5, 2.5, 2.5, 2.5]);
    assert_eq!(domain, vec!["ore", "ore", "ore", "ore"]);
}

#[test]
fn subblock_emits_sparse_target_when_source_is_sparse() {
    let rules = DistributionRules::new(vec![DistributionRule::split_equally(
        ColumnId::new("tonnes").expect("column should be valid"),
        ColumnId::new("tonnes").expect("column should be valid"),
    )])
    .expect("rules should be valid");
    let target_grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(5.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(8, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("target grid should be valid");

    let subblocked = subblock(&sample_sparse_superblock_model(), target_grid, &rules)
        .expect("subblock should work");

    assert!(subblocked.is_sparse());
    assert_eq!(subblocked.block_count(), 4);
    assert_eq!(subblocked.linear_index_at(0).expect("row should exist"), 0);
    assert_eq!(subblocked.linear_index_at(3).expect("row should exist"), 3);

    let tonnes = match subblocked
        .column(&ColumnId::new("tonnes").expect("column should be valid"))
        .expect("tonnes column should exist")
    {
        ColumnData::Floats(values) => values.clone(),
        other => panic!("unexpected tonnes column type: {other:?}"),
    };

    assert_eq!(tonnes, vec![5.0, 5.0, 10.0, 10.0]);
    assert_eq!(tonnes.iter().sum::<f64>(), 30.0);
}

#[test]
fn subblock_rejects_incompatible_target_grid() {
    let rules = DistributionRules::new(vec![DistributionRule::split_equally(
        ColumnId::new("tonnes").expect("column should be valid"),
        ColumnId::new("tonnes").expect("column should be valid"),
    )])
    .expect("rules should be valid");
    let incompatible_target_grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(12.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 2).expect("shape should be valid"),
        None,
    )
    .expect("target grid should be valid");

    let error = subblock(&sample_subblock_model(), incompatible_target_grid, &rules)
        .expect_err("misaligned grid should fail");

    assert_eq!(
        error,
        MineError::grid(
            "subblock target dimension on axis `x` must divide the source dimension exactly"
        )
    );
}

#[test]
fn reconcile_models_reports_preserved_mass_and_metal() {
    let rules = AggregationRules::new(vec![
        AggregationRule::sum(
            ColumnId::new("tonnes").expect("column should be valid"),
            ColumnId::new("tonnes").expect("column should be valid"),
        ),
        AggregationRule::weighted_average(
            ColumnId::new("cu").expect("column should be valid"),
            ColumnId::new("cu").expect("column should be valid"),
            ColumnId::new("tonnes").expect("column should be valid"),
        ),
    ])
    .expect("rules should be valid");
    let target_grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(20.0, 10.0, 20.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("target grid should be valid");
    let rebloqued = superblock(&sample_superblock_model(), target_grid, &rules)
        .expect("superblock should succeed");
    let tolerances = ReconciliationTolerances::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0).expect("valid");

    let report = reconcile_models(
        &sample_superblock_model(),
        &rebloqued,
        &ColumnId::new("tonnes").expect("column should be valid"),
        &ColumnId::new("cu").expect("column should be valid"),
        &tolerances,
    )
    .expect("reconciliation should succeed");

    assert_eq!(report.tonnage.absolute_difference, Some(0.0));
    assert_eq!(report.contained_metal.absolute_difference, Some(0.0));
    assert_eq!(report.average_grade.absolute_difference, Some(0.0));
    assert!(!report.tonnage.tolerance_exceeded);
    assert!(!report.contained_metal.tolerance_exceeded);
    assert!(!report.average_grade.tolerance_exceeded);
    assert_eq!(report.block_count.absolute_difference, 3);
    assert!(report.block_count.tolerance_exceeded);
}

#[test]
fn reconcile_models_flags_tolerance_exceedance() {
    let altered_after = BlockModel::new(
        GridDefinition::new(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
            BlockDimensions::new(20.0, 10.0, 20.0).expect("dimensions should be valid"),
            GridShape::new(1, 1, 1).expect("shape should be valid"),
            None,
        )
        .expect("grid should be valid"),
        sample_schema(),
        Metadata::new(),
        BTreeMap::from([
            (
                ColumnId::new("tonnes").expect("column should be valid"),
                ColumnData::Floats(vec![90.0]),
            ),
            (
                ColumnId::new("cu").expect("column should be valid"),
                ColumnData::Floats(vec![3.0]),
            ),
            (
                ColumnId::new("domain").expect("column should be valid"),
                ColumnData::Texts(vec!["ore".to_owned()]),
            ),
            (
                ColumnId::new("selected").expect("column should be valid"),
                ColumnData::Booleans(vec![true]),
            ),
        ]),
    )
    .expect("model should be valid");
    let tolerances =
        ReconciliationTolerances::new(1.0, 0.01, 5.0, 0.05, 0.1, 0.01, 0).expect("valid");

    let report = reconcile_models(
        &sample_subblock_model(),
        &altered_after,
        &ColumnId::new("tonnes").expect("column should be valid"),
        &ColumnId::new("cu").expect("column should be valid"),
        &tolerances,
    )
    .expect("reconciliation should succeed");

    assert!(report.tonnage.tolerance_exceeded);
    assert!(report.contained_metal.tolerance_exceeded);
    assert!(report.average_grade.tolerance_exceeded);
    assert!(!report.block_count.tolerance_exceeded);
}

#[test]
fn build_adaptive_reblock_prototype_from_domain_rules() {
    let rules = vec![
        AdaptiveZoneRule::new("ore", AdaptiveResolutionStrategy::Preserve)
            .expect("rule should be valid"),
        AdaptiveZoneRule::new(
            "waste",
            AdaptiveResolutionStrategy::Superblock { factors: (2, 1, 1) },
        )
        .expect("rule should be valid"),
    ];

    let prototype = build_adaptive_reblock_prototype(
        &sample_superblock_model(),
        &ColumnId::new("domain").expect("column should be valid"),
        Some(&ColumnId::new("tonnes").expect("column should be valid")),
        &rules,
    )
    .expect("prototype should build");

    assert_eq!(prototype.zones.len(), 2);
    assert_eq!(prototype.zones[0].zone_value, "ore");
    assert_eq!(prototype.zones[0].block_count, 3);
    assert_eq!(prototype.zones[0].total_tonnage, Some(70.0));
    assert_eq!(prototype.zones[1].zone_value, "waste");
    assert_eq!(prototype.zones[1].block_count, 1);
    assert_eq!(prototype.zones[1].total_tonnage, Some(30.0));
    assert_eq!(prototype.limitations.len(), 3);
    assert_eq!(prototype.next_steps.len(), 3);
}

#[test]
fn reject_adaptive_reblock_with_float_zone_column() {
    let rules = vec![
        AdaptiveZoneRule::new("1.0", AdaptiveResolutionStrategy::Preserve)
            .expect("rule should be valid"),
    ];

    let error = build_adaptive_reblock_prototype(
        &sample_superblock_model(),
        &ColumnId::new("cu").expect("column should be valid"),
        None,
        &rules,
    )
    .expect_err("float zone column should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "columns",
            "adaptive reblock requires categorical zone column `cu`",
        )
    );
}

fn sample_schema() -> ColumnSchemaSet {
    ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
        ColumnSchema::new(
            ColumnId::new("cu").expect("column should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("domain").expect("column should be valid"),
            ColumnLogicalType::Text,
            None,
            false,
            ColumnMiningRole::Domain,
        ),
        ColumnSchema::new(
            ColumnId::new("selected").expect("column should be valid"),
            ColumnLogicalType::Boolean,
            None,
            false,
            ColumnMiningRole::Other,
        ),
    ])
    .expect("schema should be valid")
}

fn sample_superblock_model() -> BlockModel {
    let schema = sample_schema();
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 2).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");

    BlockModel::new(
        grid,
        schema,
        Metadata::new(),
        BTreeMap::from([
            (
                ColumnId::new("tonnes").expect("column should be valid"),
                ColumnData::Floats(vec![10.0, 20.0, 30.0, 40.0]),
            ),
            (
                ColumnId::new("cu").expect("column should be valid"),
                ColumnData::Floats(vec![1.0, 2.0, 3.0, 4.0]),
            ),
            (
                ColumnId::new("domain").expect("column should be valid"),
                ColumnData::Texts(vec![
                    "ore".to_owned(),
                    "ore".to_owned(),
                    "waste".to_owned(),
                    "ore".to_owned(),
                ]),
            ),
            (
                ColumnId::new("selected").expect("column should be valid"),
                ColumnData::Booleans(vec![true, true, false, true]),
            ),
        ]),
    )
    .expect("model should be valid")
}

fn sample_sparse_superblock_model() -> BlockModel {
    let schema = sample_schema();
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(4, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");

    BlockModel::new_sparse(
        grid,
        schema,
        Metadata::new(),
        vec![0, 1],
        BTreeMap::from([
            (
                ColumnId::new("tonnes").expect("column should be valid"),
                ColumnData::Floats(vec![10.0, 20.0]),
            ),
            (
                ColumnId::new("cu").expect("column should be valid"),
                ColumnData::Floats(vec![1.0, 2.0]),
            ),
            (
                ColumnId::new("domain").expect("column should be valid"),
                ColumnData::Texts(vec!["ore".to_owned(), "ore".to_owned()]),
            ),
            (
                ColumnId::new("selected").expect("column should be valid"),
                ColumnData::Booleans(vec![true, true]),
            ),
        ]),
    )
    .expect("sparse model should be valid")
}

fn sample_subblock_model() -> BlockModel {
    let schema = sample_schema();
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(20.0, 10.0, 20.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, 1).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");

    BlockModel::new(
        grid,
        schema,
        Metadata::new(),
        BTreeMap::from([
            (
                ColumnId::new("tonnes").expect("column should be valid"),
                ColumnData::Floats(vec![100.0]),
            ),
            (
                ColumnId::new("cu").expect("column should be valid"),
                ColumnData::Floats(vec![2.5]),
            ),
            (
                ColumnId::new("domain").expect("column should be valid"),
                ColumnData::Texts(vec!["ore".to_owned()]),
            ),
            (
                ColumnId::new("selected").expect("column should be valid"),
                ColumnData::Booleans(vec![true]),
            ),
        ]),
    )
    .expect("model should be valid")
}
