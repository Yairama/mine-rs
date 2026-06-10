#[path = "../src/lp_bz_lp_kernel.rs"]
mod lp_bz_lp_kernel;

use std::collections::{BTreeMap, BTreeSet};

use lp_bz_lp_kernel::{
    LpBzCutTighteningStrategy, LpBzLpKernelAccessArtifact, LpBzLpKernelArtifact,
    LpBzLpKernelConstraintArtifact, LpBzLpKernelConstraintKind, LpBzLpKernelConstraintRow,
    LpBzLpKernelConstraintSense, LpBzLpKernelConstraintSummary, LpBzLpKernelObjectiveArtifact,
    LpBzLpKernelObjectiveCoefficient, LpBzLpKernelObjectiveSummary, LpBzLpKernelVariableEntry,
    LpBzLpKernelVariableIndexArtifact, LpBzLpKernelVariableKey, LpBzLpSolveStatus,
    LpBzPrecedenceCoverageCompleteness, LpBzPrecedenceEnforcementStrategy,
    build_lp_bz_lp_kernel_artifact, solve_lp_bz_lp_kernel_artifact,
};
use mine_sdk::{
    Metadata, ModelId, ScenarioId, ScheduleDestinationId, SchedulingObjectiveTerm,
    SchedulingPeriod, SchedulingProblem, SchedulingResourceBound, SchedulingResourceId,
    SchedulingResourceRequirement, SchedulingUnit, SchedulingUnitId,
};

#[test]
fn lp_kernel_builds_deterministic_unit_destination_period_artifacts() {
    let problem = sample_problem(false);

    let artifact_first =
        build_lp_bz_lp_kernel_artifact(&problem).expect("LP kernel artifact should build");
    let artifact_second = build_lp_bz_lp_kernel_artifact(&problem)
        .expect("LP kernel artifact should be deterministic");
    assert_eq!(artifact_first, artifact_second);

    assert_eq!(artifact_first.variable_index.variable_count, 6);
    assert_eq!(artifact_first.objective.summary.coefficient_count, 6);
    assert_eq!(artifact_first.constraints.summary.capacity_row_count, 2);
    assert_eq!(artifact_first.constraints.summary.activation_row_count, 2);
    assert_eq!(artifact_first.constraints.summary.precedence_row_count, 2);
    assert_eq!(artifact_first.constraints.summary.row_count, 6);

    let variable_index = artifact_first
        .variable_index
        .entries
        .iter()
        .find(|entry| {
            entry.key.unit_id == "unit-a"
                && entry.key.destination_id == "dest-a"
                && entry.key.period_index == 1
        })
        .expect("unit-a/dest-a/P02 variable should exist")
        .variable_index;
    let coefficient = artifact_first
        .objective
        .coefficients
        .iter()
        .find(|entry| entry.variable_index == variable_index)
        .expect("objective coefficient should exist");
    assert!((coefficient.coefficient - (100.0 / 1.1)).abs() <= 1.0e-9);

    let precedence_p01 = artifact_first
        .constraints
        .rows
        .iter()
        .find(|row| {
            row.kind == LpBzLpKernelConstraintKind::PrecedenceActivation
                && row.predecessor_unit_id.as_deref() == Some("unit-a")
                && row.successor_unit_id.as_deref() == Some("unit-b")
                && row.period_index == Some(0)
        })
        .expect("precedence row for period 0 should exist");
    assert_eq!(precedence_p01.sense, LpBzLpKernelConstraintSense::LessEqual);
    assert_eq!(precedence_p01.terms.len(), 3);

    let coefficients_by_variable = precedence_p01
        .terms
        .iter()
        .map(|term| (term.variable_index, term.coefficient))
        .collect::<BTreeMap<_, _>>();
    let successor_index = artifact_first
        .variable_index
        .entries
        .iter()
        .find(|entry| {
            entry.key.unit_id == "unit-b"
                && entry.key.destination_id == "dest-a"
                && entry.key.period_index == 0
        })
        .expect("unit-b/dest-a/P01 variable should exist")
        .variable_index;
    let predecessor_dest_a_index = artifact_first
        .variable_index
        .entries
        .iter()
        .find(|entry| {
            entry.key.unit_id == "unit-a"
                && entry.key.destination_id == "dest-a"
                && entry.key.period_index == 0
        })
        .expect("unit-a/dest-a/P01 variable should exist")
        .variable_index;
    let predecessor_dest_b_index = artifact_first
        .variable_index
        .entries
        .iter()
        .find(|entry| {
            entry.key.unit_id == "unit-a"
                && entry.key.destination_id == "dest-b"
                && entry.key.period_index == 0
        })
        .expect("unit-a/dest-b/P01 variable should exist")
        .variable_index;
    assert_eq!(coefficients_by_variable.get(&successor_index), Some(&1.0));
    assert_eq!(
        coefficients_by_variable.get(&predecessor_dest_a_index),
        Some(&-1.0)
    );
    assert_eq!(
        coefficients_by_variable.get(&predecessor_dest_b_index),
        Some(&-1.0)
    );
}

