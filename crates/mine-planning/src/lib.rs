//! Primitives deterministas de planeamiento para `mine-rs`.

mod benches;
mod comparison;
mod marvin;
mod max_closure;
mod phase_design;
mod phases;
mod pit_shells;
mod precedence;
mod pushback;
mod scenario;
mod schedule;
mod slope_templates;
mod upit;
mod upl_solver;

pub use benches::{BenchAssignment, BenchParameters, assign_benches};
pub use comparison::{
    BlockMembershipComparisonReport, NumericMetricComparison, NumericMetricComparisonReport,
    NumericMetricTolerance, PrecedenceGraphComparisonReport, compare_block_memberships,
    compare_named_numeric_metrics, compare_precedence_graphs, compare_upit_reports,
};
pub use marvin::{
    read_marvin_precedence_graph, read_marvin_upit_block_values, read_marvin_upit_solution,
};
pub use phases::{PhaseAssignment, PhaseTaggingReport, assign_phases_from_column};
pub use precedence::{
    BlockPrecedenceTemplate, PrecedenceEdge, PrecedenceGraph, PrecedenceNode, PrecedenceOffset,
    build_block_precedence_graph, read_precedence_graph_json, write_precedence_graph_json,
};
pub use max_closure::{
    MaxClosureArc, MaxClosureArcKind, MaxClosureGraph, MaxClosureNodeId, build_max_closure_graph,
    verify_closure,
};
pub use phase_design::{
    NestingAccessRules, PhaseDesign, PushbackPlan, derive_pushbacks_from_nested_shells,
};
pub use pit_shells::{
    PitShell, PitShellMetrics, PitShellSet, compute_pit_shell_metrics,
    generate_nested_shells, generate_nested_shells_from_model, uniform_revenue_factors,
};
pub use pushback::{
    PushbackGenerationRules, PushbackPrototype, PushbackPrototypeReport, build_pushback_prototype,
};
pub use scenario::{MiningScenario, ScenarioConstraints, ScenarioPeriod, ScenarioRules};
pub use schedule::{
    Schedule, ScheduleConstraints, ScheduleEntry, SchedulePeriodSummary, ScheduleViolation,
    ScheduleViolationCode, build_schedule, validate_vertical_advance,
};
pub use slope_templates::{
    SlopeAngleRule, VariableSlopeTemplate, derive_precedence_template_from_slope,
};
pub use upit::{UpitPrototypeReport, build_upit_prototype};
pub use upl_solver::{UplSolverResult, solve_upl_exact};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_blockmodel::{BlockModel, ColumnData};
    use mine_core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
        MineError, ModelId, ScenarioId,
    };

    use super::*;

    fn sample_schema() -> ColumnSchemaSet {
        ColumnSchemaSet::from_columns(vec![ColumnSchema::new(
            ColumnId::new("tonnes").expect("column id should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        )])
        .expect("schema should be valid")
    }

    fn vertical_model(origin_z: f64, dz: f64, nz: usize) -> BlockModel {
        let grid = GridDefinition::new(
            Coordinate3D::new(0.0, 0.0, origin_z).expect("origin should be valid"),
            BlockDimensions::new(10.0, 10.0, dz).expect("dimensions should be valid"),
            GridShape::new(1, 1, nz).expect("shape should be valid"),
            None,
        )
        .expect("grid should be valid");

        let tonnes = vec![1.0; nz];

        BlockModel::new(
            grid,
            sample_schema(),
            Metadata::new(),
            BTreeMap::from([(
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnData::Floats(tonnes),
            )]),
        )
        .expect("block model should be valid")
    }

    fn phase_model(phases: ColumnData) -> BlockModel {
        let grid = GridDefinition::new(
            Coordinate3D::new(0.0, 0.0, 100.0).expect("origin should be valid"),
            BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
            GridShape::new(1, 1, 2).expect("shape should be valid"),
            None,
        )
        .expect("grid should be valid");

        let schema = ColumnSchemaSet::from_columns(vec![
            ColumnSchema::new(
                ColumnId::new("tonnes").expect("column id should be valid"),
                ColumnLogicalType::Float,
                Some(MeasurementUnit::new("t").expect("unit should be valid")),
                false,
                ColumnMiningRole::Tonnage,
            ),
            ColumnSchema::new(
                ColumnId::new("phase").expect("column id should be valid"),
                phases.logical_type(),
                None,
                false,
                ColumnMiningRole::Phase,
            ),
        ])
        .expect("schema should be valid");

        BlockModel::new(
            grid,
            schema,
            Metadata::new(),
            BTreeMap::from([
                (
                    ColumnId::new("tonnes").expect("column id should be valid"),
                    ColumnData::Floats(vec![1.0, 1.0]),
                ),
                (
                    ColumnId::new("phase").expect("column id should be valid"),
                    phases,
                ),
            ]),
        )
        .expect("block model should be valid")
    }

    #[test]
    fn assign_benches_over_multiple_levels() {
        let model = vertical_model(100.0, 10.0, 4);
        let parameters =
            BenchParameters::new(20.0, 100.0, 1e-9).expect("parameters should be valid");

        let assignments = assign_benches(&model, &parameters).expect("assignments should work");

        assert_eq!(assignments.len(), 4);
        assert_eq!(
            assignments
                .iter()
                .map(|assignment| assignment.bench)
                .collect::<Vec<_>>(),
            vec![0, 0, 1, 1]
        );
        assert_eq!(assignments[0].center_elevation, 105.0);
        assert_eq!(assignments[3].center_elevation, 135.0);
    }

    #[test]
    fn assign_benches_with_shifted_origin() {
        let model = vertical_model(100.0, 10.0, 4);
        let parameters =
            BenchParameters::new(20.0, 90.0, 1e-9).expect("parameters should be valid");

        let assignments = assign_benches(&model, &parameters).expect("assignments should work");

        assert_eq!(
            assignments
                .iter()
                .map(|assignment| assignment.bench)
                .collect::<Vec<_>>(),
            vec![0, 1, 1, 2]
        );
    }

    #[test]
    fn apply_tolerance_near_upper_bench_boundary() {
        let model = vertical_model(-1e-10, 20.0, 1);
        let parameters = BenchParameters::new(10.0, 0.0, 1e-9).expect("parameters should be valid");

        let assignments = assign_benches(&model, &parameters).expect("assignments should work");

        assert_eq!(assignments[0].bench, 1);
    }

    #[test]
    fn reject_invalid_bench_parameters() {
        let error =
            BenchParameters::new(0.0, 100.0, 1e-9).expect_err("zero bench height should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "bench_height",
                "bench height must be a finite positive value"
            )
        );
    }

    #[test]
    fn assign_phases_from_text_column() {
        let model = phase_model(ColumnData::Texts(vec!["P1".to_owned(), "P2".to_owned()]));

        let report = assign_phases_from_column(
            &model,
            &ColumnId::new("phase").expect("column id should be valid"),
        )
        .expect("phase tagging should work");

        assert_eq!(report.assignments.len(), 2);
        assert!(report.unassigned_indices.is_empty());
        assert_eq!(report.assignments[0].phase, "P1");
        assert_eq!(report.assignments[1].phase, "P2");
    }

    #[test]
    fn report_unassigned_phases_for_blank_text_values() {
        let model = phase_model(ColumnData::Texts(vec!["P1".to_owned(), "".to_owned()]));

        let report = assign_phases_from_column(
            &model,
            &ColumnId::new("phase").expect("column id should be valid"),
        )
        .expect("phase tagging should work");

        assert_eq!(report.assignments.len(), 1);
        assert_eq!(report.unassigned_indices, vec![1]);
    }

    #[test]
    fn reject_phase_tagging_from_float_column() {
        let model = phase_model(ColumnData::Floats(vec![1.0, 2.0]));

        let error = assign_phases_from_column(
            &model,
            &ColumnId::new("phase").expect("column id should be valid"),
        )
        .expect_err("float phase source should fail");

        assert_eq!(
            error,
            MineError::schema(
                "phase source column `phase` must be categorical (text, integer or boolean)"
            )
        );
    }

    #[test]
    fn build_serializable_mining_scenario() {
        let scenario = MiningScenario::new(
            ScenarioId::new("scenario-01").expect("scenario id should be valid"),
            ModelId::new("model-01").expect("model id should be valid"),
            vec![
                ScenarioPeriod::new("P1", Some(1000.0), None).expect("period should be valid"),
                ScenarioPeriod::new("P2", Some(1200.0), Some(10)).expect("period should be valid"),
            ],
            ScenarioRules::new(
                Some(ColumnId::new("phase").expect("column id should be valid")),
                Some(
                    BenchParameters::new(20.0, 100.0, 1e-9)
                        .expect("bench parameters should be valid"),
                ),
            ),
            ScenarioConstraints::new(Some(30.0), Some(2)).expect("constraints should be valid"),
            Metadata::new(),
        )
        .expect("scenario should be valid");

        let json = serde_json::to_string(&scenario).expect("scenario should serialize");

        assert!(json.contains("scenario-01"));
        assert_eq!(scenario.periods().len(), 2);
        assert_eq!(scenario.constraints().max_active_phases(), Some(2));
    }

    #[test]
    fn reject_empty_scenario_periods() {
        let error = MiningScenario::new(
            ScenarioId::new("scenario-01").expect("scenario id should be valid"),
            ModelId::new("model-01").expect("model id should be valid"),
            Vec::new(),
            ScenarioRules::default(),
            ScenarioConstraints::default(),
            Metadata::new(),
        )
        .expect_err("scenario without periods should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "periods",
                "mining scenario must contain at least one period"
            )
        );
    }

    #[test]
    fn reject_duplicate_scenario_period_labels() {
        let error = MiningScenario::new(
            ScenarioId::new("scenario-01").expect("scenario id should be valid"),
            ModelId::new("model-01").expect("model id should be valid"),
            vec![
                ScenarioPeriod::new("P1", Some(1000.0), None).expect("period should be valid"),
                ScenarioPeriod::new("P1", Some(1200.0), None).expect("period should be valid"),
            ],
            ScenarioRules::default(),
            ScenarioConstraints::default(),
            Metadata::new(),
        )
        .expect_err("duplicate period labels should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter("periods", "scenario period label `P1` is duplicated")
        );
    }

    #[test]
    fn reject_invalid_scenario_constraints() {
        let error = ScenarioConstraints::new(Some(0.0), Some(1))
            .expect_err("zero max vertical advance should fail");

        assert_eq!(
            error,
            MineError::invalid_parameter(
                "max_vertical_advance",
                "scenario max vertical advance must be finite and greater than zero"
            )
        );
    }

    #[test]
    fn build_acyclic_precedence_graph() {
        let graph = PrecedenceGraph::new(vec![
            PrecedenceEdge::new(
                PrecedenceNode::Phase("P1".to_owned()),
                PrecedenceNode::Bench(100),
            ),
            PrecedenceEdge::new(PrecedenceNode::Bench(100), PrecedenceNode::Block(12)),
        ])
        .expect("acyclic graph should be valid");

        assert_eq!(graph.nodes().len(), 3);
        assert_eq!(
            graph.successors(&PrecedenceNode::Phase("P1".to_owned())),
            vec![PrecedenceNode::Bench(100)]
        );
        assert_eq!(
            graph.predecessors(&PrecedenceNode::Block(12)),
            vec![PrecedenceNode::Bench(100)]
        );
    }

    #[test]
    fn reject_cyclic_precedence_graph() {
        let error = PrecedenceGraph::new(vec![
            PrecedenceEdge::new(
                PrecedenceNode::Phase("P1".to_owned()),
                PrecedenceNode::Bench(100),
            ),
            PrecedenceEdge::new(
                PrecedenceNode::Bench(100),
                PrecedenceNode::Phase("P1".to_owned()),
            ),
        ])
        .expect_err("cycle should be rejected");

        assert_eq!(
            error,
            MineError::Planning {
                message: "precedence graph contains a cycle and is not a valid DAG".to_owned(),
            }
        );
    }

    #[test]
    fn reject_self_referencing_precedence_edge() {
        let error = PrecedenceGraph::new(vec![PrecedenceEdge::new(
            PrecedenceNode::Bench(100),
            PrecedenceNode::Bench(100),
        )])
        .expect_err("self-referencing edge should fail");

        assert_eq!(
            error,
            MineError::Planning {
                message:
                    "precedence edge cannot reference the same node as predecessor and successor"
                        .to_owned(),
            }
        );
    }

    #[test]
    fn report_vertical_advance_violations_with_location() {
        let entries = vec![
            ScheduleEntry::new("P1", 100, 500.0, 5, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P2", 103, 450.0, 4, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
        ];

        let violations =
            validate_vertical_advance(&entries, 2).expect("vertical validation should work");

        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].code,
            ScheduleViolationCode::ExceedsVerticalAdvance
        );
        assert_eq!(violations[0].period_label, "P2");
        assert_eq!(violations[0].phase.as_deref(), Some("phase-a"));
        assert_eq!(violations[0].bench, Some(103));
    }
}
