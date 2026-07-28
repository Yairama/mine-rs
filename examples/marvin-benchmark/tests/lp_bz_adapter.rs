#![allow(missing_docs)]

#[path = "../src/lp_bz_adapter.rs"]
mod lp_bz_adapter;
#[path = "../src/lp_bz_bound.rs"]
mod lp_bz_bound;
#[path = "../src/lp_bz_lp_kernel.rs"]
mod lp_bz_lp_kernel;
#[path = "../src/lp_bz_rounder.rs"]
mod lp_bz_rounder;
#[path = "../src/lp_bz_runtime_budget.rs"]
mod lp_bz_runtime_budget;
#[path = "../src/marvin_support.rs"]
mod marvin_support;

use std::collections::BTreeSet;
use std::path::Path;

use lp_bz_adapter::{MARVIN_FOCUSED_LP_BZ_ADAPTER_SCOPE, run_marvin_focused_lp_bz_adapter};
use marvin_support::{
    MarvinObjectiveTerm, MarvinScheduleAssignment, MarvinScheduleProblem,
    MarvinScheduleProblemKind, MarvinScheduleSolution,
};
use mine_sdk::{
    Metadata, ModelId, NestingAccessRules, PhaseDesign, PushbackPlan, ScenarioId,
    ScheduleDestinationId, SchedulingObjectiveTerm, SchedulingPeriod, SchedulingProblem,
    SchedulingUnit, SchedulingUnitId,
};

#[test]
fn marvin_adapter_returns_compact_summary_and_real_focused_optimizer_limitation() {
    let phase_plan = sample_phase_plan();
    let scheduling_problem = sample_scheduling_problem();
    let marvin_problem = sample_marvin_problem();
    let lp_solution = sample_lp_solution();

    let result = run_marvin_focused_lp_bz_adapter(
        &phase_plan,
        &scheduling_problem,
        &marvin_problem,
        &lp_solution,
        Path::new("repo/datasets/marvin.LPpcpsp"),
        Path::new("repo"),
        "marvin-test-local-front-phase",
        None,
        Metadata::new(),
    )
    .expect("adapter should succeed");

    assert_eq!(
        result.summary.scope_label,
        MARVIN_FOCUSED_LP_BZ_ADAPTER_SCOPE
    );
    assert_eq!(
        result.summary.seeded_schedule_entry_count,
        result.seeded_schedule.entries().len()
    );
    assert!(result.summary.lp_bz_round_repair.focused_round_repair);
    assert!(!result.summary.lp_bz_round_repair.local_optimization_skipped);
    assert!(
        result
            .summary
            .lp_bz_round_repair
            .target_score_decomposition
            .rounded_discounted_target_score_proxy
            >= result
                .summary
                .lp_bz_round_repair
                .target_score_decomposition
                .repaired_discounted_target_score_proxy
    );
    assert!(
        result
            .summary
            .lp_bz_round_repair
            .target_score_decomposition
            .local_search_discounted_target_score_proxy
            >= result
                .summary
                .lp_bz_round_repair
                .target_score_decomposition
                .repaired_discounted_target_score_proxy
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .local_optimizer_strategy_label,
        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
    );
    assert!(
        result
            .summary
            .lp_bz_round_repair
            .local_optimizer_executed_iteration_count
            > 0
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .target_score_decomposition
            .local_search_score_delta_vs_repair_proxy,
        result
            .summary
            .lp_bz_round_repair
            .target_score_decomposition
            .local_search_discounted_target_score_proxy
            - result
                .summary
                .lp_bz_round_repair
                .target_score_decomposition
                .repaired_discounted_target_score_proxy
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .target_score_decomposition
            .local_search_score_delta_vs_round_proxy,
        result
            .summary
            .lp_bz_round_repair
            .target_score_decomposition
            .local_search_discounted_target_score_proxy
            - result
                .summary
                .lp_bz_round_repair
                .target_score_decomposition
                .rounded_discounted_target_score_proxy
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_budget_profile
            .mode_label,
        "full-round-repair"
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_runtime_budget_contract
            .strategy_label,
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_strategy_label
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_runtime_budget_contract
            .executed_iteration_count,
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_executed_iteration_count
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_runtime_budget_contract
            .termination_reason,
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_termination_reason
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_runtime_budget_contract
            .max_iteration_count,
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_budget_profile
            .effective_iteration_budget
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_search_discounted_target_score_proxy,
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .local_search_score_delta_vs_focused_proxy
            + result
                .summary
                .lp_bz_round_repair
                .target_score_decomposition
                .local_search_discounted_target_score_proxy
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .improvement_status,
        if result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .local_search_score_delta_vs_focused_proxy
            > 1.0e-9
        {
            "full-round-repair-probe-improves-focused-proxy"
        } else if result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .local_search_score_delta_vs_focused_proxy
            < -1.0e-9
        {
            "focused-candidate-beats-full-round-repair-probe"
        } else if result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .target_period_change_count_vs_focused
            > 0
        {
            "full-round-repair-probe-reorders-without-proxy-gain"
        } else if result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_residual_opportunity
            .improving_move_available
        {
            "full-round-repair-probe-still-has-residual-headroom"
        } else {
            "focused-candidate-matches-full-round-repair-probe"
        }
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .local_optimizer_budget_profile
            .mode_label,
        "focused-refresh-budgeted"
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .local_optimizer_budget_profile
            .effective_iteration_budget,
        result
            .summary
            .lp_bz_round_repair
            .local_optimizer_runtime_budget_contract
            .max_iteration_count
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .local_optimizer_runtime_budget_contract
            .execution_state,
        "completed-within-budget"
    );
    assert!(
        !result
            .summary
            .lp_bz_round_repair
            .local_optimizer_residual_opportunity
            .improving_move_available
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .local_optimizer_residual_opportunity
            .move_kind_label,
        "none"
    );
    assert_eq!(
        result
            .summary
            .lp_bz_round_repair
            .competitive_probe
            .competitive_local_optimizer_strategy_label,
        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
    );
    assert_eq!(
        result.summary.lp_bz_lp_kernel.variable_count,
        scheduling_problem.periods().len() * scheduling_problem.units().len()
    );
    assert_eq!(
        result.summary.lp_bz_bound.unit_count,
        scheduling_problem.units().len()
    );
    assert_eq!(
        result.summary.representative_period_block_count,
        lp_solution.unique_block_count
    );
    assert!(
        result
            .summary
            .limitations
            .iter()
            .any(|limitation| limitation.contains("Marvin-scoped"))
    );
    assert!(
        result
            .summary
            .limitations
            .iter()
            .any(|limitation| { limitation.contains("optimized benchmark-side LP/BZ candidate") })
    );
}