#[test]
fn lp_kernel_rejects_duplicate_destination_specific_objective_terms() {
    let problem = sample_problem(true);
    let error = build_lp_bz_lp_kernel_artifact(&problem)
        .expect_err("duplicate objective terms should be rejected");
    assert!(
        error
            .to_string()
            .contains("duplicate destination-specific coefficient"),
        "unexpected error: {error}"
    );
}

#[test]
fn lp_kernel_native_solve_returns_optimal_discounted_upper_bound() {
    let problem = sample_problem(false);
    let kernel = build_lp_bz_lp_kernel_artifact(&problem).expect("kernel artifact should build");
    let solve_artifact =
        solve_lp_bz_lp_kernel_artifact(&kernel).expect("native LP kernel solve should run");

    assert_eq!(solve_artifact.solve_status, LpBzLpSolveStatus::Optimal);
    assert_eq!(solve_artifact.variable_count, 6);
    assert!(
        solve_artifact.active_variable_count >= 2,
        "expected at least unit-a and unit-b active variables"
    );
    assert!(
        solve_artifact
            .max_variable_value
            .expect("max variable should exist")
            <= 1.0 + 1.0e-9
    );
    assert!(
        (solve_artifact
            .discounted_objective_bound
            .expect("objective should exist")
            - 160.0)
            .abs()
            <= 1.0e-9
    );
    assert_eq!(
        solve_artifact.precedence_diagnostics.strategy,
        LpBzPrecedenceEnforcementStrategy::FullPerPeriod
    );
    assert_eq!(
        solve_artifact.precedence_diagnostics.total_precedence_rows,
        solve_artifact
            .precedence_diagnostics
            .enforced_precedence_rows
    );
    assert_eq!(
        solve_artifact.precedence_diagnostics.coverage_completeness,
        LpBzPrecedenceCoverageCompleteness::Complete
    );
    assert_eq!(
        solve_artifact.precedence_diagnostics.coverage_basis_points,
        Some(10_000)
    );
    assert_eq!(
        solve_artifact
            .precedence_diagnostics
            .skipped_precedence_rows,
        0
    );
    assert_eq!(
        solve_artifact.cut_diagnostics.strategy,
        LpBzCutTighteningStrategy::PrecedenceCumulativePrefix
    );
    assert!(
        solve_artifact.cut_diagnostics.total_applied_row_count > 0,
        "full precedence solve should add cumulative-prefix cuts"
    );
    assert!(
        solve_artifact.limitations[0].contains("full per-period precedence"),
        "limitations should describe full precedence enforcement"
    );
    assert!(
        solve_artifact.limitations[0].contains("coverage complete (100.00%)"),
        "limitations should report explicit full precedence coverage"
    );
}

#[test]
fn lp_kernel_native_solve_is_deterministic_on_small_fixture() {
    let problem = sample_problem(false);
    let kernel = build_lp_bz_lp_kernel_artifact(&problem).expect("kernel artifact should build");
    let first = solve_lp_bz_lp_kernel_artifact(&kernel).expect("first solve should succeed");
    let second = solve_lp_bz_lp_kernel_artifact(&kernel).expect("second solve should succeed");
    assert_eq!(first, second);
}

