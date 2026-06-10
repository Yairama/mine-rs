#[path = "../src/lp_bz_rounder.rs"]
mod lp_bz_rounder;
#[path = "../src/marvin_support.rs"]
mod marvin_support;

use std::collections::{BTreeMap, BTreeSet};

use lp_bz_rounder::{
    assert_precedence_feasible_unit_targets, build_lp_guided_round_repair_targets,
    build_target_period_seeded_schedule_from_lp_round_repair_v3,
    build_target_period_seeded_schedule_from_lp_round_repair_v6_focused,
    round_and_repair_phase_target_periods, round_and_repair_unit_target_periods,
};
use marvin_support::{MarvinScheduleAssignment, MarvinScheduleProblemKind, MarvinScheduleSolution};
use mine_sdk::{
    Metadata, ModelId, NestingAccessRules, PhaseDesign, PushbackPlan, ScenarioId,
    ScheduleDestinationId, SchedulingObjectiveTerm, SchedulingPeriod, SchedulingProblem,
    SchedulingUnit, SchedulingUnitId,
};

#[test]
fn lp_guided_round_repair_is_deterministic_and_precedence_feasible() {
    let phase_plan = sample_phase_plan(true);
    let scheduling_problem = sample_scheduling_problem(true);
    let lp_solution = sample_lp_solution(&[(10, 2.4), (20, 0.2)]);

    let artifacts_first =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("round/repair should succeed");
    let artifacts_second =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("round/repair should be deterministic");

    assert_eq!(artifacts_first, artifacts_second);
    assert_eq!(artifacts_first.repaired_phase_target_count, 1);
    assert_eq!(
        artifacts_first.phase_target_period_by_phase.get("phase-a"),
        Some(&2usize)
    );
    assert_eq!(
        artifacts_first.phase_target_period_by_phase.get("phase-b"),
        Some(&2usize)
    );
    assert_eq!(
        artifacts_first.unit_round_repair.repaired_unit_target_count,
        0
    );
    assert_eq!(artifacts_first.unit_round_repair.horizon_clamp_count, 0);
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .local_improvement_move_count,
        0
    );
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .target_score_decomposition
            .local_search_score_delta_vs_repair_proxy,
        0.0
    );
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .target_score_decomposition
            .local_search_score_delta_vs_round_proxy,
        artifacts_first
            .unit_round_repair
            .target_score_decomposition
            .local_search_discounted_target_score_proxy
            - artifacts_first
                .unit_round_repair
                .target_score_decomposition
                .rounded_discounted_target_score_proxy
    );
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .local_optimizer_diagnostics
            .strategy_label,
        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
    );
    assert!(
        !artifacts_first
            .unit_round_repair
            .local_optimizer_diagnostics
            .residual_opportunity
            .improving_move_available
    );
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .local_optimizer_diagnostics
            .residual_opportunity
            .move_kind_label,
        "none"
    );
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid")),
        Some(&1usize)
    );
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-a::part-02").expect("unit id should be valid")),
        Some(&2usize)
    );
    assert_eq!(
        artifacts_first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid")),
        Some(&2usize)
    );
    assert_precedence_feasible_unit_targets(
        &scheduling_problem,
        &artifacts_first.unit_round_repair.target_period_by_unit,
    )
    .expect("targets should remain precedence-feasible");
}

#[test]
fn focused_round_repair_executes_local_optimizer_and_preserves_feasible_seeded_schedule() {
    let phase_plan = sample_phase_plan(true);
    let scheduling_problem = sample_scheduling_problem(true);
    let lp_solution = sample_lp_solution(&[(10, 2.4), (20, 0.2)]);

    let (artifacts, schedule) =
        build_target_period_seeded_schedule_from_lp_round_repair_v6_focused(
            &phase_plan,
            &scheduling_problem,
            &lp_solution,
            None,
            Metadata::new(),
        )
        .expect("focused round/repair should succeed");

    assert_eq!(
        artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .strategy_label,
        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
    );
    assert!(
        artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .executed_iteration_count
            > 0
    );
    assert!(
        !artifacts
            .unit_round_repair
            .local_optimizer_diagnostics
            .residual_opportunity
            .improving_move_available
    );
    assert!(
        artifacts
            .unit_round_repair
            .target_score_decomposition
            .rounded_discounted_target_score_proxy
            >= artifacts
                .unit_round_repair
                .target_score_decomposition
                .repaired_discounted_target_score_proxy
    );
    assert!(
        artifacts
            .unit_round_repair
            .target_score_decomposition
            .local_search_discounted_target_score_proxy
            >= artifacts
                .unit_round_repair
                .target_score_decomposition
                .repaired_discounted_target_score_proxy
    );
    assert_eq!(
        artifacts
            .unit_round_repair
            .target_score_decomposition
            .local_search_score_delta_vs_round_proxy,
        artifacts
            .unit_round_repair
            .target_score_decomposition
            .local_search_discounted_target_score_proxy
            - artifacts
                .unit_round_repair
                .target_score_decomposition
                .rounded_discounted_target_score_proxy
    );
    assert_precedence_feasible_unit_targets(
        &scheduling_problem,
        &artifacts.unit_round_repair.target_period_by_unit,
    )
    .expect("focused targets should remain precedence-feasible");
    assert!(
        !schedule.entries().is_empty(),
        "focused schedule should still materialize seeded assignments"
    );
}

