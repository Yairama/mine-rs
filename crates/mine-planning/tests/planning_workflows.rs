//! Tests de integración para workflows públicos de `mine-planning`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, MineError,
};
use mine_planning::{
    BenchParameters, BlockPrecedenceTemplate, NumericMetricTolerance, PrecedenceNode,
    PrecedenceOffset, PushbackGenerationRules, ScheduleConstraints, ScheduleEntry,
    ScheduleViolationCode, assign_benches, assign_phases_from_column, build_block_precedence_graph,
    build_pushback_prototype, build_schedule, build_upit_prototype, compare_block_memberships,
    compare_named_numeric_metrics, compare_precedence_graphs, compare_upit_reports,
    read_precedence_graph_json, write_precedence_graph_json,
};

#[test]
fn build_schedule_aggregates_period_tonnage() {
    let schedule = build_schedule(
        vec![
            ScheduleEntry::new("P1", 100, 500.0, 5, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P1", 101, 450.0, 4, Some("phase-b".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P2", 102, 400.0, 3, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
        ],
        ScheduleConstraints::default(),
    )
    .expect("schedule should build");

    assert_eq!(schedule.period_summaries().len(), 2);
    assert_eq!(schedule.period_summaries()[0].period_label, "P1");
    assert_eq!(schedule.period_summaries()[0].total_tonnage, 950.0);
    assert_eq!(schedule.period_summaries()[0].total_blocks, 9);
    assert_eq!(schedule.violations().len(), 0);
}

#[test]
fn build_schedule_reports_tonnage_constraint_violations() {
    let schedule = build_schedule(
        vec![
            ScheduleEntry::new("P1", 100, 700.0, 5, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P1", 101, 450.0, 4, Some("phase-b".to_owned()))
                .expect("entry should be valid"),
        ],
        ScheduleConstraints::new(Some(1000.0), None).expect("constraints should be valid"),
    )
    .expect("schedule should build");

    assert_eq!(schedule.violations().len(), 1);
    assert_eq!(
        schedule.violations()[0].code,
        ScheduleViolationCode::ExceedsPeriodTonnage
    );
    assert_eq!(schedule.violations()[0].period_label, "P1");
    assert!(
        schedule.violations()[0]
            .message
            .contains("configured limit")
    );
}

#[test]
fn build_pushback_prototype_groups_schedule_by_phase() {
    let schedule = build_schedule(
        vec![
            ScheduleEntry::new("P1", 100, 500.0, 5, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P2", 101, 450.0, 4, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P1", 90, 300.0, 3, Some("phase-b".to_owned()))
                .expect("entry should be valid"),
        ],
        ScheduleConstraints::default(),
    )
    .expect("schedule should be valid");
    let rules = PushbackGenerationRules::new(true, Some(3)).expect("rules should be valid");

    let report = build_pushback_prototype(&schedule, &rules).expect("report should build");

    assert_eq!(report.pushbacks.len(), 2);
    assert_eq!(report.pushbacks[0].phase.as_deref(), Some("phase-a"));
    assert_eq!(
        report.pushbacks[0].periods,
        vec!["P1".to_owned(), "P2".to_owned()]
    );
    assert_eq!(report.pushbacks[0].benches, vec![100, 101]);
    assert_eq!(report.pushbacks[0].total_tonnage, 950.0);
    assert_eq!(report.pushbacks[0].total_blocks, 9);
    assert_eq!(report.limitations.len(), 3);
    assert_eq!(report.next_steps.len(), 3);
}

#[test]
fn reject_pushback_prototype_without_required_phase() {
    let schedule = build_schedule(
        vec![ScheduleEntry::new("P1", 100, 500.0, 5, None).expect("entry should be valid")],
        ScheduleConstraints::default(),
    )
    .expect("schedule should be valid");
    let rules = PushbackGenerationRules::new(true, None).expect("rules should be valid");

    let error = build_pushback_prototype(&schedule, &rules).expect_err("missing phase should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "schedule",
            "pushback prototype requires every schedule entry to declare a phase",
        )
    );
}

#[test]
fn reject_pushback_prototype_when_group_count_exceeds_limit() {
    let schedule = build_schedule(
        vec![
            ScheduleEntry::new("P1", 100, 500.0, 5, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P1", 90, 300.0, 3, Some("phase-b".to_owned()))
                .expect("entry should be valid"),
        ],
        ScheduleConstraints::default(),
    )
    .expect("schedule should be valid");
    let rules = PushbackGenerationRules::new(true, Some(1)).expect("rules should be valid");

    let error = build_pushback_prototype(&schedule, &rules).expect_err("limit should be enforced");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "schedule",
            "pushback prototype derived 2 pushbacks, exceeding configured limit of 1",
        )
    );
}