#[test]
fn lp_kernel_build_artifact_limitations_describe_separate_native_solve_step() {
    let problem = sample_problem(false);
    let kernel = build_lp_bz_lp_kernel_artifact(&problem).expect("kernel artifact should build");

    assert!(
        kernel.limitations[0].contains("native LP solve runs as a separate step"),
        "artifact limitations should describe the build/solve split"
    );
}

#[test]
fn lp_kernel_precedence_tightening_beats_activation_only_relaxation() {
    let problem = precedence_tightening_problem();
    let kernel = build_lp_bz_lp_kernel_artifact(&problem).expect("kernel artifact should build");

    let precedence_solve =
        solve_lp_bz_lp_kernel_artifact(&kernel).expect("precedence solve should succeed");
    let precedence_bound = precedence_solve
        .discounted_objective_bound
        .expect("precedence objective should exist");

    let mut activation_only_kernel = kernel.clone();
    activation_only_kernel
        .constraints
        .rows
        .retain(|row| row.kind != LpBzLpKernelConstraintKind::PrecedenceActivation);
    activation_only_kernel.constraints.summary.row_count =
        activation_only_kernel.constraints.rows.len();
    activation_only_kernel
        .constraints
        .summary
        .precedence_row_count = 0;
    let activation_only_solve = solve_lp_bz_lp_kernel_artifact(&activation_only_kernel)
        .expect("activation-only solve should succeed");
    let activation_only_bound = activation_only_solve
        .discounted_objective_bound
        .expect("activation-only objective should exist");

    assert!(
        precedence_bound + 1.0e-9 < activation_only_bound,
        "precedence bound ({precedence_bound}) should be tighter than activation-only bound ({activation_only_bound})"
    );
    assert_eq!(
        precedence_solve.precedence_diagnostics.strategy,
        LpBzPrecedenceEnforcementStrategy::FullPerPeriod
    );
    assert_eq!(
        precedence_solve
            .precedence_diagnostics
            .skipped_precedence_rows,
        0
    );
    assert!(
        precedence_solve.limitations[0].contains("enforced"),
        "precedence limitations should report enforced precedence rows"
    );
}

#[test]
fn lp_kernel_cumulative_prefix_cuts_tighten_legacy_precedence_rows() {
    let problem = precedence_tightening_problem();
    let kernel = build_lp_bz_lp_kernel_artifact(&problem).expect("kernel artifact should build");

    let tightened_solve =
        solve_lp_bz_lp_kernel_artifact(&kernel).expect("tightened solve should succeed");
    let tightened_bound = tightened_solve
        .discounted_objective_bound
        .expect("tightened objective should exist");

    let mut legacy_kernel = kernel.clone();
    for row in &mut legacy_kernel.constraints.rows {
        if row.kind == LpBzLpKernelConstraintKind::PrecedenceActivation {
            row.predecessor_unit_id = None;
            row.successor_unit_id = None;
        }
    }
    let legacy_solve =
        solve_lp_bz_lp_kernel_artifact(&legacy_kernel).expect("legacy solve should succeed");
    let legacy_bound = legacy_solve
        .discounted_objective_bound
        .expect("legacy objective should exist");

    assert_eq!(tightened_solve.solve_status, LpBzLpSolveStatus::Optimal);
    assert_eq!(legacy_solve.solve_status, LpBzLpSolveStatus::Optimal);
    assert!(
        tightened_bound + 1.0e-9 < legacy_bound,
        "cumulative-prefix cuts should tighten bound ({tightened_bound}) vs legacy precedence rows ({legacy_bound})"
    );
    assert_eq!(
        tightened_solve.cut_diagnostics.strategy,
        LpBzCutTighteningStrategy::PrecedenceCumulativePrefix
    );
    assert!(
        tightened_solve.cut_diagnostics.total_applied_row_count > 0,
        "tightened solve should apply cumulative-prefix cuts"
    );
    assert_eq!(
        legacy_solve.cut_diagnostics.strategy,
        LpBzCutTighteningStrategy::None
    );
    assert_eq!(legacy_solve.cut_diagnostics.total_applied_row_count, 0);
}