#[test]
fn lp_guided_round_repair_v3_improves_discounted_target_score_vs_phase_flattened_baseline() {
    let phase_plan = sample_phase_plan(true);
    let scheduling_problem = sample_scheduling_problem_with_objectives(true, (120.0, 8.0, 4.0));
    let lp_solution = sample_lp_solution(&[(10, 1.6), (20, 1.6)]);

    let artifacts =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("round/repair should succeed");
    let legacy_fractional_target_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| {
            let phase_id = phase_id_for_test_unit(unit.unit_id());
            let phase_target = artifacts
                .phase_target_period_by_phase
                .get(phase_id)
                .copied()
                .expect("phase target should exist");
            (unit.unit_id().clone(), phase_target as f64)
        })
        .collect::<BTreeMap<_, _>>();
    let legacy_round_repair = round_and_repair_unit_target_periods(
        &scheduling_problem,
        &legacy_fractional_target_by_unit,
    )
    .expect("legacy-style phase-flattened targets should repair");

    let improved_score = discounted_target_score(
        &scheduling_problem,
        &artifacts.unit_round_repair.target_period_by_unit,
    );
    let baseline_score = discounted_target_score(
        &scheduling_problem,
        &legacy_round_repair.target_period_by_unit,
    );

    assert!(
        improved_score >= baseline_score - 1.0e-9,
        "improved score {improved_score} should be >= legacy baseline {baseline_score}"
    );
    assert_eq!(
        artifacts
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid")),
        Some(&1usize)
    );
}

#[test]
fn lp_guided_round_repair_v5_improves_crafted_bimodal_fractional_fixture() {
    let phase_plan = sample_phase_plan(true);
    let scheduling_problem = sample_scheduling_problem_with_objectives(true, (120.0, 8.0, 4.0));
    let lp_solution =
        sample_lp_solution_from_assignments(&[(10, 0, 0, 0.5), (10, 0, 2, 0.5), (20, 0, 2, 1.0)]);

    let first =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("round/repair should succeed on crafted fixture");
    let second =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("crafted fixture round/repair should be deterministic");
    assert_eq!(first, second);
    assert_precedence_feasible_unit_targets(
        &scheduling_problem,
        &first.unit_round_repair.target_period_by_unit,
    )
    .expect("crafted fixture targets should remain precedence-feasible");

    let legacy_fractional_target_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| {
            let phase_id = phase_id_for_test_unit(unit.unit_id());
            let phase_target = first
                .phase_target_period_by_phase
                .get(phase_id)
                .copied()
                .expect("phase target should exist");
            (unit.unit_id().clone(), phase_target as f64)
        })
        .collect::<BTreeMap<_, _>>();
    let legacy_round_repair = round_and_repair_unit_target_periods(
        &scheduling_problem,
        &legacy_fractional_target_by_unit,
    )
    .expect("legacy-style phase-flattened targets should repair");

    let improved_score = discounted_target_score(
        &scheduling_problem,
        &first.unit_round_repair.target_period_by_unit,
    );
    let baseline_score = discounted_target_score(
        &scheduling_problem,
        &legacy_round_repair.target_period_by_unit,
    );
    assert!(
        improved_score > baseline_score + 1.0e-9,
        "crafted fixture improved score {improved_score} should be > baseline {baseline_score}"
    );
    assert_eq!(
        first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid")),
        Some(&0usize)
    );
}

