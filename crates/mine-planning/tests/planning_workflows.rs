//! Tests de integración para workflows públicos de `mine-planning`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, MineError, ModelId,
    ScenarioId,
};
use mine_planning::{
    BenchAssignment, BenchParameters, BlockPrecedenceTemplate, DecomposedSchedulingConfig,
    DecomposedTemporalSolver, LongTermSchedule, LongTermScheduleEntry,
    LongTermSchedulePeriodCapacity, LongTermScheduleStockpile, LongTermScheduleViolationCode,
    LongTermStockpileDepositPolicy, LongTermStockpilePolicy, LongTermStockpileReclaimPolicy,
    NestingAccessRules, NumericMetricTolerance, PitShell, PitShellSet, PrecedenceEdge,
    PrecedenceGraph, PrecedenceNode, PrecedenceOffset, PushbackGenerationRules, PushbackPlan,
    ScheduleConstraints, ScheduleDestinationCapacity, ScheduleDestinationId, ScheduleEntry,
    ScheduleStockpileCapacity, ScheduleStockpileId, ScheduleViolationCode, SchedulingObjectiveTerm,
    SchedulingPeriod, SchedulingProblem, SchedulingResourceBound, SchedulingResourceId,
    SchedulingResourceRequirement, SchedulingUnit, SchedulingUnitId, SmallSchedulingSolution,
    apply_long_term_stockpile_policy, assign_benches, assign_phases_from_column,
    build_aggregated_long_term_schedule, build_block_precedence_graph, build_pushback_prototype,
    build_ready_frontier_long_term_schedule, build_ready_frontier_schedule, build_schedule,
    build_target_period_seeded_schedule, build_target_period_windowed_schedule,
    build_upit_prototype, compare_block_memberships, compare_named_numeric_metrics,
    compare_precedence_graphs, compare_upit_reports, derive_phase_design_from_nested_shells,
    evaluate_long_term_schedule_material_flows, read_pit_shell_set_json,
    read_precedence_graph_json, solve_decomposed_scheduling_problem,
    solve_small_scheduling_problem, stockpile_reclaim_capacity_resource_id,
    write_pit_shell_set_json, write_precedence_graph_json,
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
fn build_long_term_schedule_from_legacy_schedule() {
    let schedule = build_schedule(
        vec![
            ScheduleEntry::new("P1", 100, 700.0, 5, Some("phase-a".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P1", 101, 450.0, 4, Some("phase-b".to_owned()))
                .expect("entry should be valid"),
            ScheduleEntry::new("P2", 99, 400.0, 3, Some("phase-c".to_owned()))
                .expect("entry should be valid"),
        ],
        ScheduleConstraints::new(Some(1000.0), None).expect("constraints should be valid"),
    )
    .expect("schedule should build");

    let long_term_schedule = LongTermSchedule::from_schedule(
        ScenarioId::new("scenario-01").expect("scenario should be valid"),
        ModelId::new("model-01").expect("model should be valid"),
        &schedule,
        Metadata::new(),
    )
    .expect("long-term schedule should build");

    assert_eq!(long_term_schedule.entries().len(), 3);
    assert_eq!(long_term_schedule.capacities().len(), 2);
    assert_eq!(long_term_schedule.entries()[0].phase_id(), Some("phase-a"));
    assert_eq!(long_term_schedule.entries()[0].bench(), Some(100));
    assert_eq!(
        long_term_schedule.violations()[0].code,
        LongTermScheduleViolationCode::ExceedsMineCapacity
    );
}

#[test]
fn reject_long_term_schedule_entry_with_ambiguous_routing() {
    let error = LongTermScheduleEntry::new(
        "P1",
        Some("phase-a".to_owned()),
        Some(0),
        Some(100),
        500.0,
        5,
        Some(ScheduleDestinationId::new("mill").expect("destination should be valid")),
        Some(ScheduleStockpileId::new("sp-main").expect("stockpile should be valid")),
        vec!["phase-root".to_owned()],
    )
    .expect_err("ambiguous routing should fail");

    assert_eq!(
        error,
        MineError::Validation {
            message:
                "long-term schedule entry cannot route simultaneously to a destination and a stockpile"
                    .to_owned(),
        }
    );
}

#[test]
fn build_aggregated_long_term_schedule_respects_predecessors_and_splits_tonnage() {
    let phase_plan = PushbackPlan {
        phases: vec![
            mine_planning::PhaseDesign {
                phase_id: "phase-a".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(0.6),
                bench: Some(101),
                block_indices: vec![0, 1],
                block_count: 10,
                total_tonnage: Some(1_200.0),
                predecessor_phase_ids: vec![],
            },
            mine_planning::PhaseDesign {
                phase_id: "phase-b".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(0.6),
                bench: Some(95),
                block_indices: vec![2, 3],
                block_count: 8,
                total_tonnage: Some(800.0),
                predecessor_phase_ids: vec!["phase-a".to_owned()],
            },
        ],
        phase_count: 2,
        total_block_count: 4,
        total_tonnage: Some(2_000.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: vec![],
    };
    let capacities = vec![
        LongTermSchedulePeriodCapacity::new("P1", Some(1_000.0), None, vec![], vec![])
            .expect("capacity should be valid"),
        LongTermSchedulePeriodCapacity::new("P2", Some(1_000.0), None, vec![], vec![])
            .expect("capacity should be valid"),
        LongTermSchedulePeriodCapacity::new("P3", Some(1_000.0), None, vec![], vec![])
            .expect("capacity should be valid"),
    ];

    let schedule = build_aggregated_long_term_schedule(
        ScenarioId::new("scenario-agg").expect("scenario should be valid"),
        ModelId::new("model-agg").expect("model should be valid"),
        &phase_plan,
        capacities,
        Some(10),
        Metadata::new(),
    )
    .expect("aggregated long-term schedule should build");

    assert_eq!(schedule.entries().len(), 3);
    assert_eq!(schedule.entries()[0].period_label(), "P1");
    assert_eq!(schedule.entries()[0].tonnage(), 1_000.0);
    assert_eq!(schedule.entries()[1].period_label(), "P2");
    assert_eq!(schedule.entries()[1].phase_id(), Some("phase-a"));
    assert_eq!(schedule.entries()[2].period_label(), "P3");
    assert_eq!(schedule.entries()[2].phase_id(), Some("phase-b"));
}

#[test]
fn build_aggregated_long_term_schedule_reports_vertical_advance_violation() {
    let phase_plan = PushbackPlan {
        phases: vec![
            mine_planning::PhaseDesign {
                phase_id: "phase-a".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(0.6),
                bench: Some(110),
                block_indices: vec![0],
                block_count: 5,
                total_tonnage: Some(500.0),
                predecessor_phase_ids: vec![],
            },
            mine_planning::PhaseDesign {
                phase_id: "phase-b".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(0.6),
                bench: Some(95),
                block_indices: vec![1],
                block_count: 5,
                total_tonnage: Some(500.0),
                predecessor_phase_ids: vec!["phase-a".to_owned()],
            },
        ],
        phase_count: 2,
        total_block_count: 2,
        total_tonnage: Some(1_000.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: vec![],
    };
    let capacities = vec![
        LongTermSchedulePeriodCapacity::new("P1", Some(1_000.0), None, vec![], vec![])
            .expect("capacity should be valid"),
        LongTermSchedulePeriodCapacity::new("P2", Some(1_000.0), None, vec![], vec![])
            .expect("capacity should be valid"),
    ];

    let schedule = build_aggregated_long_term_schedule(
        ScenarioId::new("scenario-vert").expect("scenario should be valid"),
        ModelId::new("model-vert").expect("model should be valid"),
        &phase_plan,
        capacities,
        Some(10),
        Metadata::new(),
    )
    .expect("aggregated long-term schedule should build");

    assert_eq!(schedule.violations().len(), 1);
    assert_eq!(
        schedule.violations()[0].code,
        LongTermScheduleViolationCode::ExceedsVerticalAdvance
    );
}

#[test]
fn build_aggregated_long_term_schedule_preserves_tonnage_when_block_counts_round_to_zero() {
    let phase_plan = PushbackPlan {
        phases: vec![mine_planning::PhaseDesign {
            phase_id: "phase-a".to_owned(),
            pushback_index: 0,
            shell_index: Some(0),
            revenue_factor: Some(1.0),
            bench: Some(100),
            block_indices: vec![0, 1],
            block_count: 2,
            total_tonnage: Some(30.0),
            predecessor_phase_ids: vec![],
        }],
        phase_count: 1,
        total_block_count: 2,
        total_tonnage: Some(30.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: vec![],
    };
    let capacities = vec![
        LongTermSchedulePeriodCapacity::new("P1", Some(10.0), None, vec![], vec![])
            .expect("capacity should be valid"),
        LongTermSchedulePeriodCapacity::new("P2", Some(10.0), None, vec![], vec![])
            .expect("capacity should be valid"),
        LongTermSchedulePeriodCapacity::new("P3", Some(10.0), None, vec![], vec![])
            .expect("capacity should be valid"),
    ];

    let schedule = build_aggregated_long_term_schedule(
        ScenarioId::new("scenario-rounding").expect("scenario should be valid"),
        ModelId::new("model-rounding").expect("model should be valid"),
        &phase_plan,
        capacities,
        Some(10),
        Metadata::new(),
    )
    .expect("aggregated long-term schedule should build");

    assert_eq!(schedule.entries().len(), 3);
    assert_eq!(schedule.entries()[0].block_count(), 1);
    assert_eq!(schedule.entries()[1].block_count(), 0);
    assert_eq!(schedule.entries()[2].block_count(), 1);
    assert_eq!(schedule.entries()[1].period_label(), "P2");
    assert_eq!(
        schedule
            .entries()
            .iter()
            .map(|entry| entry.tonnage())
            .sum::<f64>(),
        30.0
    );
}

#[test]
fn build_scheduling_problem_from_explicit_contract() {
    let destination_id = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let stockpile_id = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let plant_resource =
        SchedulingResourceId::new("plant_tonnage").expect("resource should be valid");
    let unit_a_id = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("unit-b").expect("unit id should be valid");

    let problem = SchedulingProblem::new(
        ScenarioId::new("schedule-problem").expect("scenario should be valid"),
        ModelId::new("model-problem").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), Some(500.0), Some(1_000.0))
                        .expect("mine bound should be valid"),
                    SchedulingResourceBound::new(plant_resource.clone(), None, Some(700.0))
                        .expect("plant bound should be valid"),
                ],
                vec![
                    ScheduleDestinationCapacity::new(destination_id.clone(), Some(700.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(250.0), Some(80.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), Some(500.0), Some(1_000.0))
                        .expect("mine bound should be valid"),
                    SchedulingResourceBound::new(plant_resource.clone(), None, Some(700.0))
                        .expect("plant bound should be valid"),
                ],
                vec![
                    ScheduleDestinationCapacity::new(destination_id.clone(), Some(700.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(300.0), Some(100.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                600.0,
                2,
                vec![],
                vec![destination_id.clone()],
                vec![stockpile_id.clone()],
                vec![0, 1],
                Some(100),
                Some(0),
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                550.0,
                2,
                vec![unit_a_id.clone()],
                vec![destination_id.clone()],
                vec![],
                vec![2, 3],
                Some(95),
                Some(1),
                Metadata::new(),
            )
            .expect("unit b should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), Some(destination_id.clone()), 120.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), Some(destination_id.clone()), 80.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(
                unit_a_id.clone(),
                mine_resource.clone(),
                None,
                600.0,
            )
            .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(
                unit_a_id,
                plant_resource.clone(),
                Some(destination_id.clone()),
                600.0,
            )
            .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(unit_b_id, mine_resource, None, 550.0)
                .expect("resource requirement should be valid"),
        ],
        vec![destination_id],
        vec![
            LongTermScheduleStockpile::new(stockpile_id, 50.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.10,
        Metadata::new(),
        vec![],
    )
    .expect("scheduling problem should build");

    assert_eq!(problem.periods().len(), 2);
    assert_eq!(problem.units().len(), 2);
    assert_eq!(problem.objective_terms().len(), 2);
    assert_eq!(problem.resource_requirements().len(), 3);
    assert_eq!(problem.discount_rate(), 0.10);
    assert_eq!(
        problem.units()[1].predecessor_unit_ids()[0].as_str(),
        "unit-a"
    );
}

#[test]
fn reject_scheduling_problem_with_unknown_predecessor() {
    let error = SchedulingProblem::new(
        ScenarioId::new("schedule-problem").expect("scenario should be valid"),
        ModelId::new("model-problem").expect("model should be valid"),
        vec![SchedulingPeriod::new("P1", vec![], vec![], vec![]).expect("period should be valid")],
        vec![
            SchedulingUnit::new(
                SchedulingUnitId::new("unit-a").expect("unit id should be valid"),
                600.0,
                1,
                vec![SchedulingUnitId::new("unit-missing").expect("unit id should be valid")],
                vec![],
                vec![],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
        ],
        vec![],
        vec![],
        vec![],
        vec![],
        0.10,
        Metadata::new(),
        vec![],
    )
    .expect_err("unknown predecessor should fail");

    assert_eq!(
        error,
        MineError::Validation {
            message: "unit `unit-a` references unknown predecessor `unit-missing`".to_owned(),
        }
    );
}

#[test]
fn reject_scheduling_problem_with_unbounded_stockpile_deposit_contract() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let error = SchedulingProblem::new(
        ScenarioId::new("stockpile-contract").expect("scenario should be valid"),
        ModelId::new("stockpile-contract-model").expect("model should be valid"),
        vec![SchedulingPeriod::new("P1", vec![], vec![], vec![]).expect("period should be valid")],
        vec![
            SchedulingUnit::new(
                SchedulingUnitId::new("phase-stockpile").expect("unit id should be valid"),
                25.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
        ],
        vec![],
        vec![],
        vec![],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 0.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect_err("stockpile deposit contract should fail");

    assert_eq!(
        error,
        MineError::Validation {
            message: "stockpile routing for `sp-main` requires an explicit stockpile capacity in period `P1`".to_owned(),
        }
    );
}

#[test]
fn derive_scheduling_problem_from_pushback_plan_and_capacities() {
    let phase_plan = PushbackPlan {
        phases: vec![mine_planning::PhaseDesign {
            phase_id: "phase-a".to_owned(),
            pushback_index: 0,
            shell_index: Some(0),
            revenue_factor: Some(0.8),
            bench: Some(101),
            block_indices: vec![0, 1],
            block_count: 2,
            total_tonnage: Some(800.0),
            predecessor_phase_ids: vec![],
        }],
        phase_count: 1,
        total_block_count: 2,
        total_tonnage: Some(800.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: vec![],
    };
    let destination_id = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let stockpile_id = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");

    let problem = SchedulingProblem::from_pushback_plan(
        ScenarioId::new("derived-problem").expect("scenario should be valid"),
        ModelId::new("derived-model").expect("model should be valid"),
        &phase_plan,
        vec![
            LongTermSchedulePeriodCapacity::new(
                "P1",
                Some(1_000.0),
                Some(700.0),
                vec![
                    ScheduleDestinationCapacity::new(destination_id.clone(), Some(700.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(300.0), Some(120.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("capacity should be valid"),
        ],
        vec![
            LongTermScheduleStockpile::new(stockpile_id, 25.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.08,
        Metadata::new(),
    )
    .expect("derived scheduling problem should build");

    assert_eq!(problem.units().len(), 1);
    assert_eq!(problem.units()[0].unit_id().as_str(), "phase-a");
    assert_eq!(problem.periods().len(), 1);
    assert_eq!(problem.periods()[0].resource_bounds().len(), 2);
    assert_eq!(problem.destination_ids(), &[destination_id]);
    assert_eq!(problem.stockpiles().len(), 1);
    assert_eq!(problem.limitations().len(), 1);
}

#[test]
fn solve_small_scheduling_problem_respects_precedence_and_capacity() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let unit_a_id = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("unit-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("small-solver").expect("scenario should be valid"),
        ModelId::new("small-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("resource bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("resource bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                1.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                1.0,
                1,
                vec![unit_a_id.clone()],
                vec![],
                vec![],
                vec![1],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 10.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), None, 8.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(unit_a_id, mine_resource.clone(), None, 1.0)
                .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(unit_b_id, mine_resource, None, 1.0)
                .expect("resource requirement should be valid"),
        ],
        vec![],
        vec![],
        0.10,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = solve_small_scheduling_problem(&problem).expect("solver should find solution");

    assert_eq!(solution.assignments().len(), 2);
    assert_eq!(solution.assignments()[0].period_label(), "P1");
    assert_eq!(solution.assignments()[1].period_label(), "P2");
    assert_eq!(solution.assignments()[1].unit_id().as_str(), "unit-b");
    assert!((solution.total_discounted_objective_value() - (10.0 + 8.0 / 1.1)).abs() < 1.0e-9);
}

#[test]
fn solve_small_scheduling_problem_chooses_best_destination() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let plant_resource =
        SchedulingResourceId::new("plant_tonnage").expect("resource should be valid");
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let waste = ScheduleDestinationId::new("waste").expect("destination should be valid");
    let unit_id = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("small-destination").expect("scenario should be valid"),
        ModelId::new("small-destination-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("mine bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("mine bound should be valid"),
                    SchedulingResourceBound::new(plant_resource.clone(), None, Some(1.0))
                        .expect("plant bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_id.clone(),
                1.0,
                1,
                vec![],
                vec![mill.clone(), waste.clone()],
                vec![],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_id.clone(), Some(mill.clone()), 15.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_id.clone(), Some(waste.clone()), 5.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(unit_id.clone(), mine_resource.clone(), None, 1.0)
                .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(unit_id, plant_resource, Some(mill.clone()), 1.0)
                .expect("resource requirement should be valid"),
        ],
        vec![mill.clone(), waste],
        vec![],
        0.10,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution: SmallSchedulingSolution =
        solve_small_scheduling_problem(&problem).expect("solver should find solution");

    assert_eq!(solution.assignments().len(), 1);
    assert_eq!(solution.assignments()[0].period_label(), "P2");
    assert_eq!(solution.assignments()[0].destination_id(), Some(&mill));
}

#[test]
fn solve_small_scheduling_problem_respects_cumulative_stockpile_inventory_capacity() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("small-stockpile-cap").expect("scenario should be valid"),
        ModelId::new("small-stockpile-cap-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(60.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(50.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![1],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit b should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 30.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), None, 25.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = solve_small_scheduling_problem(&problem).expect("solver should find solution");

    assert_eq!(solution.assignments().len(), 1);
    assert_eq!(solution.assignments()[0].stockpile_id(), Some(&stockpile));
    assert_eq!(solution.assignments()[0].unit_id(), &unit_a_id);
}

#[test]
fn solve_small_scheduling_problem_uses_explicit_stockpile_inventory_delta() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("small-stockpile-explicit-delta").expect("scenario should be valid"),
        ModelId::new("small-stockpile-explicit-delta-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(60.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(50.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit a should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(15.0))
            .expect("unit a explicit stockpile delta should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![1],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit b should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(15.0))
            .expect("unit b explicit stockpile delta should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 30.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), None, 25.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = solve_small_scheduling_problem(&problem).expect("solver should find solution");

    assert_eq!(solution.assignments().len(), 2);
    assert!(
        solution
            .assignments()
            .iter()
            .all(|assignment| assignment.stockpile_id() == Some(&stockpile))
    );
    assert!(
        solution
            .assignments()
            .iter()
            .all(|assignment| assignment.stockpile_inventory_delta_tonnage() == 15.0)
    );
    assert_eq!(solution.periods().len(), 2);
    assert_eq!(solution.periods()[0].stockpile_usage().len(), 1);
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].stockpile_id(),
        &stockpile
    );
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].opening_tonnage(),
        20.0
    );
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].inventory_delta_tonnage(),
        30.0
    );
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].closing_tonnage(),
        50.0
    );
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].max_inventory_tonnage(),
        Some(60.0)
    );
    assert_eq!(solution.periods()[1].stockpile_usage().len(), 1);
    assert_eq!(
        solution.periods()[1].stockpile_usage()[0].opening_tonnage(),
        50.0
    );
    assert_eq!(
        solution.periods()[1].stockpile_usage()[0].inventory_delta_tonnage(),
        0.0
    );
    assert_eq!(
        solution.periods()[1].stockpile_usage()[0].closing_tonnage(),
        50.0
    );
}

#[test]
fn build_target_period_windowed_schedule_preserves_explicit_stockpile_inventory_delta() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let unit_c_id = SchedulingUnitId::new("phase-c").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("windowed-stockpile-explicit-delta").expect("scenario should be valid"),
        ModelId::new("windowed-stockpile-explicit-delta-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(35.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(50.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit a should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(15.0))
            .expect("unit a explicit stockpile delta should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![1],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit b should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(15.0))
            .expect("unit b explicit stockpile delta should be valid"),
            SchedulingUnit::new(
                unit_c_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![2],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit c should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(15.0))
            .expect("unit c explicit stockpile delta should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 30.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), None, 25.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_c_id.clone(), None, 20.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = build_target_period_windowed_schedule(
        &problem,
        &BTreeMap::from([
            (unit_a_id.clone(), 0usize),
            (unit_b_id.clone(), 0usize),
            (unit_c_id.clone(), 1usize),
        ]),
        3,
    )
    .expect("windowed heuristic should build");

    assert_eq!(solution.assignments().len(), 2);
    assert_eq!(solution.assignments()[0].unit_id(), &unit_a_id);
    assert_eq!(solution.assignments()[1].unit_id(), &unit_b_id);
}

#[test]
fn build_ready_frontier_schedule_accepts_zero_stockpile_inventory_delta() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let unit_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-stockpile-zero-delta").expect("scenario should be valid"),
        ModelId::new("frontier-stockpile-zero-delta-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(20.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(0.0))
            .expect("zero stockpile delta should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_id.clone(), None, 10.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = build_ready_frontier_schedule(&problem).expect("heuristic should build");

    assert_eq!(solution.assignments().len(), 1);
    assert_eq!(solution.assignments()[0].stockpile_id(), Some(&stockpile));
    assert_eq!(
        solution.assignments()[0].stockpile_inventory_delta_tonnage(),
        0.0
    );
    assert_eq!(solution.periods().len(), 1);
    assert_eq!(solution.periods()[0].stockpile_usage().len(), 1);
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].opening_tonnage(),
        20.0
    );
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].inventory_delta_tonnage(),
        0.0
    );
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].closing_tonnage(),
        20.0
    );
}

#[test]
fn build_ready_frontier_long_term_schedule_materializes_reclaim_destination() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let unit_id = SchedulingUnitId::new("reclaim-a").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-reclaim").expect("scenario should be valid"),
        ModelId::new("frontier-reclaim-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![],
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(20.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(20.0), Some(20.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_id.clone(),
                15.0,
                1,
                vec![],
                vec![mill.clone()],
                vec![stockpile.clone()],
                vec![],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(-15.0))
            .expect("reclaim stockpile delta should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_id, Some(mill.clone()), 30.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![mill.clone()],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let schedule = build_ready_frontier_long_term_schedule(&problem, None, Metadata::new())
        .expect("long-term reclaim schedule should build");
    let flow_report = evaluate_long_term_schedule_material_flows(&schedule)
        .expect("material flow report should build");

    assert_eq!(schedule.entries().len(), 1);
    assert_eq!(schedule.entries()[0].phase_id(), Some("reclaim-a"));
    assert_eq!(schedule.entries()[0].destination_id(), Some(&mill));
    assert_eq!(schedule.entries()[0].stockpile_id(), None);
    assert_eq!(
        schedule.entries()[0].reclaim_stockpile_id(),
        Some(&stockpile)
    );
    assert_eq!(flow_report.period_flows[0].mined_tonnage, 0.0);
    assert_eq!(
        flow_report.period_flows[0].destination_tonnage["mill"],
        15.0
    );
    assert_eq!(
        flow_report.period_flows[0].stockpile_reclaims["sp-main"],
        15.0
    );
    assert_eq!(flow_report.stockpile_balances[0].opening_tonnage, 20.0);
    assert_eq!(flow_report.stockpile_balances[0].closing_tonnage, 5.0);
}

#[test]
fn solve_small_scheduling_problem_tracks_reclaim_capacity_resource_usage() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let unit_id = SchedulingUnitId::new("reclaim-a").expect("unit id should be valid");
    let reclaim_resource = stockpile_reclaim_capacity_resource_id(&stockpile)
        .expect("reclaim capacity resource should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("small-reclaim-resource").expect("scenario should be valid"),
        ModelId::new("small-reclaim-resource-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(reclaim_resource.clone(), None, Some(15.0))
                        .expect("resource bound should be valid"),
                ],
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(15.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(20.0), Some(15.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_id.clone(),
                15.0,
                1,
                vec![],
                vec![mill.clone()],
                vec![stockpile.clone()],
                vec![],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(-15.0))
            .expect("reclaim stockpile delta should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_id.clone(), Some(mill.clone()), 30.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(unit_id, reclaim_resource.clone(), None, 15.0)
                .expect("resource requirement should be valid"),
        ],
        vec![mill],
        vec![
            LongTermScheduleStockpile::new(stockpile, 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = solve_small_scheduling_problem(&problem).expect("small scheduling should solve");

    assert_eq!(solution.assignments().len(), 1);
    assert_eq!(
        solution.periods()[0].stockpile_usage()[0].closing_tonnage(),
        5.0
    );
    let reclaim_usage = solution.periods()[0]
        .resource_usage()
        .iter()
        .find(|usage| usage.resource_id() == &reclaim_resource)
        .expect("reclaim resource usage should be reported");
    assert_eq!(reclaim_usage.total(), 15.0);
    assert_eq!(reclaim_usage.max_total(), Some(15.0));
}

#[test]
fn scheduling_problem_rejects_reclaim_without_destination_routing() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let error = SchedulingProblem::new(
        ScenarioId::new("reclaim-without-destination").expect("scenario should be valid"),
        ModelId::new("reclaim-without-destination-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(20.0), Some(20.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                SchedulingUnitId::new("phase-a").expect("unit id should be valid"),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid")
            .with_stockpile_inventory_delta_tonnage(Some(-1.0))
            .expect("negative stockpile delta should be valid for reclaim contracts"),
        ],
        vec![],
        vec![],
        vec![],
        vec![
            LongTermScheduleStockpile::new(stockpile, 10.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect_err("reclaim without destination routing should fail");

    assert_eq!(
        error,
        MineError::Validation {
            message:
                "unit `phase-a` declares reclaim inventory delta without any eligible destination routing"
                    .to_owned(),
        }
    );
}

#[test]
fn build_ready_frontier_schedule_prioritizes_value_under_capacity() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let unit_a_id = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("unit-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-solver").expect("scenario should be valid"),
        ModelId::new("frontier-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("resource bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("resource bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                1.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                1.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![1],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id, None, 12.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id, None, 7.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(
                SchedulingUnitId::new("unit-a").expect("unit id should be valid"),
                mine_resource.clone(),
                None,
                1.0,
            )
            .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(
                SchedulingUnitId::new("unit-b").expect("unit id should be valid"),
                mine_resource,
                None,
                1.0,
            )
            .expect("resource requirement should be valid"),
        ],
        vec![],
        vec![],
        0.10,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = build_ready_frontier_schedule(&problem).expect("heuristic should build");

    assert_eq!(solution.assignments().len(), 2);
    assert_eq!(solution.assignments()[0].period_label(), "P1");
    assert_eq!(solution.assignments()[0].unit_id().as_str(), "unit-a");
}

#[test]
fn build_ready_frontier_schedule_respects_destination_capacity_in_period() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let plant_resource =
        SchedulingResourceId::new("plant_tonnage").expect("resource should be valid");
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let waste = ScheduleDestinationId::new("waste").expect("destination should be valid");
    let unit_id = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-destination").expect("scenario should be valid"),
        ModelId::new("frontier-destination-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("mine bound should be valid"),
                    SchedulingResourceBound::new(plant_resource.clone(), None, Some(1.0))
                        .expect("plant bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_id.clone(),
                1.0,
                1,
                vec![],
                vec![mill.clone(), waste.clone()],
                vec![],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_id.clone(), Some(mill.clone()), 15.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_id.clone(), Some(waste.clone()), 5.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(unit_id.clone(), mine_resource.clone(), None, 1.0)
                .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(unit_id, plant_resource, Some(mill.clone()), 1.0)
                .expect("resource requirement should be valid"),
        ],
        vec![mill.clone(), waste],
        vec![],
        0.10,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution: SmallSchedulingSolution =
        build_ready_frontier_schedule(&problem).expect("heuristic should build");

    assert_eq!(solution.assignments().len(), 1);
    assert_eq!(solution.assignments()[0].destination_id(), Some(&mill));
}

#[test]
fn build_target_period_seeded_schedule_prioritizes_lp_target_proximity() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("seeded-frontier").expect("scenario should be valid"),
        ModelId::new("seeded-frontier-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("mine bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("mine bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                1.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![0],
                Some(110),
                Some(0),
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                1.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![1],
                Some(105),
                Some(0),
                Metadata::new(),
            )
            .expect("unit b should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 9.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), None, 10.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(unit_a_id.clone(), mine_resource.clone(), None, 1.0)
                .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(unit_b_id.clone(), mine_resource, None, 1.0)
                .expect("resource requirement should be valid"),
        ],
        vec![],
        vec![],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let solution = build_target_period_seeded_schedule(
        &problem,
        &BTreeMap::from([(unit_a_id.clone(), 0usize), (unit_b_id.clone(), 1usize)]),
    )
    .expect("seeded heuristic should build");

    assert_eq!(solution.assignments().len(), 2);
    assert_eq!(solution.assignments()[0].unit_id(), &unit_a_id);
    assert_eq!(solution.assignments()[0].period_label(), "P1");
    assert_eq!(solution.assignments()[1].unit_id(), &unit_b_id);
    assert_eq!(solution.assignments()[1].period_label(), "P2");
}

#[test]
fn build_target_period_windowed_schedule_beats_greedy_pack_under_capacity() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let unit_a_id = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("unit-b").expect("unit id should be valid");
    let unit_c_id = SchedulingUnitId::new("unit-c").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("windowed-pack").expect("scenario should be valid"),
        ModelId::new("windowed-pack-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(10.0))
                        .expect("mine bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                6.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                5.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![1],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit b should be valid"),
            SchedulingUnit::new(
                unit_c_id.clone(),
                5.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![2],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit c should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 66.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), None, 55.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_c_id.clone(), None, 54.0)
                .expect("objective term should be valid"),
        ],
        vec![
            SchedulingResourceRequirement::new(unit_a_id.clone(), mine_resource.clone(), None, 6.0)
                .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(unit_b_id.clone(), mine_resource.clone(), None, 5.0)
                .expect("resource requirement should be valid"),
            SchedulingResourceRequirement::new(unit_c_id.clone(), mine_resource, None, 5.0)
                .expect("resource requirement should be valid"),
        ],
        vec![],
        vec![],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let greedy = build_ready_frontier_schedule(&problem).expect("greedy heuristic should build");
    let windowed = build_target_period_windowed_schedule(
        &problem,
        &BTreeMap::from([
            (unit_a_id.clone(), 0usize),
            (unit_b_id.clone(), 0usize),
            (unit_c_id.clone(), 0usize),
        ]),
        3,
    )
    .expect("windowed heuristic should build");

    assert_eq!(greedy.assignments().len(), 1);
    assert_eq!(greedy.assignments()[0].unit_id(), &unit_a_id);
    assert_eq!(windowed.assignments().len(), 2);
    assert_eq!(windowed.total_objective_value(), 109.0);
}

#[test]
fn build_ready_frontier_long_term_schedule_routes_destinations_during_construction() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let waste = ScheduleDestinationId::new("waste").expect("destination should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-long-term").expect("scenario should be valid"),
        ModelId::new("frontier-long-term-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource, None, Some(100.0))
                        .expect("mine bound should be valid"),
                ],
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(60.0))
                        .expect("mill capacity should be valid"),
                    ScheduleDestinationCapacity::new(waste.clone(), Some(100.0))
                        .expect("waste capacity should be valid"),
                ],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                60.0,
                3,
                vec![],
                vec![mill.clone(), waste.clone()],
                vec![],
                vec![0, 1, 2],
                Some(110),
                Some(0),
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                40.0,
                2,
                vec![],
                vec![mill.clone(), waste.clone()],
                vec![],
                vec![3, 4],
                Some(105),
                Some(0),
                Metadata::new(),
            )
            .expect("unit b should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), Some(mill.clone()), 120.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_a_id, Some(waste.clone()), 5.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), Some(mill.clone()), 80.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id, Some(waste.clone()), 10.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![mill.clone(), waste.clone()],
        vec![],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let schedule = build_ready_frontier_long_term_schedule(&problem, None, Metadata::new())
        .expect("long-term schedule should build");
    let flow_report = evaluate_long_term_schedule_material_flows(&schedule)
        .expect("material flow report should build");

    assert_eq!(schedule.entries().len(), 2);
    assert_eq!(schedule.entries()[0].destination_id(), Some(&mill));
    assert_eq!(schedule.entries()[1].destination_id(), Some(&waste));
    assert_eq!(flow_report.period_flows.len(), 1);
    assert_eq!(
        flow_report.period_flows[0]
            .destination_tonnage
            .get("mill")
            .copied(),
        Some(60.0)
    );
    assert_eq!(
        flow_report.period_flows[0]
            .destination_tonnage
            .get("waste")
            .copied(),
        Some(40.0)
    );
    assert!(flow_report.violations.is_empty());
}

#[test]
fn build_ready_frontier_long_term_schedule_routes_stockpile_deposits_during_construction() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let unit_id = SchedulingUnitId::new("phase-stockpile").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-stockpile").expect("scenario should be valid"),
        ModelId::new("frontier-stockpile-model").expect("model should be valid"),
        vec![SchedulingPeriod::new(
            "P1",
            vec![SchedulingResourceBound::new(mine_resource, None, Some(80.0))
                .expect("mine bound should be valid")],
            vec![],
            vec![
                ScheduleStockpileCapacity::new(stockpile.clone(), Some(100.0), None)
                    .expect("stockpile capacity should be valid"),
            ],
        )
        .expect("period should be valid")],
        vec![SchedulingUnit::new(
            unit_id.clone(),
            80.0,
            4,
            vec![],
            vec![],
            vec![stockpile.clone()],
            vec![0, 1, 2, 3],
            Some(110),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid")],
        vec![SchedulingObjectiveTerm::new(unit_id, None, 75.0)
            .expect("objective term should be valid")],
        vec![],
        vec![],
        vec![LongTermScheduleStockpile::new(stockpile.clone(), 0.0, Metadata::new())
            .expect("stockpile should be valid")],
        0.0,
        Metadata::new(),
        vec![
            "stockpile deposit routing in small_scheduling currently uses generic objective/resource terms; reclaim remains outside the scheduler".to_owned(),
        ],
    )
    .expect("problem should be valid");

    let schedule = build_ready_frontier_long_term_schedule(&problem, None, Metadata::new())
        .expect("long-term schedule should build");
    let flow_report = evaluate_long_term_schedule_material_flows(&schedule)
        .expect("material flow report should build");

    assert_eq!(schedule.entries().len(), 1);
    assert_eq!(schedule.entries()[0].destination_id(), None);
    assert_eq!(schedule.entries()[0].stockpile_id(), Some(&stockpile));
    assert_eq!(
        flow_report.period_flows[0]
            .stockpile_deposits
            .get("sp-main")
            .copied(),
        Some(80.0)
    );
    assert_eq!(flow_report.stockpile_balances[0].closing_tonnage, 80.0);
    assert!(flow_report.period_flows[0].destination_tonnage.is_empty());
}

#[test]
fn build_ready_frontier_long_term_schedule_respects_future_stockpile_inventory_capacity() {
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-stockpile-cap").expect("scenario should be valid"),
        ModelId::new("frontier-stockpile-cap-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(60.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![],
                vec![],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(50.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![0],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                20.0,
                1,
                vec![],
                vec![],
                vec![stockpile.clone()],
                vec![1],
                None,
                None,
                Metadata::new(),
            )
            .expect("unit b should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 30.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), None, 25.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let schedule = build_ready_frontier_long_term_schedule(&problem, None, Metadata::new())
        .expect("long-term schedule should build");
    let flow_report = evaluate_long_term_schedule_material_flows(&schedule)
        .expect("material flow report should build");

    assert_eq!(schedule.entries().len(), 1);
    assert_eq!(schedule.entries()[0].stockpile_id(), Some(&stockpile));
    assert_eq!(schedule.entries()[0].phase_id(), Some("phase-a"));
    assert_eq!(flow_report.stockpile_balances.len(), 2);
    assert_eq!(flow_report.stockpile_balances[0].closing_tonnage, 40.0);
    assert_eq!(flow_report.stockpile_balances[1].closing_tonnage, 40.0);
    assert!(flow_report.violations.is_empty());
}

#[test]
fn build_ready_frontier_long_term_schedule_reports_vertical_advance_violation() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("frontier-vertical").expect("scenario should be valid"),
        ModelId::new("frontier-vertical-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("mine bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![
                    SchedulingResourceBound::new(mine_resource, None, Some(1.0))
                        .expect("mine bound should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                1.0,
                1,
                vec![],
                vec![],
                vec![],
                vec![0],
                Some(110),
                Some(0),
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                1.0,
                1,
                vec![unit_a_id.clone()],
                vec![],
                vec![],
                vec![1],
                Some(95),
                Some(0),
                Metadata::new(),
            )
            .expect("unit b should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), None, 10.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id, None, 9.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![],
        vec![],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");

    let schedule = build_ready_frontier_long_term_schedule(&problem, Some(10), Metadata::new())
        .expect("long-term schedule should build");

    assert_eq!(schedule.violations().len(), 1);
    assert_eq!(
        schedule.violations()[0].code,
        LongTermScheduleViolationCode::ExceedsVerticalAdvance
    );
}

#[test]
fn apply_long_term_stockpile_policy_diverts_and_reclaims_material() {
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let stockpile_id = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let base_schedule = LongTermSchedule::new(
        ScenarioId::new("stockpile-policy").expect("scenario should be valid"),
        ModelId::new("stockpile-policy-model").expect("model should be valid"),
        vec![
            LongTermScheduleEntry::new(
                "P1",
                Some("phase-a".to_owned()),
                Some(0),
                Some(110),
                100.0,
                10,
                Some(mill.clone()),
                None,
                vec![],
            )
            .expect("entry should be valid"),
            LongTermScheduleEntry::new(
                "P2",
                Some("phase-b".to_owned()),
                Some(0),
                Some(105),
                80.0,
                8,
                Some(mill.clone()),
                None,
                vec!["phase-a".to_owned()],
            )
            .expect("entry should be valid"),
        ],
        vec![
            LongTermSchedulePeriodCapacity::new(
                "P1",
                Some(100.0),
                Some(100.0),
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(100.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(50.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period capacity should be valid"),
            LongTermSchedulePeriodCapacity::new(
                "P2",
                Some(120.0),
                Some(120.0),
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(120.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(50.0), Some(50.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period capacity should be valid"),
        ],
        vec![
            LongTermScheduleStockpile::new(stockpile_id.clone(), 0.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        vec![],
        Metadata::new(),
    )
    .expect("base schedule should be valid");
    let policy = LongTermStockpilePolicy::new(
        vec![
            LongTermStockpileDepositPolicy::new("P1", mill.clone(), stockpile_id.clone(), 40.0)
                .expect("deposit policy should be valid"),
        ],
        vec![
            LongTermStockpileReclaimPolicy::new("P2", stockpile_id.clone(), mill.clone(), 40.0)
                .expect("reclaim policy should be valid"),
        ],
    )
    .expect("stockpile policy should be valid");

    let updated_schedule =
        apply_long_term_stockpile_policy(&base_schedule, &policy, Metadata::new())
            .expect("stockpile policy should apply");
    let flow_report = evaluate_long_term_schedule_material_flows(&updated_schedule)
        .expect("material flow report should evaluate");

    assert_eq!(updated_schedule.entries().len(), 4);
    assert!(updated_schedule.violations().is_empty());
    assert_eq!(
        flow_report.period_flows[0]
            .destination_tonnage
            .get("mill")
            .copied(),
        Some(60.0)
    );
    assert_eq!(
        flow_report.period_flows[0]
            .stockpile_deposits
            .get("sp-main")
            .copied(),
        Some(40.0)
    );
    assert_eq!(
        flow_report.period_flows[1]
            .stockpile_reclaims
            .get("sp-main")
            .copied(),
        Some(40.0)
    );
    assert_eq!(flow_report.stockpile_balances.len(), 2);
    assert_eq!(flow_report.stockpile_balances[0].closing_tonnage, 40.0);
    assert_eq!(flow_report.stockpile_balances[1].closing_tonnage, 0.0);
}

#[test]
fn solve_decomposed_scheduling_problem_returns_candidate_bound_and_stockpile_adjusted_schedule() {
    let mine_resource =
        SchedulingResourceId::new("mine_tonnage").expect("resource should be valid");
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let stockpile_id = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let unit_a_id = SchedulingUnitId::new("phase-a").expect("unit id should be valid");
    let unit_b_id = SchedulingUnitId::new("phase-b").expect("unit id should be valid");
    let problem = SchedulingProblem::new(
        ScenarioId::new("decomposed-problem").expect("scenario should be valid"),
        ModelId::new("decomposed-model").expect("model should be valid"),
        vec![
            SchedulingPeriod::new(
                "P1",
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(100.0))
                        .expect("mine bound should be valid"),
                ],
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(100.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(50.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
            SchedulingPeriod::new(
                "P2",
                vec![
                    SchedulingResourceBound::new(mine_resource, None, Some(120.0))
                        .expect("mine bound should be valid"),
                ],
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(120.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(50.0), Some(50.0))
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        vec![
            SchedulingUnit::new(
                unit_a_id.clone(),
                100.0,
                10,
                vec![],
                vec![mill.clone()],
                vec![stockpile_id.clone()],
                vec![0, 1, 2],
                Some(110),
                Some(0),
                Metadata::new(),
            )
            .expect("unit a should be valid"),
            SchedulingUnit::new(
                unit_b_id.clone(),
                80.0,
                8,
                vec![unit_a_id.clone()],
                vec![mill.clone()],
                vec![],
                vec![3, 4],
                Some(105),
                Some(0),
                Metadata::new(),
            )
            .expect("unit b should be valid"),
        ],
        vec![
            SchedulingObjectiveTerm::new(unit_a_id.clone(), Some(mill.clone()), 120.0)
                .expect("objective term should be valid"),
            SchedulingObjectiveTerm::new(unit_b_id.clone(), Some(mill.clone()), 80.0)
                .expect("objective term should be valid"),
        ],
        vec![],
        vec![mill.clone()],
        vec![
            LongTermScheduleStockpile::new(stockpile_id.clone(), 0.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        0.0,
        Metadata::new(),
        vec![],
    )
    .expect("problem should be valid");
    let stockpile_policy = LongTermStockpilePolicy::new(
        vec![
            LongTermStockpileDepositPolicy::new("P1", mill.clone(), stockpile_id.clone(), 40.0)
                .expect("deposit policy should be valid"),
        ],
        vec![
            LongTermStockpileReclaimPolicy::new("P2", stockpile_id.clone(), mill.clone(), 40.0)
                .expect("reclaim policy should be valid"),
        ],
    )
    .expect("stockpile policy should be valid");
    let config = DecomposedSchedulingConfig::ready_frontier()
        .with_reference_bound_solver(Some(DecomposedTemporalSolver::SmallExact))
        .with_stockpile_policy(Some(stockpile_policy));

    let artifacts = solve_decomposed_scheduling_problem(&problem, &config, Metadata::new())
        .expect("decomposed schedule should solve");
    let flow_report = evaluate_long_term_schedule_material_flows(artifacts.final_schedule())
        .expect("material flow report should build");
    let reference_bound = artifacts
        .reference_bound()
        .expect("reference bound should be present");

    assert_eq!(artifacts.temporal_candidate().assignments().len(), 2);
    assert_eq!(artifacts.routed_schedule().entries().len(), 2);
    assert_eq!(artifacts.final_schedule().entries().len(), 4);
    assert!(
        reference_bound.total_discounted_objective_value()
            >= artifacts
                .temporal_candidate()
                .total_discounted_objective_value()
    );
    assert_eq!(
        flow_report
            .period_flows
            .iter()
            .find(|period| period.period_label == "P2")
            .and_then(|period| period.stockpile_reclaims.get("sp-main").copied()),
        Some(40.0)
    );
}

#[test]
fn apply_long_term_stockpile_policy_rejects_deposit_above_available_tonnage() {
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let stockpile_id = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let base_schedule = LongTermSchedule::new(
        ScenarioId::new("stockpile-policy-error").expect("scenario should be valid"),
        ModelId::new("stockpile-policy-error-model").expect("model should be valid"),
        vec![
            LongTermScheduleEntry::new(
                "P1",
                Some("phase-a".to_owned()),
                Some(0),
                Some(110),
                100.0,
                10,
                Some(mill.clone()),
                None,
                vec![],
            )
            .expect("entry should be valid"),
        ],
        vec![
            LongTermSchedulePeriodCapacity::new(
                "P1",
                Some(100.0),
                Some(100.0),
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(100.0))
                        .expect("destination capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile_id.clone(), Some(80.0), None)
                        .expect("stockpile capacity should be valid"),
                ],
            )
            .expect("period capacity should be valid"),
        ],
        vec![
            LongTermScheduleStockpile::new(stockpile_id.clone(), 0.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        vec![],
        Metadata::new(),
    )
    .expect("base schedule should be valid");
    let policy = LongTermStockpilePolicy::new(
        vec![
            LongTermStockpileDepositPolicy::new("P1", mill.clone(), stockpile_id, 120.0)
                .expect("deposit policy should be valid"),
        ],
        vec![],
    )
    .expect("stockpile policy should be valid");

    let error = apply_long_term_stockpile_policy(&base_schedule, &policy, Metadata::new())
        .expect_err("policy should reject impossible diversion");
    assert!(error.to_string().contains("but only 100 t are available"));
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

#[test]
fn derive_phase_design_from_shells_uses_benches_and_precedence() {
    let shell_set = PitShellSet {
        shells: vec![
            PitShell {
                revenue_factor: 0.5,
                selected_blocks: vec![0, 1],
                pit_value: 5.0,
                block_count: 2,
            },
            PitShell {
                revenue_factor: 1.0,
                selected_blocks: vec![0, 1, 2, 3],
                pit_value: 8.0,
                block_count: 4,
            },
        ],
        total_block_count: 4,
        factors_evaluated: 2,
        unique_shell_count: 2,
    };
    let bench_assignments = vec![
        BenchAssignment {
            linear_index: 0,
            bench: 101,
            center_elevation: 1010.0,
        },
        BenchAssignment {
            linear_index: 1,
            bench: 100,
            center_elevation: 1000.0,
        },
        BenchAssignment {
            linear_index: 2,
            bench: 100,
            center_elevation: 1000.0,
        },
        BenchAssignment {
            linear_index: 3,
            bench: 99,
            center_elevation: 990.0,
        },
    ];
    let precedence_graph = PrecedenceGraph::new(vec![
        PrecedenceEdge::new(PrecedenceNode::Block(1), PrecedenceNode::Block(0)),
        PrecedenceEdge::new(PrecedenceNode::Block(2), PrecedenceNode::Block(3)),
    ])
    .expect("precedence graph should be valid");

    let plan = derive_phase_design_from_nested_shells(
        &shell_set,
        &bench_assignments,
        &precedence_graph,
        Some(&[100.0, 120.0, 150.0, 180.0]),
        NestingAccessRules::strict_sequential(),
    )
    .expect("phase design should derive");

    assert_eq!(plan.phase_count, 4);
    assert_eq!(plan.phases[0].bench, Some(101));
    assert_eq!(plan.phases[1].bench, Some(100));
    assert_eq!(plan.phases[2].bench, Some(100));
    assert_eq!(plan.phases[3].bench, Some(99));

    assert!(
        plan.phases[2]
            .predecessor_phase_ids
            .iter()
            .any(|phase_id| phase_id == "phase-s00-b101")
    );
    assert!(
        plan.phases[2]
            .predecessor_phase_ids
            .iter()
            .any(|phase_id| phase_id == "phase-s00-b100")
    );
    assert!(
        plan.phases[3]
            .predecessor_phase_ids
            .iter()
            .any(|phase_id| phase_id == "phase-s01-b100")
    );
    assert_eq!(plan.phases[3].total_tonnage, Some(180.0));
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
fn pit_shell_set_json_roundtrip_preserves_membership_and_metrics() {
    let shell_set = PitShellSet {
        shells: vec![
            PitShell {
                revenue_factor: 0.6,
                selected_blocks: vec![0, 1],
                pit_value: 12.5,
                block_count: 2,
            },
            PitShell {
                revenue_factor: 1.0,
                selected_blocks: vec![0, 1, 3],
                pit_value: 18.0,
                block_count: 3,
            },
        ],
        total_block_count: 4,
        factors_evaluated: 3,
        unique_shell_count: 2,
    };
    let path = temporary_json_path("pit-shell-set");

    write_pit_shell_set_json(&shell_set, &path).expect("shell set should write");
    let decoded = read_pit_shell_set_json(&path).expect("shell set should read");

    assert_eq!(decoded, shell_set);

    let _ = fs::remove_file(path);
}

#[test]
fn material_flow_report_tracks_destination_and_stockpile_balances() {
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let schedule = LongTermSchedule::new(
        ScenarioId::new("routing-scenario").expect("scenario should be valid"),
        ModelId::new("routing-model").expect("model should be valid"),
        vec![
            LongTermScheduleEntry::new(
                "P1",
                Some("phase-a".to_owned()),
                Some(0),
                Some(100),
                40.0,
                4,
                Some(mill.clone()),
                None,
                vec![],
            )
            .expect("entry should be valid"),
            LongTermScheduleEntry::new(
                "P1",
                Some("phase-a".to_owned()),
                Some(0),
                Some(100),
                30.0,
                3,
                None,
                Some(stockpile.clone()),
                vec![],
            )
            .expect("entry should be valid"),
            LongTermScheduleEntry::new_with_reclaim(
                "P2",
                None,
                None,
                None,
                25.0,
                2,
                Some(mill.clone()),
                None,
                Some(stockpile.clone()),
                vec![],
            )
            .expect("reclaim entry should be valid"),
        ],
        vec![
            LongTermSchedulePeriodCapacity::new(
                "P1",
                Some(80.0),
                Some(50.0),
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(50.0))
                        .expect("capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(60.0), Some(30.0))
                        .expect("capacity should be valid"),
                ],
            )
            .expect("capacity should be valid"),
            LongTermSchedulePeriodCapacity::new(
                "P2",
                Some(30.0),
                Some(30.0),
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(30.0))
                        .expect("capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(60.0), Some(30.0))
                        .expect("capacity should be valid"),
                ],
            )
            .expect("capacity should be valid"),
        ],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 20.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        vec![],
        Metadata::new(),
    )
    .expect("schedule should be valid");

    let report = evaluate_long_term_schedule_material_flows(&schedule)
        .expect("material flow report should evaluate");

    assert_eq!(report.period_flows.len(), 2);
    assert_eq!(report.period_flows[0].period_label, "P1");
    assert_eq!(report.period_flows[0].mined_tonnage, 70.0);
    assert_eq!(report.period_flows[0].destination_tonnage["mill"], 40.0);
    assert_eq!(report.period_flows[0].stockpile_deposits["sp-main"], 30.0);
    assert!(report.period_flows[0].stockpile_reclaims.is_empty());

    assert_eq!(report.period_flows[1].period_label, "P2");
    assert_eq!(report.period_flows[1].mined_tonnage, 0.0);
    assert_eq!(report.period_flows[1].destination_tonnage["mill"], 25.0);
    assert_eq!(report.period_flows[1].stockpile_reclaims["sp-main"], 25.0);

    assert_eq!(report.stockpile_balances.len(), 2);
    assert_eq!(report.stockpile_balances[0].opening_tonnage, 20.0);
    assert_eq!(report.stockpile_balances[0].closing_tonnage, 50.0);
    assert_eq!(report.stockpile_balances[1].opening_tonnage, 50.0);
    assert_eq!(report.stockpile_balances[1].closing_tonnage, 25.0);
    assert!(report.violations.is_empty());
}

#[test]
fn material_flow_report_surfaces_destination_and_stockpile_violations() {
    let mill = ScheduleDestinationId::new("mill").expect("destination should be valid");
    let stockpile = ScheduleStockpileId::new("sp-main").expect("stockpile should be valid");
    let schedule = LongTermSchedule::new(
        ScenarioId::new("routing-violations").expect("scenario should be valid"),
        ModelId::new("routing-model").expect("model should be valid"),
        vec![
            LongTermScheduleEntry::new(
                "P1",
                Some("phase-a".to_owned()),
                Some(0),
                Some(100),
                35.0,
                3,
                Some(mill.clone()),
                None,
                vec![],
            )
            .expect("entry should be valid"),
            LongTermScheduleEntry::new(
                "P1",
                Some("phase-a".to_owned()),
                Some(0),
                Some(100),
                40.0,
                4,
                None,
                Some(stockpile.clone()),
                vec![],
            )
            .expect("entry should be valid"),
            LongTermScheduleEntry::new_with_reclaim(
                "P2",
                None,
                None,
                None,
                60.0,
                6,
                Some(mill.clone()),
                None,
                Some(stockpile.clone()),
                vec![],
            )
            .expect("reclaim entry should be valid"),
        ],
        vec![
            LongTermSchedulePeriodCapacity::new(
                "P1",
                Some(60.0),
                Some(30.0),
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(30.0))
                        .expect("capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(50.0), Some(20.0))
                        .expect("capacity should be valid"),
                ],
            )
            .expect("capacity should be valid"),
            LongTermSchedulePeriodCapacity::new(
                "P2",
                Some(60.0),
                Some(40.0),
                vec![
                    ScheduleDestinationCapacity::new(mill.clone(), Some(40.0))
                        .expect("capacity should be valid"),
                ],
                vec![
                    ScheduleStockpileCapacity::new(stockpile.clone(), Some(50.0), Some(20.0))
                        .expect("capacity should be valid"),
                ],
            )
            .expect("capacity should be valid"),
        ],
        vec![
            LongTermScheduleStockpile::new(stockpile.clone(), 10.0, Metadata::new())
                .expect("stockpile should be valid"),
        ],
        vec![],
        Metadata::new(),
    )
    .expect("schedule should be valid");

    let report = evaluate_long_term_schedule_material_flows(&schedule)
        .expect("material flow report should evaluate");
    let violation_codes = report
        .violations
        .iter()
        .map(|violation| violation.code)
        .collect::<Vec<_>>();

    assert!(violation_codes.contains(&LongTermScheduleViolationCode::ExceedsMineCapacity));
    assert!(violation_codes.contains(&LongTermScheduleViolationCode::ExceedsPlantCapacity));
    assert!(violation_codes.contains(&LongTermScheduleViolationCode::ExceedsDestinationCapacity));
    assert!(violation_codes.contains(&LongTermScheduleViolationCode::ExceedsStockpileReclaim));
    assert!(violation_codes.contains(&LongTermScheduleViolationCode::InvalidStockpileBalance));
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
