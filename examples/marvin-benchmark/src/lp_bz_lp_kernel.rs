#![allow(dead_code)]
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::collections::{BTreeMap, BTreeSet};

use mine_sdk::MineError;
use mine_sdk::SchedulingProblem;
use minilp::{ComparisonOp, OptimizationDirection, Problem as MiniLpProblem};
use serde::Serialize;

const EPSILON: f64 = 1.0e-9;
const PRECEDENCE_FULL_ENFORCEMENT_ROW_LIMIT: usize = 200_000;
const PRECEDENCE_HYBRID_TARGET_ROW_LIMIT: usize = 60_000;
const PRECEDENCE_HYBRID_MAX_PERIOD_COUNT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzLpKernelVariableKey {
    pub unit_id: String,
    pub destination_id: String,
    pub period_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzLpKernelVariableEntry {
    pub variable_index: usize,
    pub key: LpBzLpKernelVariableKey,
    pub period_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzLpKernelVariableIndexArtifact {
    pub variable_count: usize,
    pub entries: Vec<LpBzLpKernelVariableEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelObjectiveCoefficient {
    pub variable_index: usize,
    pub coefficient: f64,
    pub undiscounted_value: f64,
    pub discount_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzLpKernelObjectiveSummary {
    pub coefficient_count: usize,
    pub non_zero_coefficient_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelObjectiveArtifact {
    pub summary: LpBzLpKernelObjectiveSummary,
    pub coefficients: Vec<LpBzLpKernelObjectiveCoefficient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LpBzLpKernelConstraintSense {
    LessEqual,
    GreaterEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LpBzLpKernelConstraintKind {
    CapacityUpper,
    CapacityLower,
    ActivationUpper,
    PrecedenceActivation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelConstraintTerm {
    pub variable_index: usize,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelConstraintRow {
    pub row_index: usize,
    pub row_id: String,
    pub kind: LpBzLpKernelConstraintKind,
    pub sense: LpBzLpKernelConstraintSense,
    pub rhs: f64,
    pub period_index: Option<usize>,
    pub period_label: Option<String>,
    pub resource_id: Option<String>,
    pub unit_id: Option<String>,
    pub predecessor_unit_id: Option<String>,
    pub successor_unit_id: Option<String>,
    pub terms: Vec<LpBzLpKernelConstraintTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzLpKernelConstraintSummary {
    pub row_count: usize,
    pub capacity_row_count: usize,
    pub activation_row_count: usize,
    pub precedence_row_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelConstraintArtifact {
    pub summary: LpBzLpKernelConstraintSummary,
    pub rows: Vec<LpBzLpKernelConstraintRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelArtifact {
    pub kernel_label: String,
    pub period_count: usize,
    pub unit_count: usize,
    pub destination_count: usize,
    pub discount_rate: f64,
    pub variable_index: LpBzLpKernelVariableIndexArtifact,
    pub objective: LpBzLpKernelObjectiveArtifact,
    pub constraints: LpBzLpKernelConstraintArtifact,
    pub access: LpBzLpKernelAccessArtifact,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelAccessClosureResource {
    pub resource_id: String,
    pub minimum_total_requirement: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelAccessUnitProfile {
    pub unit_id: String,
    pub bench: Option<i64>,
    pub shell_index: Option<usize>,
    pub direct_predecessor_count: usize,
    pub transitive_predecessor_count: usize,
    pub closure_unit_count: usize,
    pub closure_resources: Vec<LpBzLpKernelAccessClosureResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpKernelAccessArtifact {
    pub unit_profile_count: usize,
    pub unit_profiles: Vec<LpBzLpKernelAccessUnitProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LpBzLpSolveStatus {
    Optimal,
    Infeasible,
    Unbounded,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LpBzPrecedenceEnforcementStrategy {
    None,
    FullPerPeriod,
    HybridCheckpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LpBzPrecedenceCoverageCompleteness {
    NotApplicable,
    Partial,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzPrecedenceSolveDiagnostics {
    pub strategy: LpBzPrecedenceEnforcementStrategy,
    pub max_enforced_precedence_rows: usize,
    pub total_precedence_rows: usize,
    pub enforced_precedence_rows: usize,
    pub skipped_precedence_rows: usize,
    pub coverage_completeness: LpBzPrecedenceCoverageCompleteness,
    pub coverage_basis_points: Option<u16>,
    pub enforced_period_indices: Vec<usize>,
    pub skipped_period_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LpBzCutTighteningStrategy {
    None,
    PrecedenceCumulativePrefix,
    AccessClosureCapacityPrefix,
    PrecedenceCumulativePrefixAndAccessClosureCapacityPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzCutFamilySolveDiagnostics {
    pub family_label: String,
    pub generated_row_count: usize,
    pub applied_row_count: usize,
    pub skipped_row_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzCutSolveDiagnostics {
    pub strategy: LpBzCutTighteningStrategy,
    pub total_generated_row_count: usize,
    pub total_applied_row_count: usize,
    pub total_skipped_row_count: usize,
    pub families: Vec<LpBzCutFamilySolveDiagnostics>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LpBzLpSolveArtifact {
    pub solver_label: String,
    pub solve_status: LpBzLpSolveStatus,
    pub discounted_objective_bound: Option<f64>,
    pub variable_count: usize,
    pub active_variable_count: usize,
    pub min_positive_variable_value: Option<f64>,
    pub max_variable_value: Option<f64>,
    pub precedence_diagnostics: LpBzPrecedenceSolveDiagnostics,
    pub cut_diagnostics: LpBzCutSolveDiagnostics,
    pub limitations: Vec<String>,
}

pub fn build_lp_bz_lp_kernel_artifact(
    scheduling_problem: &SchedulingProblem,
) -> Result<LpBzLpKernelArtifact, MineError> {
    if scheduling_problem.periods().is_empty() {
        return Err(MineError::validation(
            "LP/BZ kernel build requires at least one scheduling period".to_owned(),
        ));
    }
    if scheduling_problem.units().is_empty() {
        return Err(MineError::validation(
            "LP/BZ kernel build requires at least one scheduling unit".to_owned(),
        ));
    }
    if !scheduling_problem.discount_rate().is_finite() || scheduling_problem.discount_rate() < 0.0 {
        return Err(MineError::validation(format!(
            "LP/BZ kernel discount rate must be finite and non-negative (received {})",
            scheduling_problem.discount_rate()
        )));
    }

    let units_by_id = scheduling_problem
        .units()
        .iter()
        .map(|unit| (unit.unit_id().as_str().to_owned(), unit))
        .collect::<BTreeMap<_, _>>();
    let fallback_destination_ids = scheduling_problem
        .destination_ids()
        .iter()
        .map(|destination_id| destination_id.as_str().to_owned())
        .collect::<Vec<_>>();

    let unit_destination_ids = units_by_id
        .iter()
        .map(|(unit_id, unit)| {
            let mut destinations = if unit.eligible_destination_ids().is_empty() {
                fallback_destination_ids.clone()
            } else {
                unit.eligible_destination_ids()
                    .iter()
                    .map(|destination_id| destination_id.as_str().to_owned())
                    .collect::<Vec<_>>()
            };
            destinations.sort();
            destinations.dedup();
            if destinations.is_empty() {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel unit `{unit_id}` has no destinations (unit eligibility and global destination list are both empty)"
                )));
            }
            Ok((unit_id.clone(), destinations))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let (
        variable_index,
        variable_index_by_key,
        variable_indices_by_unit,
        variable_indices_by_unit_period,
    ) = build_variable_index(scheduling_problem, &units_by_id, &unit_destination_ids)?;
    let (resource_specific, resource_generic) = build_resource_requirement_lookups(
        scheduling_problem,
        &units_by_id,
        &unit_destination_ids,
    )?;
    let objective = build_objective_artifact(
        scheduling_problem,
        &variable_index.entries,
        &units_by_id,
        &unit_destination_ids,
    )?;
    let constraints = build_constraint_artifact(
        scheduling_problem,
        &units_by_id,
        &unit_destination_ids,
        &variable_index_by_key,
        &variable_indices_by_unit,
        &variable_indices_by_unit_period,
        &resource_specific,
        &resource_generic,
    )?;
    let access = build_access_artifact(
        scheduling_problem,
        &units_by_id,
        &unit_destination_ids,
        &resource_specific,
        &resource_generic,
    )?;

    Ok(LpBzLpKernelArtifact {
        kernel_label: "lp-bz-lp-kernel-v8-local-front-access-progression-scaffold".to_owned(),
        period_count: scheduling_problem.periods().len(),
        unit_count: scheduling_problem.units().len(),
        destination_count: scheduling_problem.destination_ids().len(),
        discount_rate: scheduling_problem.discount_rate(),
        variable_index,
        objective,
        constraints,
        access,
        limitations: vec![
            "This artifact is an in-harness deterministic LP-relaxation kernel scaffold (variable map, objective vector, and constraint rows); native LP solve runs as a separate step over this serialized artifact.".to_owned(),
        ],
    })
}

pub fn solve_lp_bz_lp_kernel_artifact(
    artifact: &LpBzLpKernelArtifact,
) -> Result<LpBzLpSolveArtifact, MineError> {
    if artifact.variable_index.variable_count == 0 {
        return Err(MineError::validation(
            "LP/BZ kernel solve requires at least one variable".to_owned(),
        ));
    }
    if artifact.objective.coefficients.len() != artifact.variable_index.variable_count {
        return Err(MineError::validation(format!(
            "LP/BZ kernel solve objective size mismatch: coefficients={} variables={}",
            artifact.objective.coefficients.len(),
            artifact.variable_index.variable_count
        )));
    }

    let mut seen_objective_coefficients = vec![false; artifact.variable_index.variable_count];
    let mut objective_coefficient_by_variable =
        vec![0.0_f64; artifact.variable_index.variable_count];
    for coefficient in &artifact.objective.coefficients {
        if coefficient.variable_index >= artifact.variable_index.variable_count {
            return Err(MineError::validation(format!(
                "LP/BZ kernel solve objective references out-of-range variable index {} (variable_count={})",
                coefficient.variable_index, artifact.variable_index.variable_count
            )));
        }
        if !coefficient.coefficient.is_finite() {
            return Err(MineError::validation(format!(
                "LP/BZ kernel solve objective coefficient for variable {} is not finite ({})",
                coefficient.variable_index, coefficient.coefficient
            )));
        }
        if seen_objective_coefficients[coefficient.variable_index] {
            return Err(MineError::validation(format!(
                "LP/BZ kernel solve objective contains duplicate coefficient entry for variable {}",
                coefficient.variable_index
            )));
        }
        seen_objective_coefficients[coefficient.variable_index] = true;
        objective_coefficient_by_variable[coefficient.variable_index] = coefficient.coefficient;
    }
    if seen_objective_coefficients.iter().any(|seen| !seen) {
        return Err(MineError::validation(
            "LP/BZ kernel solve objective is missing coefficient entries for some variables"
                .to_owned(),
        ));
    }

    let mut lp_problem = MiniLpProblem::new(OptimizationDirection::Maximize);
    let lp_variables = objective_coefficient_by_variable
        .into_iter()
        .map(|objective_coefficient| lp_problem.add_var(objective_coefficient, (0.0, 1.0)))
        .collect::<Vec<_>>();
    let total_precedence_rows = artifact
        .constraints
        .rows
        .iter()
        .filter(|row| row.kind == LpBzLpKernelConstraintKind::PrecedenceActivation)
        .count();
    if total_precedence_rows > 0 && artifact.period_count == 0 {
        return Err(MineError::validation(
            "LP/BZ kernel solve has precedence rows but period_count is zero".to_owned(),
        ));
    }
    let precedence_plan =
        build_precedence_enforcement_plan(artifact.period_count, total_precedence_rows);
    let precedence_enforced_period_set = precedence_plan
        .enforced_period_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut enforced_precedence_rows = 0usize;
    let mut skipped_precedence_rows = 0usize;
    let mut enforced_precedence_period_indices = BTreeSet::new();
    let mut skipped_precedence_period_indices = BTreeSet::new();

    for row in &artifact.constraints.rows {
        if row.kind != LpBzLpKernelConstraintKind::PrecedenceActivation {
            continue;
        }
        let period_index = row.period_index.ok_or_else(|| {
            MineError::validation(format!(
                "LP/BZ kernel precedence row `{}` is missing period metadata",
                row.row_id
            ))
        })?;
        if period_index >= artifact.period_count {
            return Err(MineError::validation(format!(
                "LP/BZ kernel precedence row `{}` references out-of-range period index {} (period_count={})",
                row.row_id, period_index, artifact.period_count
            )));
        }
        if precedence_enforced_period_set.contains(&period_index) {
            enforced_precedence_rows += 1;
            enforced_precedence_period_indices.insert(period_index);
        } else {
            skipped_precedence_rows += 1;
            skipped_precedence_period_indices.insert(period_index);
        }
    }
    let precedence_coverage_completeness = lp_bz_precedence_coverage_completeness(
        total_precedence_rows,
        enforced_precedence_rows,
        skipped_precedence_rows,
    );
    let precedence_coverage_basis_points =
        lp_bz_precedence_coverage_basis_points(total_precedence_rows, enforced_precedence_rows);
    let precedence_diagnostics = LpBzPrecedenceSolveDiagnostics {
        strategy: precedence_plan.strategy,
        max_enforced_precedence_rows: precedence_plan.max_enforced_precedence_rows,
        total_precedence_rows,
        enforced_precedence_rows,
        skipped_precedence_rows,
        coverage_completeness: precedence_coverage_completeness,
        coverage_basis_points: precedence_coverage_basis_points,
        enforced_period_indices: enforced_precedence_period_indices.into_iter().collect(),
        skipped_period_indices: skipped_precedence_period_indices.into_iter().collect(),
    };
    let enforced_period_labels = precedence_diagnostics
        .enforced_period_indices
        .iter()
        .map(|period_index| format!("p{:02}", period_index + 1))
        .collect::<Vec<_>>();
    let skipped_period_labels = precedence_diagnostics
        .skipped_period_indices
        .iter()
        .map(|period_index| format!("p{:02}", period_index + 1))
        .collect::<Vec<_>>();
    let precedence_coverage_label =
        lp_bz_precedence_coverage_label(precedence_diagnostics.coverage_basis_points);
    let (precedence_cut_rows, precedence_cut_diagnostics) =
        build_precedence_cumulative_prefix_cut_rows(artifact, &precedence_enforced_period_set)?;
    let (access_cut_rows, access_cut_family_diagnostics) =
        build_access_closure_capacity_prefix_cut_rows(artifact)?;
    let mut generated_cut_rows = precedence_cut_rows;
    generated_cut_rows.extend(access_cut_rows);
    let cut_diagnostics =
        merge_cut_diagnostics(precedence_cut_diagnostics, access_cut_family_diagnostics);

    for row in &artifact.constraints.rows {
        if row.kind == LpBzLpKernelConstraintKind::PrecedenceActivation {
            let period_index = row.period_index.ok_or_else(|| {
                MineError::validation(format!(
                    "LP/BZ kernel precedence row `{}` is missing period metadata",
                    row.row_id
                ))
            })?;
            if period_index >= artifact.period_count {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel precedence row `{}` references out-of-range period index {} (period_count={})",
                    row.row_id, period_index, artifact.period_count
                )));
            }
            if !precedence_enforced_period_set.contains(&period_index) {
                continue;
            }
        }
        if !row.rhs.is_finite() {
            return Err(MineError::validation(format!(
                "LP/BZ kernel solve row `{}` has non-finite RHS ({})",
                row.row_id, row.rhs
            )));
        }
        let row_terms = materialize_lp_row_terms(&row.row_id, &row.terms, &lp_variables)?;

        if row_terms.is_empty() {
            let feasible = match row.sense {
                LpBzLpKernelConstraintSense::LessEqual => row.rhs >= -EPSILON,
                LpBzLpKernelConstraintSense::GreaterEqual => row.rhs <= EPSILON,
            };
            if !feasible {
                return Ok(LpBzLpSolveArtifact {
                    solver_label: "minilp".to_owned(),
                    solve_status: LpBzLpSolveStatus::Infeasible,
                    discounted_objective_bound: None,
                    variable_count: lp_variables.len(),
                    active_variable_count: 0,
                    min_positive_variable_value: None,
                    max_variable_value: None,
                    precedence_diagnostics: precedence_diagnostics.clone(),
                    cut_diagnostics: cut_diagnostics.clone(),
                    limitations: vec![
                        "Kernel row set contains at least one empty-but-infeasible row; LP solve stops before simplex.".to_owned(),
                    ],
                });
            }
            continue;
        }

        let op = match row.sense {
            LpBzLpKernelConstraintSense::LessEqual => ComparisonOp::Le,
            LpBzLpKernelConstraintSense::GreaterEqual => ComparisonOp::Ge,
        };
        lp_problem.add_constraint(&row_terms, op, row.rhs);
    }
    for cut_row in &generated_cut_rows {
        let row_terms = materialize_lp_row_terms(&cut_row.row_id, &cut_row.terms, &lp_variables)?;
        if row_terms.is_empty() {
            continue;
        }
        lp_problem.add_constraint(&row_terms, ComparisonOp::Le, cut_row.rhs);
    }

    let solution = match lp_problem.solve() {
        Ok(solution) => solution,
        Err(minilp::Error::Infeasible) => {
            return Ok(LpBzLpSolveArtifact {
                solver_label: "minilp".to_owned(),
                solve_status: LpBzLpSolveStatus::Infeasible,
                discounted_objective_bound: None,
                variable_count: lp_variables.len(),
                active_variable_count: 0,
                min_positive_variable_value: None,
                max_variable_value: None,
                precedence_diagnostics: precedence_diagnostics.clone(),
                cut_diagnostics: cut_diagnostics.clone(),
                limitations: vec![
                    "Native LP kernel solve reported infeasible constraints on the in-harness LP model.".to_owned(),
                ],
            });
        }
        Err(minilp::Error::Unbounded) => {
            return Ok(LpBzLpSolveArtifact {
                solver_label: "minilp".to_owned(),
                solve_status: LpBzLpSolveStatus::Unbounded,
                discounted_objective_bound: None,
                variable_count: lp_variables.len(),
                active_variable_count: 0,
                min_positive_variable_value: None,
                max_variable_value: None,
                precedence_diagnostics: precedence_diagnostics.clone(),
                cut_diagnostics: cut_diagnostics.clone(),
                limitations: vec![
                    "Native LP kernel solve reported an unbounded objective on the in-harness LP model.".to_owned(),
                ],
            });
        }
    };

    let mut active_variable_count = 0usize;
    let mut min_positive_variable_value: Option<f64> = None;
    let mut max_variable_value: Option<f64> = None;
    for variable in &lp_variables {
        let value = solution[*variable];
        if !value.is_finite() {
            return Err(MineError::validation(
                "LP/BZ kernel solve produced a non-finite variable value".to_owned(),
            ));
        }
        if value > EPSILON {
            active_variable_count += 1;
            min_positive_variable_value = Some(match min_positive_variable_value {
                Some(current) => current.min(value),
                None => value,
            });
        }
        max_variable_value = Some(match max_variable_value {
            Some(current) => current.max(value),
            None => value,
        });
    }

    let objective = solution.objective();
    if !objective.is_finite() {
        return Err(MineError::validation(format!(
            "LP/BZ kernel solve produced a non-finite objective ({objective})"
        )));
    }

    Ok(LpBzLpSolveArtifact {
        solver_label: "minilp".to_owned(),
        solve_status: LpBzLpSolveStatus::Optimal,
        discounted_objective_bound: Some(objective),
        variable_count: lp_variables.len(),
        active_variable_count,
        min_positive_variable_value,
        max_variable_value,
        precedence_diagnostics: precedence_diagnostics.clone(),
        cut_diagnostics: cut_diagnostics.clone(),
        limitations: vec![match precedence_diagnostics.strategy {
            LpBzPrecedenceEnforcementStrategy::None => format!(
                "Native LP solve uses an in-harness relaxed kernel with capacity+activation rows only because the artifact contains no precedence rows; cut strategy `{}` generated {} rows (applied {}, skipped {}).",
                lp_bz_cut_strategy_label(cut_diagnostics.strategy),
                cut_diagnostics.total_generated_row_count,
                cut_diagnostics.total_applied_row_count,
                cut_diagnostics.total_skipped_row_count
            ),
            LpBzPrecedenceEnforcementStrategy::FullPerPeriod => format!(
                "Native LP solve enforces deterministic full per-period precedence rows with coverage {} ({}): enforced {}/{} (skipped {}) across periods {:?}. Deterministic cut strategy `{}` generated {} rows (applied {}, skipped {}) with families {:?}.",
                lp_bz_precedence_coverage_completeness_label(
                    precedence_diagnostics.coverage_completeness
                ),
                precedence_coverage_label,
                precedence_diagnostics.enforced_precedence_rows,
                precedence_diagnostics.total_precedence_rows,
                precedence_diagnostics.skipped_precedence_rows,
                enforced_period_labels,
                lp_bz_cut_strategy_label(cut_diagnostics.strategy),
                cut_diagnostics.total_generated_row_count,
                cut_diagnostics.total_applied_row_count,
                cut_diagnostics.total_skipped_row_count,
                cut_diagnostics
                    .families
                    .iter()
                    .map(|family| family.family_label.clone())
                    .collect::<Vec<_>>()
            ),
            LpBzPrecedenceEnforcementStrategy::HybridCheckpoint => format!(
                "Native LP solve enforces deterministic hybrid precedence checkpoints with row budget {} and coverage {} ({}): enforced {}/{} (skipped {}) across periods {:?}; skipped periods {:?}. Deterministic cut strategy `{}` generated {} rows (applied {}, skipped {}) with families {:?}.",
                precedence_diagnostics.max_enforced_precedence_rows,
                lp_bz_precedence_coverage_completeness_label(
                    precedence_diagnostics.coverage_completeness
                ),
                precedence_coverage_label,
                precedence_diagnostics.enforced_precedence_rows,
                precedence_diagnostics.total_precedence_rows,
                precedence_diagnostics.skipped_precedence_rows,
                enforced_period_labels,
                skipped_period_labels,
                lp_bz_cut_strategy_label(cut_diagnostics.strategy),
                cut_diagnostics.total_generated_row_count,
                cut_diagnostics.total_applied_row_count,
                cut_diagnostics.total_skipped_row_count,
                cut_diagnostics
                    .families
                    .iter()
                    .map(|family| family.family_label.clone())
                    .collect::<Vec<_>>()
            ),
        }],
    })
}

#[derive(Debug, Clone, PartialEq)]
struct GeneratedLpCutRow {
    row_id: String,
    rhs: f64,
    terms: Vec<LpBzLpKernelConstraintTerm>,
}

fn materialize_lp_row_terms(
    row_id: &str,
    terms: &[LpBzLpKernelConstraintTerm],
    lp_variables: &[minilp::Variable],
) -> Result<Vec<(minilp::Variable, f64)>, MineError> {
    let mut row_terms = Vec::<(minilp::Variable, f64)>::new();
    for term in terms {
        if term.variable_index >= lp_variables.len() {
            return Err(MineError::validation(format!(
                "LP/BZ kernel solve row `{row_id}` references out-of-range variable index {} (variable_count={})",
                term.variable_index,
                lp_variables.len()
            )));
        }
        if !term.coefficient.is_finite() {
            return Err(MineError::validation(format!(
                "LP/BZ kernel solve row `{row_id}` has non-finite coefficient for variable {} ({})",
                term.variable_index, term.coefficient
            )));
        }
        if term.coefficient.abs() <= EPSILON {
            continue;
        }
        row_terms.push((lp_variables[term.variable_index], term.coefficient));
    }
    Ok(row_terms)
}

fn build_precedence_cumulative_prefix_cut_rows(
    artifact: &LpBzLpKernelArtifact,
    enforced_period_indices: &BTreeSet<usize>,
) -> Result<(Vec<GeneratedLpCutRow>, LpBzCutSolveDiagnostics), MineError> {
    if enforced_period_indices.is_empty() {
        return Ok((
            Vec::new(),
            LpBzCutSolveDiagnostics {
                strategy: LpBzCutTighteningStrategy::None,
                total_generated_row_count: 0,
                total_applied_row_count: 0,
                total_skipped_row_count: 0,
                families: Vec::new(),
            },
        ));
    }

    let mut variable_indices_by_unit_period = BTreeMap::<(String, usize), Vec<usize>>::new();
    for variable_entry in &artifact.variable_index.entries {
        if variable_entry.key.period_index >= artifact.period_count {
            return Err(MineError::validation(format!(
                "LP/BZ cut build found variable {} with out-of-range period index {} (period_count={})",
                variable_entry.variable_index,
                variable_entry.key.period_index,
                artifact.period_count
            )));
        }
        variable_indices_by_unit_period
            .entry((
                variable_entry.key.unit_id.clone(),
                variable_entry.key.period_index,
            ))
            .or_default()
            .push(variable_entry.variable_index);
    }

    let mut precedence_row_targets = BTreeSet::<(String, String, usize)>::new();
    let mut skipped_precedence_rows = 0usize;
    let mut eligible_precedence_rows = 0usize;
    for row in &artifact.constraints.rows {
        if row.kind != LpBzLpKernelConstraintKind::PrecedenceActivation {
            continue;
        }
        let period_index = row.period_index.ok_or_else(|| {
            MineError::validation(format!(
                "LP/BZ cut build precedence row `{}` is missing period metadata",
                row.row_id
            ))
        })?;
        if period_index >= artifact.period_count {
            return Err(MineError::validation(format!(
                "LP/BZ cut build precedence row `{}` references out-of-range period index {} (period_count={})",
                row.row_id, period_index, artifact.period_count
            )));
        }
        if !enforced_period_indices.contains(&period_index) {
            skipped_precedence_rows += 1;
            continue;
        }
        let Some(predecessor_unit_id) = row.predecessor_unit_id.as_ref() else {
            skipped_precedence_rows += 1;
            continue;
        };
        let Some(successor_unit_id) = row.successor_unit_id.as_ref() else {
            skipped_precedence_rows += 1;
            continue;
        };
        eligible_precedence_rows += 1;
        precedence_row_targets.insert((
            predecessor_unit_id.clone(),
            successor_unit_id.clone(),
            period_index,
        ));
    }
    skipped_precedence_rows +=
        eligible_precedence_rows.saturating_sub(precedence_row_targets.len());
    if precedence_row_targets.is_empty() {
        return Ok((
            Vec::new(),
            LpBzCutSolveDiagnostics {
                strategy: LpBzCutTighteningStrategy::None,
                total_generated_row_count: 0,
                total_applied_row_count: 0,
                total_skipped_row_count: skipped_precedence_rows,
                families: vec![LpBzCutFamilySolveDiagnostics {
                    family_label: "precedence_cumulative_prefix".to_owned(),
                    generated_row_count: 0,
                    applied_row_count: 0,
                    skipped_row_count: skipped_precedence_rows,
                }],
            },
        ));
    }

    let mut generated_cut_rows = Vec::<GeneratedLpCutRow>::new();
    for (predecessor_unit_id, successor_unit_id, period_index) in precedence_row_targets {
        let mut terms = Vec::<LpBzLpKernelConstraintTerm>::new();
        for prefix_period in 0..=period_index {
            let successor_period_indices = variable_indices_by_unit_period
                .get(&(successor_unit_id.clone(), prefix_period))
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "LP/BZ cut build cannot find successor variables for unit `{successor_unit_id}` period {prefix_period}"
                    ))
                })?;
            terms.extend(successor_period_indices.iter().map(|variable_index| {
                LpBzLpKernelConstraintTerm {
                    variable_index: *variable_index,
                    coefficient: 1.0,
                }
            }));
            let predecessor_period_indices = variable_indices_by_unit_period
                .get(&(predecessor_unit_id.clone(), prefix_period))
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "LP/BZ cut build cannot find predecessor variables for unit `{predecessor_unit_id}` period {prefix_period}"
                    ))
                })?;
            terms.extend(predecessor_period_indices.iter().map(|variable_index| {
                LpBzLpKernelConstraintTerm {
                    variable_index: *variable_index,
                    coefficient: -1.0,
                }
            }));
        }
        generated_cut_rows.push(GeneratedLpCutRow {
            row_id: format!(
                "cut_precedence_cumulative_prefix__{}__{}__p{:02}",
                predecessor_unit_id,
                successor_unit_id,
                period_index + 1
            ),
            rhs: 0.0,
            terms,
        });
    }

    let generated_row_count = generated_cut_rows.len();
    Ok((
        generated_cut_rows,
        LpBzCutSolveDiagnostics {
            strategy: if generated_row_count == 0 {
                LpBzCutTighteningStrategy::None
            } else {
                LpBzCutTighteningStrategy::PrecedenceCumulativePrefix
            },
            total_generated_row_count: generated_row_count,
            total_applied_row_count: generated_row_count,
            total_skipped_row_count: skipped_precedence_rows,
            families: vec![LpBzCutFamilySolveDiagnostics {
                family_label: "precedence_cumulative_prefix".to_owned(),
                generated_row_count,
                applied_row_count: generated_row_count,
                skipped_row_count: skipped_precedence_rows,
            }],
        },
    ))
}

fn build_access_closure_capacity_prefix_cut_rows(
    artifact: &LpBzLpKernelArtifact,
) -> Result<(Vec<GeneratedLpCutRow>, LpBzCutFamilySolveDiagnostics), MineError> {
    let mut prefix_capacity_by_resource = BTreeMap::<String, Vec<f64>>::new();
    for row in &artifact.constraints.rows {
        if row.kind != LpBzLpKernelConstraintKind::CapacityUpper {
            continue;
        }
        let Some(period_index) = row.period_index else {
            return Err(MineError::validation(format!(
                "LP/BZ access cut build capacity row `{}` is missing period metadata",
                row.row_id
            )));
        };
        let Some(resource_id) = row.resource_id.as_ref() else {
            return Err(MineError::validation(format!(
                "LP/BZ access cut build capacity row `{}` is missing resource metadata",
                row.row_id
            )));
        };
        if period_index >= artifact.period_count {
            return Err(MineError::validation(format!(
                "LP/BZ access cut build capacity row `{}` references out-of-range period index {} (period_count={})",
                row.row_id, period_index, artifact.period_count
            )));
        }
        let prefix = prefix_capacity_by_resource
            .entry(resource_id.clone())
            .or_insert_with(|| vec![0.0; artifact.period_count]);
        prefix[period_index] += row.rhs;
    }
    for prefix in prefix_capacity_by_resource.values_mut() {
        let mut running_total = 0.0;
        for value in prefix {
            running_total += *value;
            *value = running_total;
        }
    }

    let mut variable_indices_by_unit_period = BTreeMap::<(String, usize), Vec<usize>>::new();
    for variable_entry in &artifact.variable_index.entries {
        if variable_entry.key.period_index >= artifact.period_count {
            return Err(MineError::validation(format!(
                "LP/BZ access cut build found variable {} with out-of-range period index {} (period_count={})",
                variable_entry.variable_index,
                variable_entry.key.period_index,
                artifact.period_count
            )));
        }
        variable_indices_by_unit_period
            .entry((
                variable_entry.key.unit_id.clone(),
                variable_entry.key.period_index,
            ))
            .or_default()
            .push(variable_entry.variable_index);
    }

    let mut generated_cut_rows = Vec::<GeneratedLpCutRow>::new();
    let mut generated_row_count = 0usize;
    let mut skipped_row_count = 0usize;
    for unit_profile in &artifact.access.unit_profiles {
        if unit_profile.transitive_predecessor_count == 0
            || unit_profile.closure_resources.is_empty()
        {
            continue;
        }
        for period_index in 0..artifact.period_count {
            generated_row_count += 1;
            let mut strongest_resource_cut: Option<(&str, f64)> = None;
            for closure_resource in &unit_profile.closure_resources {
                let Some(prefix_capacity) = prefix_capacity_by_resource
                    .get(&closure_resource.resource_id)
                    .and_then(|prefix| prefix.get(period_index))
                    .copied()
                else {
                    continue;
                };
                if !prefix_capacity.is_finite()
                    || !closure_resource.minimum_total_requirement.is_finite()
                    || closure_resource.minimum_total_requirement <= EPSILON
                {
                    continue;
                }
                let rhs = (prefix_capacity / closure_resource.minimum_total_requirement).min(1.0);
                strongest_resource_cut = match strongest_resource_cut {
                    Some((current_resource_id, current_rhs))
                        if rhs > current_rhs + EPSILON
                            || ((rhs - current_rhs).abs() <= EPSILON
                                && closure_resource.resource_id.as_str()
                                    >= current_resource_id) =>
                    {
                        Some((current_resource_id, current_rhs))
                    }
                    _ => Some((closure_resource.resource_id.as_str(), rhs)),
                };
            }

            let Some((resource_id, rhs)) = strongest_resource_cut else {
                skipped_row_count += 1;
                continue;
            };
            if rhs >= 1.0 - EPSILON {
                skipped_row_count += 1;
                continue;
            }

            let mut terms = Vec::<LpBzLpKernelConstraintTerm>::new();
            for prefix_period in 0..=period_index {
                let period_indices = variable_indices_by_unit_period
                    .get(&(unit_profile.unit_id.clone(), prefix_period))
                    .ok_or_else(|| {
                        MineError::validation(format!(
                            "LP/BZ access cut build cannot find variables for unit `{}` period {}",
                            unit_profile.unit_id, prefix_period
                        ))
                    })?;
                terms.extend(period_indices.iter().map(|variable_index| {
                    LpBzLpKernelConstraintTerm {
                        variable_index: *variable_index,
                        coefficient: 1.0,
                    }
                }));
            }
            generated_cut_rows.push(GeneratedLpCutRow {
                row_id: format!(
                    "cut_access_closure_capacity_prefix__{}__{}__p{:02}",
                    unit_profile.unit_id,
                    resource_id,
                    period_index + 1
                ),
                rhs,
                terms,
            });
        }
    }

    Ok((
        generated_cut_rows,
        LpBzCutFamilySolveDiagnostics {
            family_label: "access_closure_capacity_prefix".to_owned(),
            generated_row_count,
            applied_row_count: generated_row_count.saturating_sub(skipped_row_count),
            skipped_row_count,
        },
    ))
}

fn merge_cut_diagnostics(
    precedence_diagnostics: LpBzCutSolveDiagnostics,
    access_family_diagnostics: LpBzCutFamilySolveDiagnostics,
) -> LpBzCutSolveDiagnostics {
    let base_family_count = precedence_diagnostics.families.len();
    let precedence_applied = precedence_diagnostics.total_applied_row_count > 0;
    let mut families = precedence_diagnostics.families;
    if access_family_diagnostics.generated_row_count > 0
        || access_family_diagnostics.applied_row_count > 0
        || access_family_diagnostics.skipped_row_count > 0
    {
        families.push(access_family_diagnostics);
    }

    let total_generated_row_count = precedence_diagnostics.total_generated_row_count
        + families
            .iter()
            .skip(base_family_count)
            .map(|family| family.generated_row_count)
            .sum::<usize>();
    let total_applied_row_count = precedence_diagnostics.total_applied_row_count
        + families
            .iter()
            .skip(base_family_count)
            .map(|family| family.applied_row_count)
            .sum::<usize>();
    let total_skipped_row_count = precedence_diagnostics.total_skipped_row_count
        + families
            .iter()
            .skip(base_family_count)
            .map(|family| family.skipped_row_count)
            .sum::<usize>();
    let access_applied = families.iter().any(|family| {
        family.family_label == "access_closure_capacity_prefix" && family.applied_row_count > 0
    });

    let strategy = match (precedence_applied, access_applied) {
        (false, false) => LpBzCutTighteningStrategy::None,
        (true, false) => LpBzCutTighteningStrategy::PrecedenceCumulativePrefix,
        (false, true) => LpBzCutTighteningStrategy::AccessClosureCapacityPrefix,
        (true, true) => {
            LpBzCutTighteningStrategy::PrecedenceCumulativePrefixAndAccessClosureCapacityPrefix
        }
    };

    LpBzCutSolveDiagnostics {
        strategy,
        total_generated_row_count,
        total_applied_row_count,
        total_skipped_row_count,
        families,
    }
}

fn lp_bz_precedence_coverage_completeness(
    total_precedence_rows: usize,
    enforced_precedence_rows: usize,
    skipped_precedence_rows: usize,
) -> LpBzPrecedenceCoverageCompleteness {
    if total_precedence_rows == 0 {
        return LpBzPrecedenceCoverageCompleteness::NotApplicable;
    }
    if skipped_precedence_rows == 0 && enforced_precedence_rows >= total_precedence_rows {
        return LpBzPrecedenceCoverageCompleteness::Complete;
    }
    LpBzPrecedenceCoverageCompleteness::Partial
}

fn lp_bz_precedence_coverage_basis_points(
    total_precedence_rows: usize,
    enforced_precedence_rows: usize,
) -> Option<u16> {
    if total_precedence_rows == 0 {
        return None;
    }
    let bounded_enforced_rows = enforced_precedence_rows.min(total_precedence_rows) as u128;
    let total_precedence_rows = total_precedence_rows as u128;
    Some(
        (((bounded_enforced_rows * 10_000) + (total_precedence_rows / 2)) / total_precedence_rows)
            as u16,
    )
}

fn lp_bz_precedence_coverage_label(coverage_basis_points: Option<u16>) -> String {
    match coverage_basis_points {
        Some(coverage_basis_points) => format!("{:.2}%", f64::from(coverage_basis_points) / 100.0),
        None => "n/a".to_owned(),
    }
}

fn lp_bz_precedence_coverage_completeness_label(
    completeness: LpBzPrecedenceCoverageCompleteness,
) -> &'static str {
    match completeness {
        LpBzPrecedenceCoverageCompleteness::NotApplicable => "not_applicable",
        LpBzPrecedenceCoverageCompleteness::Partial => "partial",
        LpBzPrecedenceCoverageCompleteness::Complete => "complete",
    }
}

fn lp_bz_cut_strategy_label(strategy: LpBzCutTighteningStrategy) -> &'static str {
    match strategy {
        LpBzCutTighteningStrategy::None => "none",
        LpBzCutTighteningStrategy::PrecedenceCumulativePrefix => "precedence_cumulative_prefix",
        LpBzCutTighteningStrategy::AccessClosureCapacityPrefix => "access_closure_capacity_prefix",
        LpBzCutTighteningStrategy::PrecedenceCumulativePrefixAndAccessClosureCapacityPrefix => {
            "precedence_cumulative_prefix+access_closure_capacity_prefix"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrecedenceEnforcementPlan {
    strategy: LpBzPrecedenceEnforcementStrategy,
    max_enforced_precedence_rows: usize,
    enforced_period_indices: Vec<usize>,
}

fn build_precedence_enforcement_plan(
    period_count: usize,
    total_precedence_rows: usize,
) -> PrecedenceEnforcementPlan {
    if period_count == 0 || total_precedence_rows == 0 {
        return PrecedenceEnforcementPlan {
            strategy: LpBzPrecedenceEnforcementStrategy::None,
            max_enforced_precedence_rows: 0,
            enforced_period_indices: Vec::new(),
        };
    }

    if total_precedence_rows <= PRECEDENCE_FULL_ENFORCEMENT_ROW_LIMIT {
        return PrecedenceEnforcementPlan {
            strategy: LpBzPrecedenceEnforcementStrategy::FullPerPeriod,
            max_enforced_precedence_rows: total_precedence_rows,
            enforced_period_indices: (0..period_count).collect(),
        };
    }

    let precedence_pair_count = total_precedence_rows.div_ceil(period_count);
    let max_periods_for_row_budget =
        (PRECEDENCE_HYBRID_TARGET_ROW_LIMIT / precedence_pair_count).max(2);
    let target_checkpoint_count = period_count
        .min(PRECEDENCE_HYBRID_MAX_PERIOD_COUNT.max(2))
        .min(max_periods_for_row_budget.max(2));
    if target_checkpoint_count >= period_count {
        return PrecedenceEnforcementPlan {
            strategy: LpBzPrecedenceEnforcementStrategy::FullPerPeriod,
            max_enforced_precedence_rows: total_precedence_rows,
            enforced_period_indices: (0..period_count).collect(),
        };
    }

    let enforced_period_indices =
        evenly_spaced_period_indices(period_count, target_checkpoint_count);
    PrecedenceEnforcementPlan {
        strategy: LpBzPrecedenceEnforcementStrategy::HybridCheckpoint,
        max_enforced_precedence_rows: precedence_pair_count
            .saturating_mul(enforced_period_indices.len()),
        enforced_period_indices,
    }
}

fn evenly_spaced_period_indices(period_count: usize, checkpoint_count: usize) -> Vec<usize> {
    if period_count == 0 || checkpoint_count == 0 {
        return Vec::new();
    }
    if period_count == 1 || checkpoint_count == 1 {
        return vec![0];
    }

    let bounded_checkpoint_count = checkpoint_count.min(period_count).max(2);
    let denominator = bounded_checkpoint_count - 1;
    let last_period_index = period_count - 1;
    let mut checkpoints = BTreeSet::new();
    for checkpoint_index in 0..bounded_checkpoint_count {
        let numerator = checkpoint_index * last_period_index;
        let rounded_index = (numerator + denominator / 2) / denominator;
        checkpoints.insert(rounded_index);
    }
    checkpoints.into_iter().collect()
}

fn build_variable_index(
    scheduling_problem: &SchedulingProblem,
    units_by_id: &BTreeMap<String, &mine_sdk::SchedulingUnit>,
    unit_destination_ids: &BTreeMap<String, Vec<String>>,
) -> Result<
    (
        LpBzLpKernelVariableIndexArtifact,
        BTreeMap<(String, String, usize), usize>,
        BTreeMap<String, Vec<usize>>,
        BTreeMap<(String, usize), Vec<usize>>,
    ),
    MineError,
> {
    let mut entries = Vec::new();
    let mut index_by_key = BTreeMap::<(String, String, usize), usize>::new();
    let mut indices_by_unit = BTreeMap::<String, Vec<usize>>::new();
    let mut indices_by_unit_period = BTreeMap::<(String, usize), Vec<usize>>::new();

    for unit_id in units_by_id.keys() {
        let destinations = unit_destination_ids.get(unit_id).ok_or_else(|| {
            MineError::validation(format!(
                "LP/BZ kernel variable map is missing destination domain for unit `{unit_id}`"
            ))
        })?;
        for destination_id in destinations {
            for (period_index, period) in scheduling_problem.periods().iter().enumerate() {
                let variable_index = entries.len();
                let key = LpBzLpKernelVariableKey {
                    unit_id: unit_id.clone(),
                    destination_id: destination_id.clone(),
                    period_index,
                };
                entries.push(LpBzLpKernelVariableEntry {
                    variable_index,
                    key: key.clone(),
                    period_label: period.period_label().to_owned(),
                });
                index_by_key.insert(
                    (
                        key.unit_id.clone(),
                        key.destination_id.clone(),
                        key.period_index,
                    ),
                    variable_index,
                );
                indices_by_unit
                    .entry(key.unit_id.clone())
                    .or_default()
                    .push(variable_index);
                indices_by_unit_period
                    .entry((key.unit_id.clone(), key.period_index))
                    .or_default()
                    .push(variable_index);
            }
        }
    }

    Ok((
        LpBzLpKernelVariableIndexArtifact {
            variable_count: entries.len(),
            entries,
        },
        index_by_key,
        indices_by_unit,
        indices_by_unit_period,
    ))
}

fn build_objective_artifact(
    scheduling_problem: &SchedulingProblem,
    variable_entries: &[LpBzLpKernelVariableEntry],
    units_by_id: &BTreeMap<String, &mine_sdk::SchedulingUnit>,
    unit_destination_ids: &BTreeMap<String, Vec<String>>,
) -> Result<LpBzLpKernelObjectiveArtifact, MineError> {
    let mut specific_objective = BTreeMap::<(String, String), f64>::new();
    let mut generic_objective = BTreeMap::<String, f64>::new();
    let mut units_with_specific_objective = BTreeSet::<String>::new();
    for objective_term in scheduling_problem.objective_terms() {
        let unit_id = objective_term.unit_id().as_str().to_owned();
        if !units_by_id.contains_key(&unit_id) {
            return Err(MineError::validation(format!(
                "LP/BZ kernel objective term references unknown unit `{unit_id}`"
            )));
        }
        if let Some(destination_id) = objective_term.destination_id() {
            let destination_id = destination_id.as_str().to_owned();
            if generic_objective.contains_key(&unit_id) {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel objective for unit `{unit_id}` mixes generic and destination-specific terms"
                )));
            }
            let destinations = unit_destination_ids.get(&unit_id).ok_or_else(|| {
                MineError::validation(format!(
                    "LP/BZ kernel objective lookup has no destination domain for unit `{unit_id}`"
                ))
            })?;
            if !destinations.contains(&destination_id) {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel objective term for unit `{unit_id}` references destination `{destination_id}` outside unit destination domain"
                )));
            }
            if specific_objective
                .insert((unit_id.clone(), destination_id), objective_term.value())
                .is_some()
            {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel objective contains duplicate destination-specific coefficient for unit `{unit_id}`"
                )));
            }
            units_with_specific_objective.insert(unit_id);
            continue;
        }

        if units_with_specific_objective.contains(&unit_id) {
            return Err(MineError::validation(format!(
                "LP/BZ kernel objective for unit `{unit_id}` mixes destination-specific and generic terms"
            )));
        }
        if generic_objective
            .insert(unit_id.clone(), objective_term.value())
            .is_some()
        {
            return Err(MineError::validation(format!(
                "LP/BZ kernel objective contains duplicate generic coefficient for unit `{unit_id}`"
            )));
        }
    }

    let mut coefficients = Vec::with_capacity(variable_entries.len());
    let mut non_zero_coefficient_count = 0usize;
    for variable in variable_entries {
        let undiscounted_value = specific_objective
            .get(&(
                variable.key.unit_id.clone(),
                variable.key.destination_id.clone(),
            ))
            .copied()
            .or_else(|| generic_objective.get(&variable.key.unit_id).copied())
            .unwrap_or(0.0);
        let discount_factor = discount_factor(
            scheduling_problem.discount_rate(),
            variable.key.period_index,
        )?;
        let coefficient = undiscounted_value / discount_factor;
        if coefficient.abs() > EPSILON {
            non_zero_coefficient_count += 1;
        }
        coefficients.push(LpBzLpKernelObjectiveCoefficient {
            variable_index: variable.variable_index,
            coefficient,
            undiscounted_value,
            discount_factor,
        });
    }

    Ok(LpBzLpKernelObjectiveArtifact {
        summary: LpBzLpKernelObjectiveSummary {
            coefficient_count: coefficients.len(),
            non_zero_coefficient_count,
        },
        coefficients,
    })
}

fn build_constraint_artifact(
    scheduling_problem: &SchedulingProblem,
    units_by_id: &BTreeMap<String, &mine_sdk::SchedulingUnit>,
    unit_destination_ids: &BTreeMap<String, Vec<String>>,
    variable_index_by_key: &BTreeMap<(String, String, usize), usize>,
    variable_indices_by_unit: &BTreeMap<String, Vec<usize>>,
    variable_indices_by_unit_period: &BTreeMap<(String, usize), Vec<usize>>,
    resource_specific: &BTreeMap<(String, String, String), f64>,
    resource_generic: &BTreeMap<(String, String), f64>,
) -> Result<LpBzLpKernelConstraintArtifact, MineError> {
    let mut rows = Vec::<LpBzLpKernelConstraintRow>::new();
    let mut capacity_row_count = 0usize;
    let mut activation_row_count = 0usize;
    let mut precedence_row_count = 0usize;

    for (period_index, period) in scheduling_problem.periods().iter().enumerate() {
        let mut resource_bounds = period.resource_bounds().iter().collect::<Vec<_>>();
        resource_bounds.sort_by(|left, right| {
            left.resource_id()
                .as_str()
                .cmp(right.resource_id().as_str())
        });
        for bound in resource_bounds {
            if let Some(max_total) = bound.max_total() {
                let terms = build_capacity_row_terms(
                    period_index,
                    bound.resource_id().as_str(),
                    units_by_id,
                    unit_destination_ids,
                    variable_index_by_key,
                    resource_specific,
                    resource_generic,
                )?;
                rows.push(LpBzLpKernelConstraintRow {
                    row_index: rows.len(),
                    row_id: format!(
                        "capacity_upper__{}__{}",
                        period.period_label(),
                        bound.resource_id()
                    ),
                    kind: LpBzLpKernelConstraintKind::CapacityUpper,
                    sense: LpBzLpKernelConstraintSense::LessEqual,
                    rhs: max_total,
                    period_index: Some(period_index),
                    period_label: Some(period.period_label().to_owned()),
                    resource_id: Some(bound.resource_id().as_str().to_owned()),
                    unit_id: None,
                    predecessor_unit_id: None,
                    successor_unit_id: None,
                    terms,
                });
                capacity_row_count += 1;
            }
            if let Some(min_total) = bound.min_total() {
                let terms = build_capacity_row_terms(
                    period_index,
                    bound.resource_id().as_str(),
                    units_by_id,
                    unit_destination_ids,
                    variable_index_by_key,
                    resource_specific,
                    resource_generic,
                )?;
                rows.push(LpBzLpKernelConstraintRow {
                    row_index: rows.len(),
                    row_id: format!(
                        "capacity_lower__{}__{}",
                        period.period_label(),
                        bound.resource_id()
                    ),
                    kind: LpBzLpKernelConstraintKind::CapacityLower,
                    sense: LpBzLpKernelConstraintSense::GreaterEqual,
                    rhs: min_total,
                    period_index: Some(period_index),
                    period_label: Some(period.period_label().to_owned()),
                    resource_id: Some(bound.resource_id().as_str().to_owned()),
                    unit_id: None,
                    predecessor_unit_id: None,
                    successor_unit_id: None,
                    terms,
                });
                capacity_row_count += 1;
            }
        }
    }

    for unit_id in units_by_id.keys() {
        let terms = variable_indices_by_unit
            .get(unit_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|variable_index| LpBzLpKernelConstraintTerm {
                variable_index,
                coefficient: 1.0,
            })
            .collect::<Vec<_>>();
        rows.push(LpBzLpKernelConstraintRow {
            row_index: rows.len(),
            row_id: format!("activation_upper__{unit_id}"),
            kind: LpBzLpKernelConstraintKind::ActivationUpper,
            sense: LpBzLpKernelConstraintSense::LessEqual,
            rhs: 1.0,
            period_index: None,
            period_label: None,
            resource_id: None,
            unit_id: Some(unit_id.clone()),
            predecessor_unit_id: None,
            successor_unit_id: None,
            terms,
        });
        activation_row_count += 1;
    }

    for (successor_unit_id, unit) in units_by_id {
        let mut predecessor_unit_ids = unit
            .predecessor_unit_ids()
            .iter()
            .map(|unit_id| unit_id.as_str().to_owned())
            .collect::<Vec<_>>();
        predecessor_unit_ids.sort();
        for predecessor_unit_id in predecessor_unit_ids {
            if !units_by_id.contains_key(&predecessor_unit_id) {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel precedence row references unknown predecessor unit `{predecessor_unit_id}` for successor `{successor_unit_id}`"
                )));
            }
            for period_index in 0..scheduling_problem.periods().len() {
                let mut terms = Vec::<LpBzLpKernelConstraintTerm>::new();
                let successor_period_indices = variable_indices_by_unit_period
                    .get(&(successor_unit_id.clone(), period_index))
                    .ok_or_else(|| {
                        MineError::validation(format!(
                            "LP/BZ kernel precedence row cannot find successor variables for unit `{successor_unit_id}` period {period_index}"
                        ))
                    })?;
                terms.extend(successor_period_indices.iter().map(|variable_index| {
                    LpBzLpKernelConstraintTerm {
                        variable_index: *variable_index,
                        coefficient: 1.0,
                    }
                }));

                for predecessor_period in 0..=period_index {
                    let predecessor_period_indices = variable_indices_by_unit_period
                        .get(&(predecessor_unit_id.clone(), predecessor_period))
                        .ok_or_else(|| {
                            MineError::validation(format!(
                                "LP/BZ kernel precedence row cannot find predecessor variables for unit `{predecessor_unit_id}` period {predecessor_period}"
                            ))
                        })?;
                    terms.extend(predecessor_period_indices.iter().map(|variable_index| {
                        LpBzLpKernelConstraintTerm {
                            variable_index: *variable_index,
                            coefficient: -1.0,
                        }
                    }));
                }

                rows.push(LpBzLpKernelConstraintRow {
                    row_index: rows.len(),
                    row_id: format!(
                        "precedence_activation__{}__{}__p{:02}",
                        predecessor_unit_id,
                        successor_unit_id,
                        period_index + 1
                    ),
                    kind: LpBzLpKernelConstraintKind::PrecedenceActivation,
                    sense: LpBzLpKernelConstraintSense::LessEqual,
                    rhs: 0.0,
                    period_index: Some(period_index),
                    period_label: Some(
                        scheduling_problem.periods()[period_index]
                            .period_label()
                            .to_owned(),
                    ),
                    resource_id: None,
                    unit_id: None,
                    predecessor_unit_id: Some(predecessor_unit_id.clone()),
                    successor_unit_id: Some(successor_unit_id.clone()),
                    terms,
                });
                precedence_row_count += 1;
            }
        }
    }

    Ok(LpBzLpKernelConstraintArtifact {
        summary: LpBzLpKernelConstraintSummary {
            row_count: rows.len(),
            capacity_row_count,
            activation_row_count,
            precedence_row_count,
        },
        rows,
    })
}