fn sample_phase_plan() -> PushbackPlan {
    let phase_a = PhaseDesign {
        phase_id: "phase-a".to_owned(),
        pushback_index: 0,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(100),
        block_indices: vec![10, 20],
        block_count: 2,
        total_tonnage: Some(2.0),
        predecessor_phase_ids: Vec::new(),
    };
    let phase_b = PhaseDesign {
        phase_id: "phase-b".to_owned(),
        pushback_index: 1,
        shell_index: Some(1),
        revenue_factor: Some(1.0),
        bench: Some(99),
        block_indices: vec![30],
        block_count: 1,
        total_tonnage: Some(1.0),
        predecessor_phase_ids: vec!["phase-a".to_owned()],
    };

    PushbackPlan {
        phases: vec![phase_a, phase_b],
        phase_count: 2,
        total_block_count: 3,
        total_tonnage: Some(3.0),
        nesting_rules: NestingAccessRules::default_open(),
        limitations: vec!["test fixture".to_owned()],
    }
}

fn sample_scheduling_problem() -> SchedulingProblem {
    let period_01 = SchedulingPeriod::new("P01", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_02 = SchedulingPeriod::new("P02", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_03 = SchedulingPeriod::new("P03", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let destination_id =
        ScheduleDestinationId::new("dest-00").expect("destination id should be valid");
    let unit_a_part_01 =
        SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let unit_a_part_02 =
        SchedulingUnitId::new("phase-a::part-02").expect("unit id should be valid");
    let unit_b_part_01 =
        SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            unit_a_part_01.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(100),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            unit_a_part_02.clone(),
            1.0,
            1,
            vec![unit_a_part_01.clone()],
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(100),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            unit_b_part_01.clone(),
            1.0,
            1,
            vec![unit_a_part_02.clone()],
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
    ];
    let objective_terms = vec![
        SchedulingObjectiveTerm::new(unit_a_part_01, Some(destination_id.clone()), 120.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_a_part_02, Some(destination_id.clone()), 8.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_b_part_01, Some(destination_id.clone()), 4.0)
            .expect("objective should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("marvin-adapter-problem").expect("scenario id should be valid"),
        ModelId::new("marvin-adapter-model").expect("model id should be valid"),
        vec![period_01, period_02, period_03],
        units,
        objective_terms,
        Vec::new(),
        vec![destination_id],
        Vec::new(),
        0.1,
        Metadata::new(),
        vec!["test fixture".to_owned()],
    )
    .expect("scheduling problem should be valid")
}

fn sample_marvin_problem() -> MarvinScheduleProblem {
    MarvinScheduleProblem {
        kind: MarvinScheduleProblemKind::Pcpsp,
        name: "marvin-adapter-test".to_owned(),
        block_count: 3,
        period_count: 3,
        destination_count: 1,
        resource_constraint_count: 0,
        general_constraint_count: 0,
        discount_rate: 0.1,
        resource_constraint_limits: Vec::new(),
        objective_terms: vec![
            MarvinObjectiveTerm {
                linear_index: 10,
                destination_index: 0,
                objective_value: 120.0,
            },
            MarvinObjectiveTerm {
                linear_index: 20,
                destination_index: 0,
                objective_value: 8.0,
            },
            MarvinObjectiveTerm {
                linear_index: 30,
                destination_index: 0,
                objective_value: 4.0,
            },
        ],
        resource_coefficients: Vec::new(),
    }
}

fn sample_lp_solution() -> MarvinScheduleSolution {
    let assignments = vec![
        MarvinScheduleAssignment {
            linear_index: 10,
            destination_index: 0,
            period_index: 1,
            fraction: 0.6,
        },
        MarvinScheduleAssignment {
            linear_index: 10,
            destination_index: 0,
            period_index: 2,
            fraction: 0.4,
        },
        MarvinScheduleAssignment {
            linear_index: 20,
            destination_index: 0,
            period_index: 1,
            fraction: 0.8,
        },
        MarvinScheduleAssignment {
            linear_index: 20,
            destination_index: 0,
            period_index: 2,
            fraction: 0.2,
        },
        MarvinScheduleAssignment {
            linear_index: 30,
            destination_index: 0,
            period_index: 2,
            fraction: 1.0,
        },
    ];
    let unique_block_count = assignments
        .iter()
        .map(|assignment| assignment.linear_index)
        .collect::<BTreeSet<_>>()
        .len();

    MarvinScheduleSolution {
        kind: MarvinScheduleProblemKind::Pcpsp,
        assignments,
        unique_block_count,
    }
}