#[test]
fn lp_kernel_hybrid_precedence_reports_partial_coverage_explicitly() {
    let total_precedence_rows = 200_001usize;
    let period_count = 12usize;
    let kernel = LpBzLpKernelArtifact {
        kernel_label: "hybrid-coverage-fixture".to_owned(),
        period_count,
        unit_count: 1,
        destination_count: 1,
        discount_rate: 0.0,
        variable_index: LpBzLpKernelVariableIndexArtifact {
            variable_count: 1,
            entries: vec![LpBzLpKernelVariableEntry {
                variable_index: 0,
                key: LpBzLpKernelVariableKey {
                    unit_id: "unit-a".to_owned(),
                    destination_id: "dest-a".to_owned(),
                    period_index: 0,
                },
                period_label: "P01".to_owned(),
            }],
        },
        objective: LpBzLpKernelObjectiveArtifact {
            summary: LpBzLpKernelObjectiveSummary {
                coefficient_count: 1,
                non_zero_coefficient_count: 1,
            },
            coefficients: vec![LpBzLpKernelObjectiveCoefficient {
                variable_index: 0,
                coefficient: 1.0,
                undiscounted_value: 1.0,
                discount_factor: 1.0,
            }],
        },
        constraints: LpBzLpKernelConstraintArtifact {
            summary: LpBzLpKernelConstraintSummary {
                row_count: total_precedence_rows,
                capacity_row_count: 0,
                activation_row_count: 0,
                precedence_row_count: total_precedence_rows,
            },
            rows: (0..total_precedence_rows)
                .map(|row_index| LpBzLpKernelConstraintRow {
                    row_index,
                    row_id: format!("precedence-row-{row_index}"),
                    kind: LpBzLpKernelConstraintKind::PrecedenceActivation,
                    sense: LpBzLpKernelConstraintSense::LessEqual,
                    rhs: 0.0,
                    period_index: Some(row_index % period_count),
                    period_label: Some(format!("P{:02}", (row_index % period_count) + 1)),
                    resource_id: None,
                    unit_id: None,
                    predecessor_unit_id: None,
                    successor_unit_id: None,
                    terms: Vec::new(),
                })
                .collect(),
        },
        access: LpBzLpKernelAccessArtifact {
            unit_profile_count: 0,
            unit_profiles: Vec::new(),
        },
        limitations: vec!["synthetic hybrid coverage fixture".to_owned()],
    };

    let solve_artifact =
        solve_lp_bz_lp_kernel_artifact(&kernel).expect("hybrid coverage solve should succeed");

    assert_eq!(solve_artifact.solve_status, LpBzLpSolveStatus::Optimal);
    assert_eq!(
        solve_artifact.precedence_diagnostics.strategy,
        LpBzPrecedenceEnforcementStrategy::HybridCheckpoint
    );
    assert_eq!(
        solve_artifact.precedence_diagnostics.coverage_completeness,
        LpBzPrecedenceCoverageCompleteness::Partial
    );
    assert_eq!(
        solve_artifact.precedence_diagnostics.coverage_basis_points,
        Some(2_500)
    );
    assert!(
        solve_artifact
            .precedence_diagnostics
            .enforced_precedence_rows
            < solve_artifact.precedence_diagnostics.total_precedence_rows,
        "hybrid coverage should enforce only a subset of precedence rows"
    );
    assert!(
        solve_artifact.limitations[0].contains("coverage partial (25.00%)"),
        "limitations should report explicit partial precedence coverage"
    );
}