fn build_access_artifact(
    scheduling_problem: &SchedulingProblem,
    units_by_id: &BTreeMap<String, &mine_sdk::SchedulingUnit>,
    unit_destination_ids: &BTreeMap<String, Vec<String>>,
    resource_specific: &BTreeMap<(String, String, String), f64>,
    resource_generic: &BTreeMap<(String, String), f64>,
) -> Result<LpBzLpKernelAccessArtifact, MineError> {
    let minimum_unit_resource_requirements = build_minimum_unit_resource_requirements(
        units_by_id,
        unit_destination_ids,
        resource_specific,
        resource_generic,
    )?;
    let predecessor_ids_by_unit = scheduling_problem
        .units()
        .iter()
        .map(|unit| {
            (
                unit.unit_id().as_str().to_owned(),
                unit.predecessor_unit_ids()
                    .iter()
                    .map(|predecessor_id| predecessor_id.as_str().to_owned())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut closure_cache = BTreeMap::<String, BTreeSet<String>>::new();
    let mut active_stack = BTreeSet::<String>::new();
    let mut unit_profiles = Vec::<LpBzLpKernelAccessUnitProfile>::new();
    for unit in scheduling_problem.units() {
        let unit_id = unit.unit_id().as_str().to_owned();
        let closure = build_transitive_access_closure(
            &unit_id,
            &predecessor_ids_by_unit,
            &mut closure_cache,
            &mut active_stack,
        )?;
        let closure_resources = minimum_unit_resource_requirements
            .keys()
            .filter(|(closure_unit_id, _)| closure.contains(closure_unit_id))
            .map(|(_, resource_id)| resource_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|resource_id| {
                let mut minimum_total_requirement = 0.0;
                for closure_unit_id in &closure {
                    let requirement = minimum_unit_resource_requirements
                        .get(&(closure_unit_id.clone(), resource_id.clone()))
                        .copied()?;
                    minimum_total_requirement += requirement;
                }
                if minimum_total_requirement <= EPSILON {
                    return None;
                }
                Some(LpBzLpKernelAccessClosureResource {
                    resource_id,
                    minimum_total_requirement,
                })
            })
            .collect::<Vec<_>>();
        unit_profiles.push(LpBzLpKernelAccessUnitProfile {
            unit_id,
            bench: unit.bench(),
            shell_index: unit.shell_index(),
            direct_predecessor_count: unit.predecessor_unit_ids().len(),
            transitive_predecessor_count: closure.len().saturating_sub(1),
            closure_unit_count: closure.len(),
            closure_resources,
        });
    }
    unit_profiles.sort_by(|left, right| left.unit_id.cmp(&right.unit_id));
    Ok(LpBzLpKernelAccessArtifact {
        unit_profile_count: unit_profiles.len(),
        unit_profiles,
    })
}

fn build_minimum_unit_resource_requirements(
    units_by_id: &BTreeMap<String, &mine_sdk::SchedulingUnit>,
    unit_destination_ids: &BTreeMap<String, Vec<String>>,
    resource_specific: &BTreeMap<(String, String, String), f64>,
    resource_generic: &BTreeMap<(String, String), f64>,
) -> Result<BTreeMap<(String, String), f64>, MineError> {
    let resource_ids = resource_specific
        .keys()
        .map(|(_, resource_id, _)| resource_id.clone())
        .chain(
            resource_generic
                .keys()
                .map(|(_, resource_id)| resource_id.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut minimum_requirements = BTreeMap::<(String, String), f64>::new();
    for unit_id in units_by_id.keys() {
        let destinations = unit_destination_ids.get(unit_id).ok_or_else(|| {
            MineError::validation(format!(
                "LP/BZ access profile build has no destination domain for unit `{unit_id}`"
            ))
        })?;
        for resource_id in &resource_ids {
            let mut minimum_requirement: Option<f64> = resource_generic
                .get(&(unit_id.clone(), resource_id.clone()))
                .copied();
            for destination_id in destinations {
                if let Some(requirement) = resource_specific
                    .get(&(unit_id.clone(), resource_id.clone(), destination_id.clone()))
                    .copied()
                {
                    minimum_requirement = Some(match minimum_requirement {
                        Some(current_minimum) => current_minimum.min(requirement),
                        None => requirement,
                    });
                }
            }
            if let Some(minimum_requirement) = minimum_requirement
                && minimum_requirement > EPSILON
            {
                minimum_requirements
                    .insert((unit_id.clone(), resource_id.clone()), minimum_requirement);
            }
        }
    }
    Ok(minimum_requirements)
}

fn build_transitive_access_closure(
    unit_id: &str,
    predecessor_ids_by_unit: &BTreeMap<String, Vec<String>>,
    closure_cache: &mut BTreeMap<String, BTreeSet<String>>,
    active_stack: &mut BTreeSet<String>,
) -> Result<BTreeSet<String>, MineError> {
    if let Some(cached) = closure_cache.get(unit_id) {
        return Ok(cached.clone());
    }
    if !active_stack.insert(unit_id.to_owned()) {
        return Err(MineError::validation(format!(
            "LP/BZ access profile build found a precedence cycle involving unit `{unit_id}`"
        )));
    }

    let mut closure = BTreeSet::new();
    closure.insert(unit_id.to_owned());
    let predecessor_ids = predecessor_ids_by_unit.get(unit_id).ok_or_else(|| {
        MineError::validation(format!(
            "LP/BZ access profile build cannot find predecessor metadata for unit `{unit_id}`"
        ))
    })?;
    for predecessor_unit_id in predecessor_ids {
        let predecessor_closure = build_transitive_access_closure(
            predecessor_unit_id,
            predecessor_ids_by_unit,
            closure_cache,
            active_stack,
        )?;
        closure.extend(predecessor_closure);
    }
    active_stack.remove(unit_id);
    closure_cache.insert(unit_id.to_owned(), closure.clone());
    Ok(closure)
}

fn build_resource_requirement_lookups(
    scheduling_problem: &SchedulingProblem,
    units_by_id: &BTreeMap<String, &mine_sdk::SchedulingUnit>,
    unit_destination_ids: &BTreeMap<String, Vec<String>>,
) -> Result<
    (
        BTreeMap<(String, String, String), f64>,
        BTreeMap<(String, String), f64>,
    ),
    MineError,
> {
    let mut specific_requirement = BTreeMap::<(String, String, String), f64>::new();
    let mut generic_requirement = BTreeMap::<(String, String), f64>::new();
    let mut scoped_specific_pairs = BTreeSet::<(String, String)>::new();
    for requirement in scheduling_problem.resource_requirements() {
        let unit_id = requirement.unit_id().as_str().to_owned();
        let resource_id = requirement.resource_id().as_str().to_owned();
        if !units_by_id.contains_key(&unit_id) {
            return Err(MineError::validation(format!(
                "LP/BZ kernel resource requirement references unknown unit `{unit_id}`"
            )));
        }
        if let Some(destination_id) = requirement.destination_id() {
            let destination_id = destination_id.as_str().to_owned();
            if generic_requirement.contains_key(&(unit_id.clone(), resource_id.clone())) {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel resource requirement for unit `{unit_id}` resource `{resource_id}` mixes generic and destination-specific terms"
                )));
            }
            let destinations = unit_destination_ids.get(&unit_id).ok_or_else(|| {
                MineError::validation(format!(
                    "LP/BZ kernel resource lookup has no destination domain for unit `{unit_id}`"
                ))
            })?;
            if !destinations.contains(&destination_id) {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel resource requirement for unit `{unit_id}` resource `{resource_id}` references destination `{destination_id}` outside unit destination domain"
                )));
            }
            if specific_requirement
                .insert(
                    (unit_id.clone(), resource_id.clone(), destination_id),
                    requirement.amount(),
                )
                .is_some()
            {
                return Err(MineError::validation(format!(
                    "LP/BZ kernel contains duplicate destination-specific resource requirement for unit `{unit_id}` resource `{resource_id}`"
                )));
            }
            scoped_specific_pairs.insert((unit_id, resource_id));
            continue;
        }

        if scoped_specific_pairs.contains(&(unit_id.clone(), resource_id.clone())) {
            return Err(MineError::validation(format!(
                "LP/BZ kernel resource requirement for unit `{unit_id}` resource `{resource_id}` mixes destination-specific and generic terms"
            )));
        }
        if generic_requirement
            .insert((unit_id.clone(), resource_id.clone()), requirement.amount())
            .is_some()
        {
            return Err(MineError::validation(format!(
                "LP/BZ kernel contains duplicate generic resource requirement for unit `{unit_id}` resource `{resource_id}`"
            )));
        }
    }

    Ok((specific_requirement, generic_requirement))
}