fn vertical_model(nz: usize) -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, nz).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");
    let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
        ColumnId::new("tonnes").expect("column id should be valid"),
        ColumnLogicalType::Float,
        Some(MeasurementUnit::new("t").expect("unit should be valid")),
        false,
        ColumnMiningRole::Tonnage,
    )])
    .expect("schema should be valid");

    BlockModel::new(
        grid,
        schema,
        Metadata::new(),
        BTreeMap::from([(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnData::Floats(vec![1.0; nz]),
        )]),
    )
    .expect("block model should be valid")
}

fn sparse_vertical_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, 3).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");
    let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
        ColumnId::new("tonnes").expect("column id should be valid"),
        ColumnLogicalType::Float,
        Some(MeasurementUnit::new("t").expect("unit should be valid")),
        false,
        ColumnMiningRole::Tonnage,
    )])
    .expect("schema should be valid");

    BlockModel::new_sparse(
        grid,
        schema,
        Metadata::new(),
        vec![0, 2],
        BTreeMap::from([(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnData::Floats(vec![1.0, 1.0]),
        )]),
    )
    .expect("block model should be valid")
}

fn sparse_phase_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, 3).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");
    let schema = ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
        ColumnId::new("phase").expect("column id should be valid"),
        ColumnLogicalType::Text,
        None,
        false,
        ColumnMiningRole::Phase,
    )])
    .expect("schema should be valid");

    BlockModel::new_sparse(
        grid,
        schema,
        Metadata::new(),
        vec![0, 2],
        BTreeMap::from([(
            ColumnId::new("phase").expect("column id should be valid"),
            ColumnData::Texts(vec!["P1".to_owned(), "P3".to_owned()]),
        )]),
    )
    .expect("block model should be valid")
}

#[test]
fn build_block_precedence_graph_from_vertical_offsets() {
    let model = vertical_model(3);
    let template = BlockPrecedenceTemplate::new(vec![
        PrecedenceOffset::new(0, 0, 1).expect("offset should be valid"),
    ])
    .expect("template should be valid");

    let graph = build_block_precedence_graph(&model, &template).expect("graph should build");

    assert_eq!(graph.nodes().len(), 3);
    assert_eq!(graph.edges().len(), 2);
    assert_eq!(graph.edges()[0].predecessor(), &PrecedenceNode::Block(1));
    assert_eq!(graph.edges()[0].successor(), &PrecedenceNode::Block(0));
    assert_eq!(graph.edges()[1].predecessor(), &PrecedenceNode::Block(2));
    assert_eq!(graph.edges()[1].successor(), &PrecedenceNode::Block(1));
}

#[test]
fn assign_benches_uses_sparse_linear_indices() {
    let model = sparse_vertical_model();
    let assignments = assign_benches(
        &model,
        &BenchParameters::new(10.0, 0.0, 1e-9).expect("parameters should be valid"),
    )
    .expect("bench assignment should work");

    assert_eq!(assignments.len(), 2);
    assert_eq!(assignments[0].linear_index, 0);
    assert_eq!(assignments[0].bench, 0);
    assert_eq!(assignments[1].linear_index, 2);
    assert_eq!(assignments[1].bench, 2);
}

