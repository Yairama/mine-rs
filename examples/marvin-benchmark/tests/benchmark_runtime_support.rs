//! Tests de integración para el soporte benchmark de runtime (MR-209/MR-211/MR-215):
//! telemetría por etapas, CLI compartido y adaptación CPIT TopoSort.

#[path = "../src/benchmark_cli_support.rs"]
mod benchmark_cli_support;
#[path = "../src/benchmark_runtime_telemetry.rs"]
mod benchmark_runtime_telemetry;
#[path = "../src/cpit_toposort_support.rs"]
mod cpit_toposort_support;
#[path = "../src/marvin_support.rs"]
mod marvin_support;

use std::collections::BTreeMap;

use benchmark_cli_support::parse_benchmark_cli_args;
use benchmark_runtime_telemetry::{RUNTIME_TELEMETRY_CONTRACT_VERSION, StageTimer};
use cpit_toposort_support::{
    build_expected_period_scores, build_toposort_problem_from_minelib_cpit,
    verify_schedule_precedence,
};
use marvin_support::{
    MarvinObjectiveTerm, MarvinResourceConstraintLimit, MarvinScheduleAssignment,
    MarvinScheduleProblem, MarvinScheduleProblemKind,
};
use mine_sdk::{
    CpitToposortAssignment, CpitToposortOptions, CpitToposortSchedule, PrecedenceEdge,
    PrecedenceGraph, PrecedenceNode, solve_cpit_with_toposort,
};

// ── Telemetría (MR-215) ──────────────────────────────────────────────────────

#[test]
fn stage_timer_records_ordered_stages_and_totals() {
    let mut timer = StageTimer::start();
    timer.record_stage("load");
    timer.record_stage("solve");
    let telemetry = timer.finish();

    assert_eq!(
        telemetry.contract_version,
        RUNTIME_TELEMETRY_CONTRACT_VERSION
    );
    assert_eq!(telemetry.stage_timings.len(), 2);
    assert_eq!(telemetry.stage_timings[0].stage, "load");
    assert_eq!(telemetry.stage_timings[1].stage, "solve");
    assert!(telemetry.total_wall_clock_ms >= 0.0);
    assert!(!telemetry.comparability_note.is_empty());
    assert!(!telemetry.limitations.is_empty());
}

// ── CLI compartido (MR-209/MR-211) ───────────────────────────────────────────

#[test]
fn parse_benchmark_cli_args_supports_flags_and_output_path() {
    let options = parse_benchmark_cli_args(&[
        "--include-full".to_owned(),
        "--quiet".to_owned(),
        "custom/output.json".to_owned(),
    ])
    .expect("args should parse");
    assert!(options.include_full);
    assert!(options.quiet);
    assert_eq!(
        options.output_path.as_deref(),
        Some(std::path::Path::new("custom/output.json"))
    );
}

#[test]
fn parse_benchmark_cli_args_rejects_unknown_flags() {
    assert!(parse_benchmark_cli_args(&["--nope".to_owned()]).is_err());
}

#[test]
fn parse_benchmark_cli_args_rejects_multiple_output_paths() {
    assert!(parse_benchmark_cli_args(&["a.json".to_owned(), "b.json".to_owned()]).is_err());
}

// ── Scores esperados desde la relajación LP (MR-211) ─────────────────────────

#[test]
fn expected_period_scores_weight_fractions() {
    let assignments = vec![
        MarvinScheduleAssignment {
            linear_index: 7,
            destination_index: 0,
            period_index: 0,
            fraction: 0.25,
        },
        MarvinScheduleAssignment {
            linear_index: 7,
            destination_index: 0,
            period_index: 2,
            fraction: 0.75,
        },
    ];
    let scores = build_expected_period_scores(&assignments);
    assert_eq!(scores.len(), 1);
    assert!((scores[&7] - 1.5).abs() < 1e-12);
}

#[test]
fn expected_period_scores_skip_zero_support() {
    let assignments = vec![MarvinScheduleAssignment {
        linear_index: 3,
        destination_index: 0,
        period_index: 1,
        fraction: 0.0,
    }];
    assert!(build_expected_period_scores(&assignments).is_empty());
}

// ── Adaptación del contrato MineLib CPIT (MR-211) ────────────────────────────