fn build_capacity_row_terms(
    period_index: usize,
    resource_id: &str,
    units_by_id: &BTreeMap<String, &mine_sdk::SchedulingUnit>,
    unit_destination_ids: &BTreeMap<String, Vec<String>>,
    variable_index_by_key: &BTreeMap<(String, String, usize), usize>,
    resource_specific: &BTreeMap<(String, String, String), f64>,
    resource_generic: &BTreeMap<(String, String), f64>,
) -> Result<Vec<LpBzLpKernelConstraintTerm>, MineError> {
    let mut terms = Vec::<LpBzLpKernelConstraintTerm>::new();

    for unit_id in units_by_id.keys() {
        let destinations = unit_destination_ids.get(unit_id).ok_or_else(|| {
            MineError::validation(format!(
                "LP/BZ kernel capacity row build has no destination domain for unit `{unit_id}`"
            ))
        })?;
        for destination_id in destinations {
            let coefficient = resource_specific
                .get(&(
                    unit_id.clone(),
                    resource_id.to_owned(),
                    destination_id.clone(),
                ))
                .copied()
                .or_else(|| {
                    resource_generic
                        .get(&(unit_id.clone(), resource_id.to_owned()))
                        .copied()
                })
                .unwrap_or(0.0);
            if coefficient.abs() <= EPSILON {
                continue;
            }
            let variable_index = variable_index_by_key
                .get(&(unit_id.clone(), destination_id.clone(), period_index))
                .copied()
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "LP/BZ kernel capacity row build cannot find variable index for ({unit_id}, {destination_id}, period {period_index})"
                    ))
                })?;
            terms.push(LpBzLpKernelConstraintTerm {
                variable_index,
                coefficient,
            });
        }
    }

    Ok(terms)
}

fn discount_factor(discount_rate: f64, period_index: usize) -> Result<f64, MineError> {
    let base = 1.0 + discount_rate;
    if !base.is_finite() || base <= 0.0 {
        return Err(MineError::validation(format!(
            "LP/BZ kernel discount base must be finite and greater than zero (received {base})"
        )));
    }
    let exponent = i32::try_from(period_index).map_err(|_| {
        MineError::validation(format!(
            "LP/BZ kernel period index `{period_index}` is too large for discount exponent conversion"
        ))
    })?;
    let discount_factor = base.powi(exponent);
    if !discount_factor.is_finite() || discount_factor <= 0.0 {
        return Err(MineError::validation(format!(
            "LP/BZ kernel discount factor for period {period_index} is invalid ({discount_factor})"
        )));
    }
    Ok(discount_factor)
}
