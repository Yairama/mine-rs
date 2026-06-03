#[path = "../src/benchmark_blocks_support.rs"]
mod benchmark_blocks_support;
#[path = "../src/marvin_support.rs"]
mod marvin_support;
#[path = "../src/minelib_scheduling_support.rs"]
mod minelib_scheduling_support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use marvin_support::{
    read_minelib_cpit_solution, read_minelib_precedence_graph, read_minelib_upit_block_values,
};
use mine_sdk::{ColumnId, NestingAccessRules, PhaseDesign, PushbackPlan, uniform_revenue_factors};
use minelib_scheduling_support::{
    NestedShellAccessMode, build_linear_index_to_row_index,
    build_marvin_phase_plan_from_revenue_factor_shells,
    build_marvin_preferred_nested_shell_family_contract,
    build_marvin_preferred_nested_shell_family_contract_for_phase_plan,
    build_phase_plan_from_nested_shells, build_phase_plan_from_reference_periods,
    build_preferred_phase_plan_for_minelib_scheduling,
};

fn benchmark_path(instance: &str, file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("datasets")
        .join("benchmarks")
        .join(instance)
        .join(file_name)
}

#[test]
fn build_reference_period_bench_phase_plan_from_marvin_cpit() {
    let model = read_benchmark_blocks(benchmark_path("marvin", "marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let cpit_solution = read_minelib_cpit_solution(
        benchmark_path("marvin", "references\\marvin_cpit_gmunoz120723.sol"),
        &model,
    )
    .expect("marvin CPIT solution should load");
    let linear_index_to_row_index =
        build_linear_index_to_row_index(&model).expect("lookup should build");
    let tonnage_column = ColumnId::new("field_4").expect("field_4 should be a valid column id");

    let phase_plan = build_phase_plan_from_reference_periods(
        &model,
        &linear_index_to_row_index,
        &cpit_solution.assignments,
        &tonnage_column,
        "Reference-period × bench aggregation test",
    )
    .expect("phase plan should build from CPIT memberships");

    let pushback_indices = phase_plan
        .phases
        .iter()
        .map(|phase| phase.pushback_index)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        phase_plan.total_block_count,
        cpit_solution.unique_block_count
    );
    assert!(pushback_indices.len() > 1);
    assert!(
        phase_plan
            .phases
            .iter()
            .all(|phase| phase.phase_id.starts_with("period-"))
    );
    assert!(phase_plan.phases.iter().all(|phase| phase.bench.is_some()));
    assert!(
        phase_plan
            .phases
            .iter()
            .any(|phase| phase.predecessor_phase_ids.len() > 1)
    );
    assert!(
        phase_plan.limitations[0].contains("Reference-period"),
        "limitations should describe the reference-period aggregation"
    );
}