fn sample_cpit_problem() -> MarvinScheduleProblem {
    MarvinScheduleProblem {
        kind: MarvinScheduleProblemKind::Cpit,
        name: "fixture".to_owned(),
        block_count: 2,
        period_count: 2,
        destination_count: 1,
        resource_constraint_count: 1,
        general_constraint_count: 0,
        discount_rate: 0.1,
        resource_constraint_limits: vec![
            MarvinResourceConstraintLimit {
                resource_index: 0,
                period_index: 0,
                relation: 'L',
                limit: 10.0,
            },
            MarvinResourceConstraintLimit {
                resource_index: 0,
                period_index: 1,
                relation: 'G',
                limit: 1.0,
            },
        ],
        objective_terms: vec![
            MarvinObjectiveTerm {
                linear_index: 0,
                destination_index: 0,
                objective_value: 5.0,
            },
            MarvinObjectiveTerm {
                linear_index: 1,
                destination_index: 0,
                objective_value: -2.0,
            },
        ],
        resource_coefficients: vec![],
    }
}

#[test]
fn toposort_problem_maps_upper_limits_and_reports_unenforced_relations() {
    let problem = sample_cpit_problem();
    let (toposort_problem, unenforced) =
        build_toposort_problem_from_minelib_cpit(&problem).expect("conversion should work");

    assert_eq!(toposort_problem.period_count, 2);
    assert_eq!(toposort_problem.resource_count, 1);
    assert_eq!(
        toposort_problem.period_resource_upper_limits[0][0],
        Some(10.0)
    );
    // La relación `G` del periodo 1 no se refuerza y queda auditada.
    assert_eq!(toposort_problem.period_resource_upper_limits[1][0], None);
    assert_eq!(unenforced.len(), 1);
    assert!(unenforced[0].contains("relation `G`"));
}

#[test]
fn toposort_problem_rejects_multi_destination_problems() {
    let mut problem = sample_cpit_problem();
    problem.destination_count = 2;
    assert!(build_toposort_problem_from_minelib_cpit(&problem).is_err());
}

// ── Verificación de precedencias del schedule (MR-211) ───────────────────────

fn schedule_with(assignments: Vec<CpitToposortAssignment>) -> CpitToposortSchedule {
    CpitToposortSchedule {
        scheduled_block_count: assignments.len(),
        assignments,
        dropped_for_capacity_count: 0,
        dropped_for_predecessor_count: 0,
        delayed_negative_block_count: 0,
        undiscounted_objective: 0.0,
        discounted_objective: 0.0,
        period_resource_usage: vec![],
        used_period_count: 0,
    }
}

fn block_edge(pred: usize, succ: usize) -> PrecedenceEdge {
    PrecedenceEdge::new(PrecedenceNode::Block(pred), PrecedenceNode::Block(succ))
}

#[test]
fn verify_schedule_precedence_accepts_feasible_schedules() {
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1)]).expect("graph should be valid");
    let schedule = schedule_with(vec![
        CpitToposortAssignment {
            linear_index: 0,
            period_index: 0,
        },
        CpitToposortAssignment {
            linear_index: 1,
            period_index: 1,
        },
    ]);
    assert_eq!(
        verify_schedule_precedence(&schedule, &graph).expect("schedule should verify"),
        1
    );
}

#[test]
fn verify_schedule_precedence_rejects_temporal_violations() {
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1)]).expect("graph should be valid");
    let schedule = schedule_with(vec![
        CpitToposortAssignment {
            linear_index: 0,
            period_index: 2,
        },
        CpitToposortAssignment {
            linear_index: 1,
            period_index: 1,
        },
    ]);
    assert!(verify_schedule_precedence(&schedule, &graph).is_err());
}

#[test]
fn verify_schedule_precedence_rejects_closure_violations() {
    let graph = PrecedenceGraph::new(vec![block_edge(0, 1)]).expect("graph should be valid");
    let schedule = schedule_with(vec![CpitToposortAssignment {
        linear_index: 1,
        period_index: 0,
    }]);
    assert!(verify_schedule_precedence(&schedule, &graph).is_err());
}

// ── Wiring end-to-end mínimo del solver core desde el harness ────────────────

#[test]
fn core_solver_consumes_adapted_problem_end_to_end() {
    let problem = sample_cpit_problem();
    let (toposort_problem, _) =
        build_toposort_problem_from_minelib_cpit(&problem).expect("conversion should work");
    let graph = PrecedenceGraph::new(vec![block_edge(1, 0)]).expect("graph should be valid");
    let scores = BTreeMap::from([(0usize, 1.0), (1usize, 0.5)]);

    let schedule = solve_cpit_with_toposort(
        &toposort_problem,
        &graph,
        &scores,
        &CpitToposortOptions::default(),
    )
    .expect("solver should succeed");

    assert_eq!(schedule.scheduled_block_count, 2);
    verify_schedule_precedence(&schedule, &graph).expect("schedule should be feasible");
}