#[test]
fn lp_guided_round_repair_v3_seeded_schedule_is_deterministic() {
    let phase_plan = sample_phase_plan(true);
    let scheduling_problem = sample_scheduling_problem_with_objectives(true, (120.0, 8.0, 4.0));
    let lp_solution = sample_lp_solution(&[(10, 1.6), (20, 1.6)]);

    let first = build_target_period_seeded_schedule_from_lp_round_repair_v3(
        &phase_plan,
        &scheduling_problem,
        &lp_solution,
        None,
        Metadata::new(),
    )
    .expect("v3 schedule build should succeed");
    let second = build_target_period_seeded_schedule_from_lp_round_repair_v3(
        &phase_plan,
        &scheduling_problem,
        &lp_solution,
        None,
        Metadata::new(),
    )
    .expect("v3 schedule build should be deterministic");

    assert_eq!(first, second);
}

#[test]
fn lp_guided_round_repair_reassigns_pull_forward_to_precedence_feasible_units() {
    let phase_plan = sample_phase_plan_with_precedence_slack();
    let scheduling_problem = sample_scheduling_problem_with_precedence_slack();
    let lp_solution = sample_lp_solution(&[(10, 1.6), (20, 1.6), (30, 1.1)]);

    let first =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("round/repair should succeed");
    let second =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("round/repair should be deterministic");

    assert_eq!(first, second);
    assert_eq!(first.unit_round_repair.repaired_unit_target_count, 0);
    assert_eq!(first.unit_round_repair.local_improvement_move_count, 1);
    assert_eq!(
        first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid")),
        Some(&0usize)
    );
    assert_eq!(
        first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid")),
        Some(&0usize)
    );
    assert_eq!(
        first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid")),
        Some(&0usize)
    );
    assert_eq!(
        first
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-b::part-02").expect("unit id should be valid")),
        Some(&1usize)
    );
    assert_precedence_feasible_unit_targets(
        &scheduling_problem,
        &first.unit_round_repair.target_period_by_unit,
    )
    .expect("reassigned targets should remain precedence-feasible");

    let baseline_target_by_unit = BTreeMap::from([
        (
            SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid"),
            2usize,
        ),
        (
            SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid"),
            0usize,
        ),
        (
            SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid"),
            2usize,
        ),
        (
            SchedulingUnitId::new("phase-b::part-02").expect("unit id should be valid"),
            1usize,
        ),
    ]);
    let improved_score = discounted_target_score(
        &scheduling_problem,
        &first.unit_round_repair.target_period_by_unit,
    );
    let baseline_score = discounted_target_score(&scheduling_problem, &baseline_target_by_unit);
    assert!(
        improved_score > baseline_score + 1.0e-9,
        "precedence slack score {improved_score} should improve baseline {baseline_score}"
    );
}

#[test]
fn lp_guided_round_repair_prefers_mass_weighted_phase_signal_when_block_average_hits_integer() {
    let phase_plan = sample_phase_plan_with_partial_lp_mass();
    let scheduling_problem = sample_scheduling_problem_with_partial_lp_mass();
    let lp_solution =
        sample_lp_solution_from_assignments(&[(10, 0, 0, 0.1), (11, 0, 2, 1.0), (20, 0, 2, 1.0)]);

    let artifacts =
        build_lp_guided_round_repair_targets(&phase_plan, &scheduling_problem, &lp_solution)
            .expect("round/repair should respect weighted fractional mass");

    assert_eq!(
        artifacts.phase_target_period_by_phase.get("phase-a"),
        Some(&2usize),
        "phase-a should follow the weighted LP signal instead of the stale integer block average"
    );
    assert_eq!(
        artifacts
            .unit_round_repair
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid")),
        Some(&2usize)
    );
    assert_precedence_feasible_unit_targets(
        &scheduling_problem,
        &artifacts.unit_round_repair.target_period_by_unit,
    )
    .expect("weighted-signal targets should remain precedence-feasible");
}

#[test]
fn unit_round_repair_clamps_to_horizon() {
    let scheduling_problem = sample_scheduling_problem(true);
    let target_period_by_unit = BTreeMap::from([
        (
            SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid"),
            9.2,
        ),
        (
            SchedulingUnitId::new("phase-a::part-02").expect("unit id should be valid"),
            9.8,
        ),
        (
            SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid"),
            9.1,
        ),
    ]);

    let repaired =
        round_and_repair_unit_target_periods(&scheduling_problem, &target_period_by_unit)
            .expect("round/repair should clamp to period horizon");
    assert_eq!(repaired.horizon_clamp_count, 3);
    assert_eq!(repaired.local_improvement_move_count, 0);
    assert!(
        repaired
            .target_period_by_unit
            .values()
            .all(|period| *period == 2)
    );
}