#[test]
fn build_nested_shell_bench_phase_plan_from_marvin_upit() {
    let model = read_benchmark_blocks(benchmark_path("marvin", "marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let precedence_graph =
        read_minelib_precedence_graph(benchmark_path("marvin", "references\\marvin.prec"), &model)
            .expect("marvin precedence should load");
    let upit_block_values =
        read_minelib_upit_block_values(benchmark_path("marvin", "references\\marvin.upit"), &model)
            .expect("marvin upit values should load")
            .into_iter()
            .collect();
    let tonnage_column = ColumnId::new("field_4").expect("field_4 should be a valid column id");
    let revenue_factors = uniform_revenue_factors(3).expect("revenue factors should build");

    let artifacts = build_phase_plan_from_nested_shells(
        &model,
        &precedence_graph,
        &upit_block_values,
        &tonnage_column,
        &revenue_factors,
        "Nested-shell × bench aggregation test",
    )
    .expect("nested-shell phase plan should build");

    assert!(artifacts.shell_set.unique_shell_count >= 1);
    assert_eq!(artifacts.shell_set.factors_evaluated, 3);
    assert!(artifacts.phase_plan.phase_count >= artifacts.shell_set.unique_shell_count);
    assert!(
        artifacts
            .phase_plan
            .phases
            .iter()
            .all(|phase| phase.shell_index.is_some())
    );
    assert!(
        artifacts
            .phase_plan
            .phases
            .iter()
            .all(|phase| phase.bench.is_some())
    );
    assert!(
        artifacts
            .phase_plan
            .limitations
            .iter()
            .any(|limitation| limitation.contains("Nested-shell"))
    );
}

#[test]
fn build_marvin_revenue_factor_shells_produces_multiple_shells() {
    let model = read_benchmark_blocks(benchmark_path("marvin", "marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let precedence_graph =
        read_minelib_precedence_graph(benchmark_path("marvin", "references\\marvin.prec"), &model)
            .expect("marvin precedence should load");
    let revenue_factors = uniform_revenue_factors(7).expect("revenue factors should build");

    let artifacts = build_marvin_phase_plan_from_revenue_factor_shells(
        &model,
        &precedence_graph,
        &revenue_factors,
        NestingAccessRules::strict_sequential(),
        "Marvin revenue-factor shell test",
    )
    .expect("marvin revenue-factor phase plan should build");

    assert!(
        artifacts.shell_set.unique_shell_count > 1,
        "revenue/cost-aware factor scenarios should create more than one shell"
    );
    assert!(
        artifacts.phase_plan.phase_count >= artifacts.shell_set.unique_shell_count,
        "phase plan should preserve at least one phase per shell"
    );
}

#[test]
fn preferred_phase_plan_uses_marvin_nested_shell_primary_route() {
    let model = read_benchmark_blocks(benchmark_path("marvin", "marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");
    let cpit_solution = read_minelib_cpit_solution(
        benchmark_path("marvin", "references\\marvin_cpit_gmunoz120723.sol"),
        &model,
    )
    .expect("marvin CPIT solution should load");
    let precedence_graph =
        read_minelib_precedence_graph(benchmark_path("marvin", "references\\marvin.prec"), &model)
            .expect("marvin precedence should load");
    let linear_index_to_row_index =
        build_linear_index_to_row_index(&model).expect("lookup should build");
    let tonnage_column = ColumnId::new("field_4").expect("field_4 should be a valid column id");

    let preferred = build_preferred_phase_plan_for_minelib_scheduling(
        "marvin",
        true,
        &model,
        &linear_index_to_row_index,
        &cpit_solution.assignments,
        Some(&precedence_graph),
        &tonnage_column,
        7,
    )
    .expect("preferred Marvin phase plan should build");

    assert_eq!(
        preferred.metadata.aggregation_strategy,
        "nested-shell-bench"
    );
    assert!(preferred.metadata.nested_shell_primary);
    let preferred_shell_family = preferred
        .metadata
        .marvin_nested_shell_family_contract
        .expect("preferred Marvin shell family contract should be surfaced");
    assert_eq!(preferred_shell_family.revenue_factor_count, 7);
    assert_eq!(
        preferred_shell_family.revenue_factors,
        uniform_revenue_factors(7).expect("canonical revenue factors should build")
    );
    assert_eq!(
        preferred_shell_family.shell_access_mode,
        NestedShellAccessMode::StrictSequential
    );
    assert_eq!(
        preferred.phase_plan.nesting_rules,
        NestingAccessRules::strict_sequential()
    );
    assert!(
        preferred_shell_family
            .realized_shell_count
            .expect("realized shell count")
            > 1,
        "Marvin preferred route should keep the promoted multi-shell family"
    );
    assert_eq!(
        preferred.metadata.unique_shell_count,
        preferred_shell_family.realized_shell_count
    );
    assert!(
        preferred.metadata.descriptive_note.contains("7-factor"),
        "report note should describe the promoted seven-factor path"
    );
    assert!(
        preferred
            .metadata
            .descriptive_note
            .contains("strict-sequential"),
        "report note should describe the promoted strict-sequential path"
    );
    assert!(
        preferred
            .phase_plan
            .phases
            .iter()
            .all(|phase| phase.shell_index.is_some())
    );
}

#[test]
fn preferred_phase_plan_falls_back_to_reference_period_bench_when_nested_shell_is_disabled() {
    let model = read_benchmark_blocks(
        benchmark_path("mclaughlin", "mclaughlin.blocks"),
        "mclaughlin",
    )
    .expect("mclaughlin.blocks should load");
    let cpit_solution = read_minelib_cpit_solution(
        benchmark_path("mclaughlin", "references\\mclaughlin_cpit_gmunoz120723.sol"),
        &model,
    )
    .expect("mclaughlin CPIT solution should load");
    let linear_index_to_row_index =
        build_linear_index_to_row_index(&model).expect("lookup should build");
    let tonnage_column = ColumnId::new("field_5").expect("field_5 should be a valid column id");

    let preferred = build_preferred_phase_plan_for_minelib_scheduling(
        "mclaughlin",
        false,
        &model,
        &linear_index_to_row_index,
        &cpit_solution.assignments,
        None,
        &tonnage_column,
        7,
    )
    .expect("fallback reference-period phase plan should build");

    assert_eq!(
        preferred.metadata.aggregation_strategy,
        "reference-period-bench"
    );
    assert!(!preferred.metadata.nested_shell_primary);
    assert_eq!(preferred.metadata.unique_shell_count, None);
    assert_eq!(preferred.metadata.marvin_nested_shell_family_contract, None);
    assert!(
        preferred
            .metadata
            .descriptive_note
            .contains("nested-shell is not enabled"),
        "fallback note should explain why nested-shell is not primary"
    );
    assert!(
        preferred
            .phase_plan
            .phases
            .iter()
            .all(|phase| phase.phase_id.starts_with("period-"))
    );
    assert!(
        preferred
            .phase_plan
            .phases
            .iter()
            .all(|phase| phase.shell_index.is_none())
    );
}

#[test]
fn marvin_preferred_shell_family_contract_exposes_exact_factor_vector_and_access_mode() {
    let contract = build_marvin_preferred_nested_shell_family_contract(7)
        .expect("preferred Marvin shell family contract should build");

    assert_eq!(contract.aggregation_strategy, "nested-shell-bench");
    assert_eq!(contract.revenue_factor_count, 7);
    assert_eq!(
        contract.revenue_factors,
        uniform_revenue_factors(7).expect("canonical revenue factors should build")
    );
    assert_eq!(
        contract.shell_access_mode,
        NestedShellAccessMode::StrictSequential
    );
    assert_eq!(contract.realized_shell_count, None);
}

#[test]
fn marvin_preferred_shell_family_contract_for_phase_plan_tracks_realized_shell_count() {
    let phase_plan = PushbackPlan {
        phases: vec![
            PhaseDesign {
                phase_id: "shell-a".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(0.8),
                bench: Some(100),
                block_indices: vec![10, 11],
                block_count: 2,
                total_tonnage: Some(4.0),
                predecessor_phase_ids: Vec::new(),
            },
            PhaseDesign {
                phase_id: "shell-b".to_owned(),
                pushback_index: 1,
                shell_index: Some(2),
                revenue_factor: Some(1.2),
                bench: Some(99),
                block_indices: vec![20],
                block_count: 1,
                total_tonnage: Some(3.0),
                predecessor_phase_ids: vec!["shell-a".to_owned()],
            },
            PhaseDesign {
                phase_id: "bench-only".to_owned(),
                pushback_index: 1,
                shell_index: None,
                revenue_factor: None,
                bench: Some(98),
                block_indices: vec![30],
                block_count: 1,
                total_tonnage: Some(2.0),
                predecessor_phase_ids: vec!["shell-b".to_owned()],
            },
        ],
        phase_count: 3,
        total_block_count: 4,
        total_tonnage: Some(9.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: Vec::new(),
    };

    let contract =
        build_marvin_preferred_nested_shell_family_contract_for_phase_plan(7, &phase_plan)
            .expect("phase-plan contract should build");

    assert_eq!(contract.revenue_factor_count, 7);
    assert_eq!(
        contract.revenue_factors,
        uniform_revenue_factors(7).expect("canonical revenue factors should build")
    );
    assert_eq!(
        contract.shell_access_mode,
        NestedShellAccessMode::StrictSequential
    );
    assert_eq!(contract.realized_shell_count, Some(2));
}