#[test]
fn assign_phases_preserves_sparse_linear_indices() {
    let model = sparse_phase_model();
    let report = assign_phases_from_column(
        &model,
        &ColumnId::new("phase").expect("column id should be valid"),
    )
    .expect("phase assignment should work");

    assert_eq!(report.assignments.len(), 2);
    assert_eq!(report.assignments[0].linear_index, 0);
    assert_eq!(report.assignments[0].phase, "P1");
    assert_eq!(report.assignments[1].linear_index, 2);
    assert_eq!(report.assignments[1].phase, "P3");
}

#[test]
fn build_upit_prototype_closes_positive_blocks_by_precedence() {
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(1, 1, 3).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");
    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("value").expect("column id should be valid"),
            ColumnLogicalType::Float,
            None,
            false,
            ColumnMiningRole::Other,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
    ])
    .expect("schema should be valid");
    let model = BlockModel::new(
        grid,
        schema,
        Metadata::new(),
        BTreeMap::from([
            (
                ColumnId::new("value").expect("column id should be valid"),
                ColumnData::Floats(vec![10.0, -3.0, 2.0]),
            ),
            (
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnData::Floats(vec![1.0, 1.0, 1.0]),
            ),
        ]),
    )
    .expect("block model should be valid");
    let template = BlockPrecedenceTemplate::new(vec![
        PrecedenceOffset::new(0, 0, 1).expect("offset should be valid"),
    ])
    .expect("template should be valid");
    let graph = build_block_precedence_graph(&model, &template).expect("graph should build");

    let report = build_upit_prototype(
        &model,
        &graph,
        &ColumnId::new("value").expect("column id should be valid"),
        Some(&ColumnId::new("tonnes").expect("column id should be valid")),
    )
    .expect("upit prototype should build");

    assert_eq!(report.selected_linear_indices, vec![0, 1, 2]);
    assert_eq!(report.block_count, 3);
    assert_eq!(report.total_value, 9.0);
    assert_eq!(report.total_tonnage, Some(3.0));
    assert_eq!(report.heuristic, "positive-block-closure");
    assert_eq!(report.limitations.len(), 3);
}

#[test]
fn precedence_graph_json_roundtrip_preserves_generated_graph() {
    let model = vertical_model(3);
    let template = BlockPrecedenceTemplate::new(vec![
        PrecedenceOffset::new(0, 0, 1).expect("offset should be valid"),
    ])
    .expect("template should be valid");
    let graph = build_block_precedence_graph(&model, &template).expect("graph should build");
    let path = temporary_json_path("precedence-graph");

    write_precedence_graph_json(&graph, &path).expect("graph should write");
    let decoded = read_precedence_graph_json(&path).expect("graph should read");

    assert_eq!(decoded, graph);

    let _ = fs::remove_file(path);
}

#[test]
fn compare_precedence_graphs_reports_missing_edges_and_nodes() {
    let reference = mine_planning::PrecedenceGraph::from_nodes_and_edges(
        vec![
            PrecedenceNode::Block(0),
            PrecedenceNode::Block(1),
            PrecedenceNode::Block(2),
        ],
        vec![
            mine_planning::PrecedenceEdge::new(PrecedenceNode::Block(2), PrecedenceNode::Block(1)),
            mine_planning::PrecedenceEdge::new(PrecedenceNode::Block(1), PrecedenceNode::Block(0)),
        ],
    )
    .expect("reference graph should build");
    let candidate = mine_planning::PrecedenceGraph::from_nodes_and_edges(
        vec![PrecedenceNode::Block(0), PrecedenceNode::Block(1)],
        vec![mine_planning::PrecedenceEdge::new(
            PrecedenceNode::Block(1),
            PrecedenceNode::Block(0),
        )],
    )
    .expect("candidate graph should build");

    let comparison = compare_precedence_graphs(&reference, &candidate);

    assert_eq!(comparison.reference_node_count, 3);
    assert_eq!(comparison.candidate_node_count, 2);
    assert_eq!(comparison.shared_nodes, 2);
    assert_eq!(
        comparison.reference_only_nodes,
        vec![PrecedenceNode::Block(2)]
    );
    assert!(comparison.candidate_only_nodes.is_empty());
    assert_eq!(comparison.shared_edges, 1);
    assert_eq!(comparison.reference_only_edges.len(), 1);
    assert!(comparison.candidate_only_edges.is_empty());
    assert!(comparison.edge_jaccard_index < 1.0);
}