#[test]
fn unit_round_repair_repairs_non_topological_unit_input() {
    let scheduling_problem = sample_scheduling_problem(false);
    let target_period_by_unit = BTreeMap::from([
        (
            SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid"),
            2.4,
        ),
        (
            SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid"),
            0.2,
        ),
    ]);

    let repaired_first =
        round_and_repair_unit_target_periods(&scheduling_problem, &target_period_by_unit)
            .expect("non-topological input should be repaired by deterministic topo ordering");
    let repaired_second =
        round_and_repair_unit_target_periods(&scheduling_problem, &target_period_by_unit)
            .expect("round/repair should be deterministic");
    assert_eq!(repaired_first, repaired_second);
    assert_eq!(repaired_first.repaired_unit_target_count, 1);
    assert_eq!(repaired_first.local_improvement_move_count, 0);
    assert_eq!(
        repaired_first
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid")),
        Some(&2usize)
    );
    assert_eq!(
        repaired_first
            .target_period_by_unit
            .get(&SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid")),
        Some(&2usize)
    );
}

#[test]
fn unit_round_repair_local_optimizer_is_deterministic_and_improves_discounted_score() {
    let scheduling_problem = sample_scheduling_problem_for_local_swap_optimizer();
    let low_unit = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let high_unit = SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid");
    let downstream_unit =
        SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");
    let fractional_target_by_unit = BTreeMap::from([
        (low_unit.clone(), 0.4),
        (high_unit.clone(), 0.6),
        (downstream_unit.clone(), 2.0),
    ]);

    let first =
        round_and_repair_unit_target_periods(&scheduling_problem, &fractional_target_by_unit)
            .expect("local optimizer fixture should be repaired");
    let second =
        round_and_repair_unit_target_periods(&scheduling_problem, &fractional_target_by_unit)
            .expect("local optimizer fixture should be deterministic");
    assert_eq!(first, second);
    assert_eq!(first.local_improvement_move_count, 1);
    assert_eq!(first.target_period_by_unit.get(&high_unit), Some(&0usize));
    assert_eq!(first.target_period_by_unit.get(&low_unit), Some(&1usize));
    assert_eq!(
        first.target_period_by_unit.get(&downstream_unit),
        Some(&2usize)
    );
    assert_precedence_feasible_unit_targets(&scheduling_problem, &first.target_period_by_unit)
        .expect("optimized targets should remain precedence-feasible");

    let baseline_target_by_unit = BTreeMap::from([
        (low_unit, 0usize),
        (high_unit, 1usize),
        (downstream_unit, 2usize),
    ]);
    let improved_score = discounted_target_score(&scheduling_problem, &first.target_period_by_unit);
    let baseline_score = discounted_target_score(&scheduling_problem, &baseline_target_by_unit);
    assert!(
        improved_score > baseline_score + 1.0e-9,
        "local optimizer score {improved_score} should improve baseline {baseline_score}"
    );
}

#[test]
fn unit_round_repair_precedence_chain_optimizer_improves_when_swaps_stagnate() {
    let scheduling_problem = sample_scheduling_problem_for_precedence_chain_optimizer();
    let blocker_unit = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let predecessor_unit =
        SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");
    let anchor_unit = SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid");
    let fractional_target_by_unit = BTreeMap::from([
        (blocker_unit.clone(), 0.1),
        (predecessor_unit.clone(), 1.0),
        (anchor_unit.clone(), 2.0),
    ]);

    let first =
        round_and_repair_unit_target_periods(&scheduling_problem, &fractional_target_by_unit)
            .expect("precedence chain fixture should be repaired");
    let second =
        round_and_repair_unit_target_periods(&scheduling_problem, &fractional_target_by_unit)
            .expect("precedence chain fixture should be deterministic");
    assert_eq!(first, second);
    assert_eq!(first.local_improvement_move_count, 1);
    assert_eq!(
        first.target_period_by_unit.get(&blocker_unit),
        Some(&0usize)
    );
    assert_eq!(
        first.target_period_by_unit.get(&predecessor_unit),
        Some(&0usize)
    );
    assert_eq!(first.target_period_by_unit.get(&anchor_unit), Some(&0usize));
    assert_precedence_feasible_unit_targets(&scheduling_problem, &first.target_period_by_unit)
        .expect("precedence chain targets should remain precedence-feasible");

    let baseline_target_by_unit = BTreeMap::from([
        (blocker_unit, 0usize),
        (predecessor_unit, 1usize),
        (anchor_unit, 2usize),
    ]);
    let improved_score = discounted_target_score(&scheduling_problem, &first.target_period_by_unit);
    let baseline_score = discounted_target_score(&scheduling_problem, &baseline_target_by_unit);
    assert!(
        improved_score > baseline_score + 1.0e-9,
        "precedence chain score {improved_score} should improve stagnated baseline {baseline_score}"
    );
}

#[test]
fn unit_round_repair_period_ejection_optimizer_improves_when_adjacent_moves_stagnate() {
    let scheduling_problem = sample_scheduling_problem_for_period_ejection_optimizer();
    let stable_unit = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let mover_unit = SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");
    let lock_unit = SchedulingUnitId::new("phase-b::part-02").expect("unit id should be valid");
    let blocker_unit = SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid");
    let anchor_unit = SchedulingUnitId::new("phase-d::part-01").expect("unit id should be valid");
    let fractional_target_by_unit = BTreeMap::from([
        (stable_unit.clone(), 0.0),
        (mover_unit.clone(), 1.0),
        (lock_unit.clone(), 2.0),
        (blocker_unit.clone(), 2.0),
        (anchor_unit.clone(), 3.0),
    ]);

    let first =
        round_and_repair_unit_target_periods(&scheduling_problem, &fractional_target_by_unit)
            .expect("period ejection fixture should be repaired");
    let second =
        round_and_repair_unit_target_periods(&scheduling_problem, &fractional_target_by_unit)
            .expect("period ejection fixture should be deterministic");
    assert_eq!(first, second);
    assert!(
        first.local_improvement_move_count >= 1,
        "period ejection fixture should apply at least one improving local move"
    );
    assert_eq!(
        first.local_optimizer_diagnostics.strategy_label,
        "deterministic-adjacent-swap-plus-period-ejection-plus-precedence-chain-v8"
    );
    assert_eq!(first.target_period_by_unit.get(&stable_unit), Some(&0usize));
    assert_eq!(first.target_period_by_unit.get(&anchor_unit), Some(&1usize));
    assert_eq!(first.target_period_by_unit.get(&mover_unit), Some(&3usize));
    assert_precedence_feasible_unit_targets(&scheduling_problem, &first.target_period_by_unit)
        .expect("period ejection targets should remain precedence-feasible");

    let baseline_target_by_unit = BTreeMap::from([
        (stable_unit, 0usize),
        (mover_unit, 1usize),
        (lock_unit, 2usize),
        (blocker_unit, 2usize),
        (anchor_unit, 3usize),
    ]);
    let improved_score = discounted_target_score(&scheduling_problem, &first.target_period_by_unit);
    let baseline_score = discounted_target_score(&scheduling_problem, &baseline_target_by_unit);
    assert!(
        improved_score > baseline_score + 1.0e-9,
        "period ejection score {improved_score} should improve adjacent-stagnated baseline {baseline_score}"
    );
}

#[test]
fn phase_round_repair_repairs_non_topological_phase_input() {
    let phase_plan = sample_phase_plan(false);
    let representative_period_by_block = BTreeMap::from([(10usize, 2.4), (20usize, 0.2)]);

    let repaired_first =
        round_and_repair_phase_target_periods(&phase_plan, &representative_period_by_block)
            .expect("phase repair should work even if input phases are not topological");
    let repaired_second =
        round_and_repair_phase_target_periods(&phase_plan, &representative_period_by_block)
            .expect("phase repair should be deterministic");
    assert_eq!(repaired_first, repaired_second);
    assert_eq!(repaired_first.1, 1);
    assert_eq!(repaired_first.0.get("phase-a"), Some(&2usize));
    assert_eq!(repaired_first.0.get("phase-b"), Some(&2usize));
}

fn sample_phase_plan(topological_order: bool) -> PushbackPlan {
    let phase_a = PhaseDesign {
        phase_id: "phase-a".to_owned(),
        pushback_index: 0,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(100),
        block_count: 1,
        total_tonnage: Some(1.0),
        block_indices: vec![10],
        predecessor_phase_ids: Vec::new(),
    };
    let phase_b = PhaseDesign {
        phase_id: "phase-b".to_owned(),
        pushback_index: 1,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(99),
        block_count: 1,
        total_tonnage: Some(1.0),
        block_indices: vec![20],
        predecessor_phase_ids: vec!["phase-a".to_owned()],
    };
    PushbackPlan {
        phase_count: 2,
        total_block_count: 2,
        total_tonnage: Some(2.0),
        phases: if topological_order {
            vec![phase_a, phase_b]
        } else {
            vec![phase_b, phase_a]
        },
        nesting_rules: NestingAccessRules::default_open(),
        limitations: vec!["test fixture".to_owned()],
    }
}

fn sample_phase_plan_with_partial_lp_mass() -> PushbackPlan {
    let phase_a = PhaseDesign {
        phase_id: "phase-a".to_owned(),
        pushback_index: 0,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(100),
        block_count: 2,
        total_tonnage: Some(2.0),
        block_indices: vec![10, 11],
        predecessor_phase_ids: Vec::new(),
    };
    let phase_b = PhaseDesign {
        phase_id: "phase-b".to_owned(),
        pushback_index: 1,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(99),
        block_count: 1,
        total_tonnage: Some(1.0),
        block_indices: vec![20],
        predecessor_phase_ids: vec!["phase-a".to_owned()],
    };
    PushbackPlan {
        phase_count: 2,
        total_block_count: 3,
        total_tonnage: Some(3.0),
        phases: vec![phase_a, phase_b],
        nesting_rules: NestingAccessRules::default_open(),
        limitations: vec!["test fixture".to_owned()],
    }
}

fn sample_lp_solution(period_by_block: &[(usize, f64)]) -> MarvinScheduleSolution {
    let assignments = period_by_block
        .iter()
        .flat_map(|(linear_index, representative_period)| {
            let lower = representative_period.floor().max(0.0) as usize;
            let upper = representative_period.ceil().max(0.0) as usize;
            if lower == upper {
                return vec![(*linear_index, 0usize, lower, 1.0)];
            }

            let upper_fraction = representative_period - lower as f64;
            let lower_fraction = 1.0 - upper_fraction;
            vec![
                (*linear_index, 0usize, lower, lower_fraction),
                (*linear_index, 0usize, upper, upper_fraction),
            ]
        })
        .collect::<Vec<_>>();
    sample_lp_solution_from_assignments(&assignments)
}

fn sample_lp_solution_from_assignments(
    assignments: &[(usize, usize, usize, f64)],
) -> MarvinScheduleSolution {
    MarvinScheduleSolution {
        kind: MarvinScheduleProblemKind::Pcpsp,
        unique_block_count: assignments
            .iter()
            .map(|(linear_index, _, _, _)| *linear_index)
            .collect::<BTreeSet<_>>()
            .len(),
        assignments: assignments
            .iter()
            .map(
                |(linear_index, destination_index, period_index, fraction)| {
                    MarvinScheduleAssignment {
                        linear_index: *linear_index,
                        destination_index: *destination_index,
                        period_index: *period_index,
                        fraction: *fraction,
                    }
                },
            )
            .collect(),
    }
}

fn sample_scheduling_problem(topological_order: bool) -> SchedulingProblem {
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

    let ordered_units = if topological_order {
        vec![
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
        ]
    } else {
        vec![
            SchedulingUnit::new(
                unit_b_part_01.clone(),
                1.0,
                1,
                vec![unit_a_part_01.clone()],
                vec![destination_id.clone()],
                Vec::new(),
                Vec::new(),
                Some(99),
                Some(0),
                Metadata::new(),
            )
            .expect("unit should be valid"),
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
        ]
    };

    let objective_terms = ordered_units
        .iter()
        .map(|unit| {
            SchedulingObjectiveTerm::new(unit.unit_id().clone(), Some(destination_id.clone()), 1.0)
                .expect("objective should be valid")
        })
        .collect();

    SchedulingProblem::new(
        ScenarioId::new("lp-bz-rounder-test").expect("scenario id should be valid"),
        ModelId::new("lp-bz-rounder-model").expect("model id should be valid"),
        vec![period_01, period_02, period_03],
        ordered_units,
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

fn sample_scheduling_problem_with_partial_lp_mass() -> SchedulingProblem {
    let period_01 = SchedulingPeriod::new("P01", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_02 = SchedulingPeriod::new("P02", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_03 = SchedulingPeriod::new("P03", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let destination_id =
        ScheduleDestinationId::new("dest-00").expect("destination id should be valid");
    let unit_a = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let unit_b = SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            unit_a.clone(),
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
            unit_b.clone(),
            1.0,
            1,
            vec![unit_a.clone()],
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
        SchedulingObjectiveTerm::new(unit_a, Some(destination_id.clone()), 1.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_b, Some(destination_id.clone()), 1.0)
            .expect("objective should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-bz-rounder-partial-mass-test").expect("scenario id should be valid"),
        ModelId::new("lp-bz-rounder-partial-mass-model").expect("model id should be valid"),
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

fn sample_scheduling_problem_with_objectives(
    topological_order: bool,
    objective_values: (f64, f64, f64),
) -> SchedulingProblem {
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

    let ordered_units = if topological_order {
        vec![
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
        ]
    } else {
        vec![
            SchedulingUnit::new(
                unit_b_part_01.clone(),
                1.0,
                1,
                vec![unit_a_part_01.clone()],
                vec![destination_id.clone()],
                Vec::new(),
                Vec::new(),
                Some(99),
                Some(0),
                Metadata::new(),
            )
            .expect("unit should be valid"),
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
        ]
    };

    let (value_a1, value_a2, value_b1) = objective_values;
    let objective_terms = vec![
        SchedulingObjectiveTerm::new(unit_a_part_01, Some(destination_id.clone()), value_a1)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_a_part_02, Some(destination_id.clone()), value_a2)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_b_part_01, Some(destination_id.clone()), value_b1)
            .expect("objective should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-bz-rounder-test-objective").expect("scenario id should be valid"),
        ModelId::new("lp-bz-rounder-model-objective").expect("model id should be valid"),
        vec![period_01, period_02, period_03],
        ordered_units,
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

fn sample_scheduling_problem_for_local_swap_optimizer() -> SchedulingProblem {
    let period_01 = SchedulingPeriod::new("P01", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_02 = SchedulingPeriod::new("P02", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_03 = SchedulingPeriod::new("P03", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let destination_id =
        ScheduleDestinationId::new("dest-00").expect("destination id should be valid");
    let low_unit = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let high_unit = SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid");
    let downstream_unit =
        SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            low_unit.clone(),
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
            high_unit.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(100),
            Some(1),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            downstream_unit.clone(),
            1.0,
            1,
            vec![high_unit.clone()],
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
        SchedulingObjectiveTerm::new(low_unit, Some(destination_id.clone()), 5.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(high_unit, Some(destination_id.clone()), 150.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(downstream_unit, Some(destination_id.clone()), 1.0)
            .expect("objective should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-bz-rounder-test-local-opt").expect("scenario id should be valid"),
        ModelId::new("lp-bz-rounder-model-local-opt").expect("model id should be valid"),
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

fn sample_scheduling_problem_for_precedence_chain_optimizer() -> SchedulingProblem {
    let period_01 = SchedulingPeriod::new("P01", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_02 = SchedulingPeriod::new("P02", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_03 = SchedulingPeriod::new("P03", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let destination_id =
        ScheduleDestinationId::new("dest-00").expect("destination id should be valid");
    let blocker_unit = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let predecessor_unit =
        SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");
    let anchor_unit = SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            blocker_unit.clone(),
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
            predecessor_unit.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            anchor_unit.clone(),
            1.0,
            1,
            vec![predecessor_unit.clone()],
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(98),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
    ];
    let objective_terms = vec![
        SchedulingObjectiveTerm::new(blocker_unit, Some(destination_id.clone()), 100.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(predecessor_unit, Some(destination_id.clone()), 10.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(anchor_unit, Some(destination_id.clone()), 150.0)
            .expect("objective should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-bz-rounder-test-chain-opt").expect("scenario id should be valid"),
        ModelId::new("lp-bz-rounder-model-chain-opt").expect("model id should be valid"),
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

fn sample_scheduling_problem_for_period_ejection_optimizer() -> SchedulingProblem {
    let period_01 = SchedulingPeriod::new("P01", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_02 = SchedulingPeriod::new("P02", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_03 = SchedulingPeriod::new("P03", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_04 = SchedulingPeriod::new("P04", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let destination_id =
        ScheduleDestinationId::new("dest-00").expect("destination id should be valid");
    let stable_unit = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let mover_unit = SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");
    let lock_unit = SchedulingUnitId::new("phase-b::part-02").expect("unit id should be valid");
    let blocker_unit = SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid");
    let anchor_unit = SchedulingUnitId::new("phase-d::part-01").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            stable_unit.clone(),
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
            mover_unit.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            lock_unit.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            blocker_unit.clone(),
            1.0,
            1,
            vec![lock_unit.clone()],
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(98),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            anchor_unit.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(97),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
    ];
    let objective_terms = vec![
        SchedulingObjectiveTerm::new(stable_unit, Some(destination_id.clone()), 400.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(mover_unit, Some(destination_id.clone()), 10.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(lock_unit, Some(destination_id.clone()), 8.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(blocker_unit, Some(destination_id.clone()), 200.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(anchor_unit, Some(destination_id.clone()), 120.0)
            .expect("objective should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-bz-rounder-test-period-ejection").expect("scenario id should be valid"),
        ModelId::new("lp-bz-rounder-model-period-ejection").expect("model id should be valid"),
        vec![period_01, period_02, period_03, period_04],
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

fn sample_phase_plan_with_precedence_slack() -> PushbackPlan {
    let phase_a = PhaseDesign {
        phase_id: "phase-a".to_owned(),
        pushback_index: 0,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(100),
        block_count: 1,
        total_tonnage: Some(1.0),
        block_indices: vec![10],
        predecessor_phase_ids: Vec::new(),
    };
    let phase_c = PhaseDesign {
        phase_id: "phase-c".to_owned(),
        pushback_index: 1,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(100),
        block_count: 1,
        total_tonnage: Some(1.0),
        block_indices: vec![30],
        predecessor_phase_ids: Vec::new(),
    };
    let phase_b = PhaseDesign {
        phase_id: "phase-b".to_owned(),
        pushback_index: 2,
        shell_index: Some(0),
        revenue_factor: Some(1.0),
        bench: Some(99),
        block_count: 1,
        total_tonnage: Some(1.0),
        block_indices: vec![20],
        predecessor_phase_ids: vec!["phase-a".to_owned(), "phase-c".to_owned()],
    };
    PushbackPlan {
        phase_count: 3,
        total_block_count: 3,
        total_tonnage: Some(3.0),
        phases: vec![phase_a, phase_c, phase_b],
        nesting_rules: NestingAccessRules::default_open(),
        limitations: vec!["test fixture".to_owned()],
    }
}

fn sample_scheduling_problem_with_precedence_slack() -> SchedulingProblem {
    let period_01 = SchedulingPeriod::new("P01", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_02 = SchedulingPeriod::new("P02", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let period_03 = SchedulingPeriod::new("P03", Vec::new(), Vec::new(), Vec::new())
        .expect("period should be valid");
    let destination_id =
        ScheduleDestinationId::new("dest-00").expect("destination id should be valid");

    let unit_a = SchedulingUnitId::new("phase-a::part-01").expect("unit id should be valid");
    let unit_c = SchedulingUnitId::new("phase-c::part-01").expect("unit id should be valid");
    let unit_b_constrained =
        SchedulingUnitId::new("phase-b::part-01").expect("unit id should be valid");
    let unit_b_slack = SchedulingUnitId::new("phase-b::part-02").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            unit_a.clone(),
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
            unit_c.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(100),
            Some(1),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            unit_b_constrained.clone(),
            1.0,
            1,
            vec![unit_a.clone()],
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("unit should be valid"),
        SchedulingUnit::new(
            unit_b_slack.clone(),
            1.0,
            1,
            vec![unit_c.clone()],
            vec![destination_id.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(1),
            Metadata::new(),
        )
        .expect("unit should be valid"),
    ];

    let objective_terms = vec![
        SchedulingObjectiveTerm::new(unit_a, Some(destination_id.clone()), 1.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_c, Some(destination_id.clone()), 1.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_b_constrained, Some(destination_id.clone()), 120.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_b_slack, Some(destination_id.clone()), 80.0)
            .expect("objective should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-bz-rounder-test-slack").expect("scenario id should be valid"),
        ModelId::new("lp-bz-rounder-model-slack").expect("model id should be valid"),
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

fn phase_id_for_test_unit(unit_id: &SchedulingUnitId) -> &str {
    unit_id
        .as_str()
        .split("::part-")
        .next()
        .unwrap_or_else(|| unit_id.as_str())
}

fn discounted_target_score(
    scheduling_problem: &SchedulingProblem,
    target_period_by_unit: &BTreeMap<SchedulingUnitId, usize>,
) -> f64 {
    let objective_score_by_unit = scheduling_problem.objective_terms().iter().fold(
        BTreeMap::<SchedulingUnitId, f64>::new(),
        |mut acc, term| {
            acc.entry(term.unit_id().clone())
                .and_modify(|current| {
                    if term.value() > *current {
                        *current = term.value();
                    }
                })
                .or_insert(term.value());
            acc
        },
    );
    let discount_factor = 1.0 + scheduling_problem.discount_rate();
    target_period_by_unit
        .iter()
        .map(|(unit_id, period_index)| {
            objective_score_by_unit.get(unit_id).copied().unwrap_or(0.0)
                / discount_factor.powi(*period_index as i32)
        })
        .sum()
}