#[test]
fn lp_kernel_full_precedence_tightens_vs_legacy_checkpoint_sampling() {
    let problem = legacy_sampling_gap_problem();
    let kernel = build_lp_bz_lp_kernel_artifact(&problem).expect("kernel artifact should build");

    let full_precedence_solve =
        solve_lp_bz_lp_kernel_artifact(&kernel).expect("full precedence solve should succeed");
    let full_precedence_bound = full_precedence_solve
        .discounted_objective_bound
        .expect("full precedence objective should exist");

    let mut legacy_checkpoint_kernel = kernel.clone();
    let legacy_checkpoint_periods =
        legacy_precedence_checkpoint_period_indices(kernel.period_count)
            .into_iter()
            .collect::<BTreeSet<_>>();
    legacy_checkpoint_kernel.constraints.rows.retain(|row| {
        row.kind != LpBzLpKernelConstraintKind::PrecedenceActivation
            || row
                .period_index
                .is_some_and(|period_index| legacy_checkpoint_periods.contains(&period_index))
    });
    legacy_checkpoint_kernel.constraints.summary.row_count =
        legacy_checkpoint_kernel.constraints.rows.len();
    legacy_checkpoint_kernel
        .constraints
        .summary
        .precedence_row_count = legacy_checkpoint_kernel
        .constraints
        .rows
        .iter()
        .filter(|row| row.kind == LpBzLpKernelConstraintKind::PrecedenceActivation)
        .count();

    let legacy_checkpoint_solve = solve_lp_bz_lp_kernel_artifact(&legacy_checkpoint_kernel)
        .expect("legacy checkpoint solve should succeed");
    let legacy_checkpoint_bound = legacy_checkpoint_solve
        .discounted_objective_bound
        .expect("legacy checkpoint objective should exist");

    assert!(
        full_precedence_bound <= legacy_checkpoint_bound + 1.0e-9,
        "full precedence bound ({full_precedence_bound}) should not be weaker than legacy checkpoint bound ({legacy_checkpoint_bound})"
    );
    assert!(
        full_precedence_solve
            .precedence_diagnostics
            .skipped_precedence_rows
            == 0,
        "full precedence solve should not skip precedence rows on this fixture"
    );
    assert!(
        legacy_checkpoint_solve
            .precedence_diagnostics
            .total_precedence_rows
            < kernel.constraints.summary.precedence_row_count,
        "legacy checkpoint fixture should emulate the old sampled precedence kernel"
    );
}

#[test]
fn lp_kernel_access_closure_capacity_cuts_tighten_sparse_access_relaxation() {
    let problem = access_closure_tightening_problem();
    let kernel = build_lp_bz_lp_kernel_artifact(&problem).expect("kernel artifact should build");

    let mut sparse_kernel = kernel.clone();
    sparse_kernel.constraints.rows.retain(|row| {
        row.kind != LpBzLpKernelConstraintKind::PrecedenceActivation || row.period_index == Some(1)
    });
    for row in &mut sparse_kernel.constraints.rows {
        if row.kind == LpBzLpKernelConstraintKind::PrecedenceActivation {
            row.predecessor_unit_id = None;
            row.successor_unit_id = None;
        }
    }
    sparse_kernel.constraints.summary.row_count = sparse_kernel.constraints.rows.len();
    sparse_kernel.constraints.summary.precedence_row_count = sparse_kernel
        .constraints
        .rows
        .iter()
        .filter(|row| row.kind == LpBzLpKernelConstraintKind::PrecedenceActivation)
        .count();

    let tightened_solve = solve_lp_bz_lp_kernel_artifact(&sparse_kernel)
        .expect("tightened sparse solve should succeed");
    let tightened_bound = tightened_solve
        .discounted_objective_bound
        .expect("tightened sparse objective should exist");

    let mut legacy_sparse_kernel = sparse_kernel.clone();
    legacy_sparse_kernel.access.unit_profile_count = 0;
    legacy_sparse_kernel.access.unit_profiles.clear();
    let legacy_solve = solve_lp_bz_lp_kernel_artifact(&legacy_sparse_kernel)
        .expect("legacy sparse solve should succeed");
    let legacy_bound = legacy_solve
        .discounted_objective_bound
        .expect("legacy sparse objective should exist");

    assert!(
        tightened_bound + 1.0e-9 < legacy_bound,
        "access-closure cuts should tighten sparse access relaxation ({tightened_bound}) vs legacy sparse bound ({legacy_bound})"
    );
    assert_eq!(
        tightened_solve.cut_diagnostics.strategy,
        LpBzCutTighteningStrategy::AccessClosureCapacityPrefix
    );
    let access_family = tightened_solve
        .cut_diagnostics
        .families
        .iter()
        .find(|family| family.family_label == "access_closure_capacity_prefix")
        .expect("access-closure family diagnostics should exist");
    assert!(
        access_family.applied_row_count > 0,
        "tightened sparse solve should apply access-closure cuts"
    );
    assert_eq!(
        legacy_solve.cut_diagnostics.strategy,
        LpBzCutTighteningStrategy::None
    );
    assert_eq!(legacy_solve.cut_diagnostics.total_applied_row_count, 0);
    assert!(
        tightened_solve.limitations[0].contains("access_closure_capacity_prefix"),
        "limitations should mention the access-closure cut family"
    );
}