#[test]
fn compare_block_memberships_reports_jaccard_and_differences() {
    let comparison = compare_block_memberships(&[1, 2, 3], &[2, 3, 4]);

    assert_eq!(comparison.reference_block_count, 3);
    assert_eq!(comparison.candidate_block_count, 3);
    assert_eq!(comparison.shared_blocks, 2);
    assert_eq!(comparison.reference_only_blocks, vec![1]);
    assert_eq!(comparison.candidate_only_blocks, vec![4]);
    assert!((comparison.jaccard_index - 0.5).abs() < 1e-9);
}

#[test]
fn compare_upit_reports_uses_selected_block_membership() {
    let reference = mine_planning::UpitPrototypeReport {
        value_column: ColumnId::new("value").expect("column id should be valid"),
        tonnage_column: None,
        selected_linear_indices: vec![0, 1, 2],
        block_count: 3,
        total_value: 12.0,
        total_tonnage: None,
        heuristic: "reference".to_owned(),
        limitations: Vec::new(),
    };
    let candidate = mine_planning::UpitPrototypeReport {
        value_column: ColumnId::new("value").expect("column id should be valid"),
        tonnage_column: None,
        selected_linear_indices: vec![1, 2, 4],
        block_count: 3,
        total_value: 8.0,
        total_tonnage: None,
        heuristic: "candidate".to_owned(),
        limitations: Vec::new(),
    };

    let comparison = compare_upit_reports(&reference, &candidate);

    assert_eq!(comparison.shared_blocks, 2);
    assert_eq!(comparison.reference_only_blocks, vec![0]);
    assert_eq!(comparison.candidate_only_blocks, vec![4]);
}

#[test]
fn compare_named_numeric_metrics_reports_tolerances_and_missing_metrics() {
    let reference = BTreeMap::from([("metal".to_owned(), 20.0), ("tonnage".to_owned(), 100.0)]);
    let candidate = BTreeMap::from([
        ("metal".to_owned(), 18.5),
        ("tonnage".to_owned(), 99.5),
        ("value".to_owned(), 150.0),
    ]);
    let tolerances = BTreeMap::from([
        (
            "metal".to_owned(),
            NumericMetricTolerance {
                absolute: Some(1.0),
                relative: Some(0.1),
            },
        ),
        (
            "tonnage".to_owned(),
            NumericMetricTolerance {
                absolute: Some(1.0),
                relative: None,
            },
        ),
    ]);

    let comparison = compare_named_numeric_metrics(&reference, &candidate, &tolerances);

    assert_eq!(comparison.shared_metrics.len(), 2);
    assert!(comparison.reference_only_metrics.is_empty());
    assert_eq!(comparison.candidate_only_metrics, vec!["value".to_owned()]);

    let metal = comparison
        .shared_metrics
        .iter()
        .find(|metric| metric.metric == "metal")
        .expect("metal metric should exist");
    assert!(!metal.within_tolerance);
    assert_eq!(metal.absolute_tolerance, Some(1.0));
    assert_eq!(metal.relative_tolerance, Some(0.1));

    let tonnage = comparison
        .shared_metrics
        .iter()
        .find(|metric| metric.metric == "tonnage")
        .expect("tonnage metric should exist");
    assert!(tonnage.within_tolerance);
    assert_eq!(tonnage.absolute_difference, 0.5);
}

fn temporary_json_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();

    std::env::temp_dir().join(format!("mine-rs-{prefix}-{unique}.json"))
}