fn sample_problem(duplicate_objective_term: bool) -> SchedulingProblem {
    let mine_resource = SchedulingResourceId::new("mine").expect("resource id should be valid");
    let period_01 = SchedulingPeriod::new(
        "P01",
        vec![
            SchedulingResourceBound::new(mine_resource.clone(), None, Some(10.0))
                .expect("resource bound should be valid"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("period should be valid");
    let period_02 = SchedulingPeriod::new(
        "P02",
        vec![
            SchedulingResourceBound::new(mine_resource.clone(), None, Some(8.0))
                .expect("resource bound should be valid"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("period should be valid");

    let destination_a =
        ScheduleDestinationId::new("dest-a").expect("destination id should be valid");
    let destination_b =
        ScheduleDestinationId::new("dest-b").expect("destination id should be valid");
    let unit_a = SchedulingUnitId::new("unit-a").expect("unit id should be valid");
    let unit_b = SchedulingUnitId::new("unit-b").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            unit_a.clone(),
            10.0,
            10,
            Vec::new(),
            vec![destination_a.clone(), destination_b.clone()],
            Vec::new(),
            Vec::new(),
            Some(100),
            Some(0),
            Metadata::new(),
        )
        .expect("unit a should be valid"),
        SchedulingUnit::new(
            unit_b.clone(),
            8.0,
            8,
            vec![unit_a.clone()],
            vec![destination_a.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("unit b should be valid"),
    ];

    let mut objective_terms = vec![
        SchedulingObjectiveTerm::new(unit_a.clone(), Some(destination_a.clone()), 100.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_a.clone(), Some(destination_b.clone()), 80.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_b.clone(), Some(destination_a.clone()), 60.0)
            .expect("objective should be valid"),
    ];
    if duplicate_objective_term {
        objective_terms.push(
            SchedulingObjectiveTerm::new(unit_a.clone(), Some(destination_a.clone()), 95.0)
                .expect("duplicate objective should still be a valid term"),
        );
    }

    let resource_requirements = vec![
        SchedulingResourceRequirement::new(
            unit_a.clone(),
            mine_resource.clone(),
            Some(destination_a.clone()),
            4.0,
        )
        .expect("resource requirement should be valid"),
        SchedulingResourceRequirement::new(
            unit_a.clone(),
            mine_resource.clone(),
            Some(destination_b.clone()),
            3.0,
        )
        .expect("resource requirement should be valid"),
        SchedulingResourceRequirement::new(unit_b, mine_resource, Some(destination_a.clone()), 2.0)
            .expect("resource requirement should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-kernel-test").expect("scenario id should be valid"),
        ModelId::new("marvin").expect("model id should be valid"),
        vec![period_01, period_02],
        units,
        objective_terms,
        resource_requirements,
        vec![destination_a, destination_b],
        Vec::new(),
        0.1,
        Metadata::new(),
        vec!["test fixture".to_owned()],
    )
    .expect("sample scheduling problem should be valid")
}

fn precedence_tightening_problem() -> SchedulingProblem {
    let mine_resource = SchedulingResourceId::new("mine").expect("resource id should be valid");
    let period_01 = SchedulingPeriod::new(
        "P01",
        vec![
            SchedulingResourceBound::new(mine_resource.clone(), None, Some(2.0))
                .expect("resource bound should be valid"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("period should be valid");
    let period_02 = SchedulingPeriod::new(
        "P02",
        vec![
            SchedulingResourceBound::new(mine_resource.clone(), None, Some(2.0))
                .expect("resource bound should be valid"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("period should be valid");

    let destination = ScheduleDestinationId::new("dest-a").expect("destination id should be valid");
    let predecessor = SchedulingUnitId::new("pred").expect("unit id should be valid");
    let successor = SchedulingUnitId::new("succ").expect("unit id should be valid");

    let units = vec![
        SchedulingUnit::new(
            predecessor.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination.clone()],
            Vec::new(),
            Vec::new(),
            Some(100),
            Some(0),
            Metadata::new(),
        )
        .expect("predecessor unit should be valid"),
        SchedulingUnit::new(
            successor.clone(),
            1.0,
            1,
            vec![predecessor.clone()],
            vec![destination.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("successor unit should be valid"),
    ];

    let objective_terms = vec![
        SchedulingObjectiveTerm::new(predecessor.clone(), Some(destination.clone()), -120.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(successor.clone(), Some(destination.clone()), 100.0)
            .expect("objective should be valid"),
    ];
    let resource_requirements = vec![
        SchedulingResourceRequirement::new(
            predecessor.clone(),
            mine_resource.clone(),
            Some(destination.clone()),
            1.0,
        )
        .expect("resource requirement should be valid"),
        SchedulingResourceRequirement::new(
            successor,
            mine_resource,
            Some(destination.clone()),
            1.0,
        )
        .expect("resource requirement should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-kernel-precedence-tightening").expect("scenario id should be valid"),
        ModelId::new("marvin").expect("model id should be valid"),
        vec![period_01, period_02],
        units,
        objective_terms,
        resource_requirements,
        vec![destination],
        Vec::new(),
        0.1,
        Metadata::new(),
        vec!["precedence-tightening fixture".to_owned()],
    )
    .expect("precedence tightening problem should be valid")
}

fn legacy_sampling_gap_problem() -> SchedulingProblem {
    let mine_resource = SchedulingResourceId::new("mine").expect("resource id should be valid");
    let periods = (1..=5)
        .map(|period_number| {
            SchedulingPeriod::new(
                format!("P{period_number:02}"),
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("resource bound should be valid"),
                ],
                Vec::new(),
                Vec::new(),
            )
            .expect("period should be valid")
        })
        .collect::<Vec<_>>();

    let destination = ScheduleDestinationId::new("dest-a").expect("destination id should be valid");
    let predecessor = SchedulingUnitId::new("pred").expect("unit id should be valid");
    let successor = SchedulingUnitId::new("succ").expect("unit id should be valid");
    let units = vec![
        SchedulingUnit::new(
            predecessor.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination.clone()],
            Vec::new(),
            Vec::new(),
            Some(100),
            Some(0),
            Metadata::new(),
        )
        .expect("predecessor unit should be valid"),
        SchedulingUnit::new(
            successor.clone(),
            1.0,
            1,
            vec![predecessor.clone()],
            vec![destination.clone()],
            Vec::new(),
            Vec::new(),
            Some(99),
            Some(0),
            Metadata::new(),
        )
        .expect("successor unit should be valid"),
    ];

    let objective_terms = vec![
        SchedulingObjectiveTerm::new(predecessor.clone(), Some(destination.clone()), -200.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(successor.clone(), Some(destination.clone()), 100.0)
            .expect("objective should be valid"),
    ];
    let resource_requirements = vec![
        SchedulingResourceRequirement::new(
            predecessor.clone(),
            mine_resource.clone(),
            Some(destination.clone()),
            1.0,
        )
        .expect("resource requirement should be valid"),
        SchedulingResourceRequirement::new(
            successor,
            mine_resource,
            Some(destination.clone()),
            1.0,
        )
        .expect("resource requirement should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-kernel-legacy-checkpoint-gap").expect("scenario id should be valid"),
        ModelId::new("marvin").expect("model id should be valid"),
        periods,
        units,
        objective_terms,
        resource_requirements,
        vec![destination],
        Vec::new(),
        0.0,
        Metadata::new(),
        vec!["legacy checkpoint gap fixture".to_owned()],
    )
    .expect("legacy checkpoint gap problem should be valid")
}

fn access_closure_tightening_problem() -> SchedulingProblem {
    let mine_resource = SchedulingResourceId::new("mine").expect("resource id should be valid");
    let periods = (1..=2)
        .map(|period_number| {
            SchedulingPeriod::new(
                format!("P{period_number:02}"),
                vec![
                    SchedulingResourceBound::new(mine_resource.clone(), None, Some(1.0))
                        .expect("resource bound should be valid"),
                ],
                Vec::new(),
                Vec::new(),
            )
            .expect("period should be valid")
        })
        .collect::<Vec<_>>();

    let destination = ScheduleDestinationId::new("dest-a").expect("destination id should be valid");
    let unit_a = SchedulingUnitId::new("access-a").expect("unit id should be valid");
    let unit_b = SchedulingUnitId::new("access-b").expect("unit id should be valid");
    let unit_c = SchedulingUnitId::new("access-c").expect("unit id should be valid");
    let units = vec![
        SchedulingUnit::new(
            unit_a.clone(),
            1.0,
            1,
            Vec::new(),
            vec![destination.clone()],
            Vec::new(),
            Vec::new(),
            Some(120),
            Some(0),
            Metadata::new(),
        )
        .expect("unit a should be valid"),
        SchedulingUnit::new(
            unit_b.clone(),
            1.0,
            1,
            vec![unit_a.clone()],
            vec![destination.clone()],
            Vec::new(),
            Vec::new(),
            Some(119),
            Some(1),
            Metadata::new(),
        )
        .expect("unit b should be valid"),
        SchedulingUnit::new(
            unit_c.clone(),
            1.0,
            1,
            vec![unit_b.clone()],
            vec![destination.clone()],
            Vec::new(),
            Vec::new(),
            Some(118),
            Some(2),
            Metadata::new(),
        )
        .expect("unit c should be valid"),
    ];

    let objective_terms = vec![
        SchedulingObjectiveTerm::new(unit_a.clone(), Some(destination.clone()), -200.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_b.clone(), Some(destination.clone()), -50.0)
            .expect("objective should be valid"),
        SchedulingObjectiveTerm::new(unit_c.clone(), Some(destination.clone()), 300.0)
            .expect("objective should be valid"),
    ];
    let resource_requirements = vec![
        SchedulingResourceRequirement::new(
            unit_a,
            mine_resource.clone(),
            Some(destination.clone()),
            1.0,
        )
        .expect("resource requirement should be valid"),
        SchedulingResourceRequirement::new(
            unit_b,
            mine_resource.clone(),
            Some(destination.clone()),
            1.0,
        )
        .expect("resource requirement should be valid"),
        SchedulingResourceRequirement::new(unit_c, mine_resource, Some(destination.clone()), 1.0)
            .expect("resource requirement should be valid"),
    ];

    SchedulingProblem::new(
        ScenarioId::new("lp-kernel-access-closure-tightening")
            .expect("scenario id should be valid"),
        ModelId::new("marvin").expect("model id should be valid"),
        periods,
        units,
        objective_terms,
        resource_requirements,
        vec![destination],
        Vec::new(),
        0.0,
        Metadata::new(),
        vec!["access-closure tightening fixture".to_owned()],
    )
    .expect("access-closure tightening problem should be valid")
}

fn legacy_precedence_checkpoint_period_indices(period_count: usize) -> Vec<usize> {
    const LEGACY_PRECEDENCE_CHECKPOINT_COUNT: usize = 4;

    if period_count == 0 {
        return Vec::new();
    }
    if period_count == 1 {
        return vec![0];
    }

    let checkpoint_count = period_count.min(LEGACY_PRECEDENCE_CHECKPOINT_COUNT);
    let denominator = checkpoint_count - 1;
    let last_period_index = period_count - 1;
    let mut checkpoints = BTreeSet::new();
    for checkpoint_index in 0..checkpoint_count {
        let numerator = checkpoint_index * last_period_index;
        let rounded_index = (numerator + denominator / 2) / denominator;
        checkpoints.insert(rounded_index);
    }
    checkpoints.into_iter().collect()
}
