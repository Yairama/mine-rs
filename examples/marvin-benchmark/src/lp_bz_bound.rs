use std::collections::BTreeMap;
use std::path::Path;

use mine_sdk::MineError;
use serde::Serialize;

use crate::marvin_support::{MarvinScheduleProblem, MarvinScheduleSolution};

#[derive(Debug, Clone, Serialize)]
pub struct LpBzInputArtifact {
    pub problem_normalization: LpBzProblemNormalization,
    pub precedence_units: LpBzPrecedenceUnits,
    pub lp_relaxation_source: LpBzRelaxationSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct LpBzProblemNormalization {
    pub period_count: usize,
    pub resource_constraint_count: usize,
    pub destination_count: usize,
    pub discount_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LpBzPrecedenceUnits {
    pub unit_count: usize,
    pub edge_count: usize,
    pub unit_granularity_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LpBzRelaxationSource {
    pub source_label: String,
    pub reference_artifact_path: String,
    pub objective_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LpBzBoundArtifact {
    pub bound_label: String,
    pub discounted_objective_bound: f64,
    pub period_count: usize,
    pub resource_constraint_count: usize,
    pub destination_count: usize,
    pub unit_count: usize,
    pub bound_strategy: String,
    pub lp_proxy_discounted_objective: f64,
    pub native_block_objective_upper_bound: f64,
    pub native_resource_density_upper_bound: Option<f64>,
    pub native_resource_knapsack_upper_bound: Option<f64>,
    pub discount_inverse_upper_bound: f64,
}

#[derive(Debug, Clone)]
pub struct LpBzBoundComputationArtifacts {
    pub lp_bz_inputs: LpBzInputArtifact,
    pub lp_bz_bound_artifact: LpBzBoundArtifact,
}

pub fn compute_lp_bz_bound_artifacts(
    problem: &MarvinScheduleProblem,
    lp_relaxation_solution: &MarvinScheduleSolution,
    lp_relaxation_reference_path: &Path,
    repo_root: &Path,
    precedence_unit_count: usize,
    precedence_edge_count: usize,
    unit_granularity_label: &str,
) -> Result<LpBzBoundComputationArtifacts, MineError> {
    if unit_granularity_label.trim().is_empty() {
        return Err(MineError::validation(
            "LP/BZ bound unit granularity label cannot be empty".to_owned(),
        ));
    }

    let objective_lookup = build_objective_lookup(problem)?;
    let discount_inverse_upper_bound =
        compute_discount_inverse_upper_bound(problem.discount_rate, problem.period_count)?;
    let lp_proxy_discounted_objective =
        compute_discounted_objective(problem, lp_relaxation_solution, &objective_lookup)?;
    let native_block_objective_upper_bound =
        compute_block_objective_upper_bound(&objective_lookup, discount_inverse_upper_bound)?;
    let native_resource_density_upper_bound =
        compute_resource_density_upper_bound(problem, &objective_lookup)?;
    let native_resource_knapsack_upper_bound = compute_resource_knapsack_upper_bound(
        problem,
        &objective_lookup,
        discount_inverse_upper_bound,
    )?;
    let native_resource_upper_bound = match (
        native_resource_density_upper_bound,
        native_resource_knapsack_upper_bound,
    ) {
        (Some(density), Some(knapsack)) => Some(density.min(knapsack)),
        (Some(density), None) => Some(density),
        (None, Some(knapsack)) => Some(knapsack),
        (None, None) => None,
    };
    let native_conservative_bound = native_resource_upper_bound
        .map(|resource_bound| resource_bound.min(native_block_objective_upper_bound))
        .unwrap_or(native_block_objective_upper_bound);
    let discounted_objective_bound = native_conservative_bound.max(lp_proxy_discounted_objective);

    Ok(LpBzBoundComputationArtifacts {
        lp_bz_inputs: LpBzInputArtifact {
            problem_normalization: LpBzProblemNormalization {
                period_count: problem.period_count,
                resource_constraint_count: problem.resource_constraint_count,
                destination_count: problem.destination_count,
                discount_rate: problem.discount_rate,
            },
            precedence_units: LpBzPrecedenceUnits {
                unit_count: precedence_unit_count,
                edge_count: precedence_edge_count,
                unit_granularity_label: unit_granularity_label.to_owned(),
            },
            lp_relaxation_source: LpBzRelaxationSource {
                source_label: "lp-pcpsp-native-proxy".to_owned(),
                reference_artifact_path: relative_or_display(
                    lp_relaxation_reference_path,
                    repo_root,
                ),
                objective_kind: "discounted-objective".to_owned(),
            },
        },
        lp_bz_bound_artifact: LpBzBoundArtifact {
            bound_label: "lp-bz-native-resource-envelope".to_owned(),
            discounted_objective_bound,
            period_count: problem.period_count,
            resource_constraint_count: problem.resource_constraint_count,
            destination_count: problem.destination_count,
            unit_count: precedence_unit_count,
            bound_strategy:
                "native-resource-envelope-plus-knapsack-tightening-with-lp-proxy-safeguard"
                    .to_owned(),
            lp_proxy_discounted_objective,
            native_block_objective_upper_bound,
            native_resource_density_upper_bound,
            native_resource_knapsack_upper_bound,
            discount_inverse_upper_bound,
        },
    })
}

fn build_objective_lookup(
    problem: &MarvinScheduleProblem,
) -> Result<BTreeMap<(usize, usize), f64>, MineError> {
    let mut objective_lookup = BTreeMap::new();

    for term in &problem.objective_terms {
        if term.destination_index >= problem.destination_count {
            return Err(MineError::validation(format!(
                "Marvin objective term destination {} is outside destination range 0..{}",
                term.destination_index,
                problem.destination_count.saturating_sub(1)
            )));
        }
        if objective_lookup
            .insert(
                (term.linear_index, term.destination_index),
                term.objective_value,
            )
            .is_some()
        {
            return Err(MineError::validation(format!(
                "Marvin objective terms contain duplicate block/destination pair ({}, {})",
                term.linear_index, term.destination_index
            )));
        }
    }

    if objective_lookup.is_empty() {
        return Err(MineError::validation(
            "Marvin schedule problem has no objective terms".to_owned(),
        ));
    }

    Ok(objective_lookup)
}

fn compute_discounted_objective(
    problem: &MarvinScheduleProblem,
    solution: &MarvinScheduleSolution,
    objective_lookup: &BTreeMap<(usize, usize), f64>,
) -> Result<f64, MineError> {
    let mut discounted_objective = 0.0_f64;

    for assignment in &solution.assignments {
        if assignment.destination_index >= problem.destination_count {
            return Err(MineError::validation(format!(
                "LP relaxation destination {} is outside destination range 0..{}",
                assignment.destination_index,
                problem.destination_count.saturating_sub(1)
            )));
        }
        if assignment.period_index >= problem.period_count {
            return Err(MineError::validation(format!(
                "LP relaxation period {} is outside period range 0..{}",
                assignment.period_index,
                problem.period_count.saturating_sub(1)
            )));
        }
        if !assignment.fraction.is_finite() || assignment.fraction < -1.0e-9 {
            return Err(MineError::validation(format!(
                "LP relaxation assignment fraction for block {} is invalid ({})",
                assignment.linear_index, assignment.fraction
            )));
        }

        let objective_value = objective_lookup
            .get(&(assignment.linear_index, assignment.destination_index))
            .copied()
            .ok_or_else(|| {
                MineError::validation(format!(
                    "LP relaxation references block {} destination {} without objective term",
                    assignment.linear_index, assignment.destination_index
                ))
            })?;
        let discount_factor = discount_factor(problem.discount_rate, assignment.period_index)?;
        discounted_objective += objective_value * assignment.fraction / discount_factor;
    }

    Ok(discounted_objective)
}

fn compute_block_objective_upper_bound(
    objective_lookup: &BTreeMap<(usize, usize), f64>,
    discount_inverse_upper_bound: f64,
) -> Result<f64, MineError> {
    if !discount_inverse_upper_bound.is_finite() || discount_inverse_upper_bound <= 0.0 {
        return Err(MineError::validation(format!(
            "Invalid LP/BZ discount inverse upper bound: {discount_inverse_upper_bound}"
        )));
    }

    let mut objective_by_block = BTreeMap::<usize, f64>::new();
    for ((linear_index, _), objective_value) in objective_lookup {
        objective_by_block
            .entry(*linear_index)
            .and_modify(|current| {
                if *objective_value > *current {
                    *current = *objective_value;
                }
            })
            .or_insert(*objective_value);
    }

    if objective_by_block.is_empty() {
        return Err(MineError::validation(
            "Cannot compute LP/BZ bound without objective terms".to_owned(),
        ));
    }

    Ok(objective_by_block
        .values()
        .map(|value| value.max(0.0) * discount_inverse_upper_bound)
        .sum::<f64>())
}

fn compute_resource_knapsack_upper_bound(
    problem: &MarvinScheduleProblem,
    objective_lookup: &BTreeMap<(usize, usize), f64>,
    discount_inverse_upper_bound: f64,
) -> Result<Option<f64>, MineError> {
    if !discount_inverse_upper_bound.is_finite() || discount_inverse_upper_bound <= 0.0 {
        return Err(MineError::validation(format!(
            "Invalid LP/BZ discount inverse upper bound: {discount_inverse_upper_bound}"
        )));
    }

    let coefficient_lookup = problem
        .resource_coefficients
        .iter()
        .map(|coefficient| {
            (
                (
                    coefficient.linear_index,
                    coefficient.destination_index,
                    coefficient.resource_index,
                ),
                coefficient.coefficient,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let positive_terms = objective_lookup
        .iter()
        .filter_map(|((linear_index, destination_index), objective_value)| {
            if *objective_value <= 0.0 {
                None
            } else {
                Some((
                    *linear_index,
                    *destination_index,
                    *objective_value * discount_inverse_upper_bound,
                ))
            }
        })
        .collect::<Vec<_>>();

    if positive_terms.is_empty() {
        return Ok(Some(0.0));
    }

    let mut best_resource_upper_bound: Option<f64> = None;
    for resource_index in 0..problem.resource_constraint_count {
        let mut resource_capacity = 0.0_f64;
        let mut has_limit = false;
        for limit in &problem.resource_constraint_limits {
            if limit.resource_index != resource_index || limit.relation != 'L' {
                continue;
            }
            if limit.period_index >= problem.period_count {
                return Err(MineError::validation(format!(
                    "Marvin resource limit period {} is outside period range 0..{}",
                    limit.period_index,
                    problem.period_count.saturating_sub(1)
                )));
            }
            if !limit.limit.is_finite() {
                return Err(MineError::validation(format!(
                    "Marvin resource limit for resource {resource_index} period {} is not finite ({})",
                    limit.period_index, limit.limit
                )));
            }
            resource_capacity += limit.limit.max(0.0);
            has_limit = true;
        }
        if !has_limit {
            continue;
        }
        if !resource_capacity.is_finite() {
            return Err(MineError::validation(format!(
                "Marvin resource capacity for resource {resource_index} is not finite ({resource_capacity})"
            )));
        }

        let weighted_terms = positive_terms
            .iter()
            .map(|(linear_index, destination_index, objective_value)| {
                let coefficient = coefficient_lookup
                    .get(&(*linear_index, *destination_index, resource_index))
                    .copied()
                    .unwrap_or(0.0);
                (*objective_value, coefficient)
            })
            .collect::<Vec<_>>();

        let resource_upper_bound = solve_fractional_knapsack_upper_bound(
            resource_capacity,
            &weighted_terms,
            resource_index,
        )?;
        best_resource_upper_bound = Some(match best_resource_upper_bound {
            Some(current_best) => current_best.min(resource_upper_bound),
            None => resource_upper_bound,
        });
    }

    Ok(best_resource_upper_bound)
}

fn solve_fractional_knapsack_upper_bound(
    capacity: f64,
    weighted_terms: &[(f64, f64)],
    resource_index: usize,
) -> Result<f64, MineError> {
    if !capacity.is_finite() {
        return Err(MineError::validation(format!(
            "LP/BZ knapsack capacity for resource {resource_index} is not finite ({capacity})"
        )));
    }

    let mut remaining_capacity = capacity.max(0.0);
    let mut objective_upper_bound = 0.0_f64;
    let mut positive_weight_terms = Vec::new();

    for (objective_value, weight) in weighted_terms {
        if !objective_value.is_finite() || *objective_value < 0.0 {
            return Err(MineError::validation(format!(
                "LP/BZ knapsack objective term is invalid ({objective_value}) for resource {resource_index}"
            )));
        }
        if !weight.is_finite() {
            return Err(MineError::validation(format!(
                "LP/BZ knapsack weight is not finite ({weight}) for resource {resource_index}"
            )));
        }
        if *objective_value <= 0.0 {
            continue;
        }
        if *weight <= 0.0 {
            objective_upper_bound += objective_value;
            remaining_capacity -= weight;
            continue;
        }
        positive_weight_terms.push((*objective_value, *weight));
    }

    positive_weight_terms.sort_by(|(lhs_value, lhs_weight), (rhs_value, rhs_weight)| {
        let lhs_density = lhs_value / lhs_weight;
        let rhs_density = rhs_value / rhs_weight;
        rhs_density
            .partial_cmp(&lhs_density)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                lhs_weight
                    .partial_cmp(rhs_weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                lhs_value
                    .partial_cmp(rhs_value)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    for (objective_value, weight) in positive_weight_terms {
        if remaining_capacity <= 0.0 {
            break;
        }
        let fraction = (remaining_capacity / weight).min(1.0);
        objective_upper_bound += objective_value * fraction;
        remaining_capacity -= weight * fraction;
    }

    Ok(objective_upper_bound)
}

fn compute_resource_density_upper_bound(
    problem: &MarvinScheduleProblem,
    objective_lookup: &BTreeMap<(usize, usize), f64>,
) -> Result<Option<f64>, MineError> {
    let coefficient_lookup = problem
        .resource_coefficients
        .iter()
        .map(|coefficient| {
            (
                (
                    coefficient.linear_index,
                    coefficient.destination_index,
                    coefficient.resource_index,
                ),
                coefficient.coefficient,
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut best_resource_upper_bound: Option<f64> = None;

    for resource_index in 0..problem.resource_constraint_count {
        let mut max_density = 0.0_f64;
        let mut can_bound_positive_terms = true;

        for ((linear_index, destination_index), objective_value) in objective_lookup {
            if *objective_value <= 0.0 {
                continue;
            }
            let coefficient = coefficient_lookup
                .get(&(*linear_index, *destination_index, resource_index))
                .copied()
                .unwrap_or(0.0);
            if coefficient <= 0.0 {
                can_bound_positive_terms = false;
                break;
            }
            max_density = max_density.max(*objective_value / coefficient);
        }

        if !can_bound_positive_terms || max_density <= 0.0 {
            continue;
        }

        let mut resource_bound = 0.0_f64;
        let mut has_limit = false;
        for limit in &problem.resource_constraint_limits {
            if limit.resource_index != resource_index || limit.relation != 'L' {
                continue;
            }
            if limit.period_index >= problem.period_count {
                return Err(MineError::validation(format!(
                    "Marvin resource limit period {} is outside period range 0..{}",
                    limit.period_index,
                    problem.period_count.saturating_sub(1)
                )));
            }
            let discount = discount_factor(problem.discount_rate, limit.period_index)?;
            resource_bound += limit.limit.max(0.0) * max_density / discount;
            has_limit = true;
        }

        if has_limit {
            best_resource_upper_bound = Some(match best_resource_upper_bound {
                Some(current_best) => current_best.min(resource_bound),
                None => resource_bound,
            });
        }
    }

    Ok(best_resource_upper_bound)
}

fn discount_factor(discount_rate: f64, period_index: usize) -> Result<f64, MineError> {
    if !discount_rate.is_finite() || discount_rate <= -1.0 {
        return Err(MineError::validation(format!(
            "Invalid discount rate for LP/BZ bound computation: {discount_rate}"
        )));
    }
    let factor = (1.0 + discount_rate).powi(period_index as i32);
    if !factor.is_finite() || factor <= 0.0 {
        return Err(MineError::validation(format!(
            "Invalid discount factor for period {period_index}: {factor}"
        )));
    }
    Ok(factor)
}

fn compute_discount_inverse_upper_bound(
    discount_rate: f64,
    period_count: usize,
) -> Result<f64, MineError> {
    let mut inverse_upper_bound = 1.0_f64;
    for period_index in 0..period_count {
        let factor = discount_factor(discount_rate, period_index)?;
        inverse_upper_bound = inverse_upper_bound.max(1.0 / factor);
    }
    if !inverse_upper_bound.is_finite() || inverse_upper_bound <= 0.0 {
        return Err(MineError::validation(format!(
            "Invalid LP/BZ discount inverse upper bound: {inverse_upper_bound}"
        )));
    }
    Ok(inverse_upper_bound)
}

fn relative_or_display(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative_path) => relative_path.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::compute_lp_bz_bound_artifacts;
    use crate::marvin_support::{
        MarvinObjectiveTerm, MarvinResourceCoefficient, MarvinResourceConstraintLimit,
        MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleProblemKind,
        MarvinScheduleSolution,
    };
    use std::path::Path;

    fn build_two_block_problem(
        period_0_limit: f64,
        period_1_limit: f64,
        discount_rate: f64,
    ) -> MarvinScheduleProblem {
        MarvinScheduleProblem {
            kind: MarvinScheduleProblemKind::Pcpsp,
            name: "toy".to_owned(),
            block_count: 2,
            period_count: 2,
            destination_count: 1,
            resource_constraint_count: 1,
            general_constraint_count: 0,
            discount_rate,
            resource_constraint_limits: vec![
                MarvinResourceConstraintLimit {
                    resource_index: 0,
                    period_index: 0,
                    relation: 'L',
                    limit: period_0_limit,
                },
                MarvinResourceConstraintLimit {
                    resource_index: 0,
                    period_index: 1,
                    relation: 'L',
                    limit: period_1_limit,
                },
            ],
            objective_terms: vec![
                MarvinObjectiveTerm {
                    linear_index: 0,
                    destination_index: 0,
                    objective_value: 10.0,
                },
                MarvinObjectiveTerm {
                    linear_index: 1,
                    destination_index: 0,
                    objective_value: 8.0,
                },
            ],
            resource_coefficients: vec![
                MarvinResourceCoefficient {
                    linear_index: 0,
                    destination_index: 0,
                    resource_index: 0,
                    coefficient: 1.0,
                },
                MarvinResourceCoefficient {
                    linear_index: 1,
                    destination_index: 0,
                    resource_index: 0,
                    coefficient: 2.0,
                },
            ],
        }
    }

    #[test]
    fn compute_lp_bz_bound_artifacts_applies_knapsack_tightening() {
        let problem = build_two_block_problem(1.0, 1.0, 0.1);
        let lp_solution = MarvinScheduleSolution {
            kind: MarvinScheduleProblemKind::Pcpsp,
            unique_block_count: 2,
            assignments: vec![
                MarvinScheduleAssignment {
                    linear_index: 0,
                    destination_index: 0,
                    period_index: 0,
                    fraction: 1.0,
                },
                MarvinScheduleAssignment {
                    linear_index: 1,
                    destination_index: 0,
                    period_index: 1,
                    fraction: 0.5,
                },
            ],
        };

        let result = compute_lp_bz_bound_artifacts(
            &problem,
            &lp_solution,
            Path::new("repo/datasets/benchmarks/marvin/references/marvin.LPpcpsp"),
            Path::new("repo"),
            3,
            2,
            "shell-bench-phase",
        )
        .expect("LP/BZ bound artifacts should build");

        assert_eq!(
            result.lp_bz_inputs.problem_normalization.period_count,
            problem.period_count
        );
        assert_eq!(
            result.lp_bz_bound_artifact.bound_label,
            "lp-bz-native-resource-envelope"
        );
        assert_eq!(
            result.lp_bz_bound_artifact.discount_inverse_upper_bound,
            1.0
        );
        assert!(
            result
                .lp_bz_bound_artifact
                .bound_strategy
                .contains("knapsack-tightening")
        );
        assert!(
            (result.lp_bz_bound_artifact.lp_proxy_discounted_objective - 13.636_363_636).abs()
                < 1e-9
        );
        assert!(
            (result
                .lp_bz_bound_artifact
                .native_block_objective_upper_bound
                - 18.0)
                .abs()
                < 1e-9
        );
        let density_bound = result
            .lp_bz_bound_artifact
            .native_resource_density_upper_bound
            .expect("density bound should be available");
        let knapsack_bound = result
            .lp_bz_bound_artifact
            .native_resource_knapsack_upper_bound
            .expect("knapsack bound should be available");
        assert!(knapsack_bound < density_bound);
        assert!((knapsack_bound - 14.0).abs() < 1e-9);
        assert!((result.lp_bz_bound_artifact.discounted_objective_bound - 14.0).abs() < 1e-9);
        assert!(
            result.lp_bz_bound_artifact.discounted_objective_bound
                >= result.lp_bz_bound_artifact.lp_proxy_discounted_objective
        );
    }

    #[test]
    fn compute_lp_bz_bound_artifacts_resource_tightening_is_monotonic_with_limits() {
        let lower_limit_problem = build_two_block_problem(0.5, 0.5, 0.1);
        let higher_limit_problem = build_two_block_problem(1.0, 1.0, 0.1);
        let lp_solution = MarvinScheduleSolution {
            kind: MarvinScheduleProblemKind::Pcpsp,
            unique_block_count: 0,
            assignments: Vec::new(),
        };

        let lower_bound = compute_lp_bz_bound_artifacts(
            &lower_limit_problem,
            &lp_solution,
            Path::new("repo/datasets/benchmarks/marvin/references/marvin.LPpcpsp"),
            Path::new("repo"),
            3,
            2,
            "shell-bench-phase",
        )
        .expect("lower-limit LP/BZ bound should build")
        .lp_bz_bound_artifact;
        let higher_bound = compute_lp_bz_bound_artifacts(
            &higher_limit_problem,
            &lp_solution,
            Path::new("repo/datasets/benchmarks/marvin/references/marvin.LPpcpsp"),
            Path::new("repo"),
            3,
            2,
            "shell-bench-phase",
        )
        .expect("higher-limit LP/BZ bound should build")
        .lp_bz_bound_artifact;

        assert!(
            higher_bound
                .native_resource_knapsack_upper_bound
                .expect("higher knapsack bound should be available")
                >= lower_bound
                    .native_resource_knapsack_upper_bound
                    .expect("lower knapsack bound should be available")
        );
        assert!(higher_bound.discounted_objective_bound >= lower_bound.discounted_objective_bound);
    }

    #[test]
    fn compute_lp_bz_bound_artifacts_handles_negative_discount_with_explicit_inverse_bound() {
        let problem = MarvinScheduleProblem {
            kind: MarvinScheduleProblemKind::Pcpsp,
            name: "toy-negative-discount".to_owned(),
            block_count: 1,
            period_count: 2,
            destination_count: 1,
            resource_constraint_count: 0,
            general_constraint_count: 0,
            discount_rate: -0.5,
            resource_constraint_limits: Vec::new(),
            objective_terms: vec![MarvinObjectiveTerm {
                linear_index: 0,
                destination_index: 0,
                objective_value: 10.0,
            }],
            resource_coefficients: Vec::new(),
        };
        let lp_solution = MarvinScheduleSolution {
            kind: MarvinScheduleProblemKind::Pcpsp,
            unique_block_count: 1,
            assignments: vec![MarvinScheduleAssignment {
                linear_index: 0,
                destination_index: 0,
                period_index: 1,
                fraction: 1.0,
            }],
        };

        let bound_artifact = compute_lp_bz_bound_artifacts(
            &problem,
            &lp_solution,
            Path::new("repo/datasets/benchmarks/marvin/references/marvin.LPpcpsp"),
            Path::new("repo"),
            1,
            0,
            "shell-bench-phase",
        )
        .expect("negative discount LP/BZ bound should build")
        .lp_bz_bound_artifact;

        assert!((bound_artifact.discount_inverse_upper_bound - 2.0).abs() < 1e-9);
        assert!((bound_artifact.lp_proxy_discounted_objective - 20.0).abs() < 1e-9);
        assert!((bound_artifact.native_block_objective_upper_bound - 20.0).abs() < 1e-9);
        assert!(
            bound_artifact.discounted_objective_bound
                >= bound_artifact.lp_proxy_discounted_objective
        );
    }

    #[test]
    fn compute_lp_bz_bound_artifacts_rejects_out_of_range_period() {
        let problem = MarvinScheduleProblem {
            kind: MarvinScheduleProblemKind::Pcpsp,
            name: "toy".to_owned(),
            block_count: 1,
            period_count: 1,
            destination_count: 1,
            resource_constraint_count: 0,
            general_constraint_count: 0,
            discount_rate: 0.1,
            resource_constraint_limits: Vec::new(),
            objective_terms: vec![MarvinObjectiveTerm {
                linear_index: 0,
                destination_index: 0,
                objective_value: 10.0,
            }],
            resource_coefficients: Vec::new(),
        };
        let lp_solution = MarvinScheduleSolution {
            kind: MarvinScheduleProblemKind::Pcpsp,
            unique_block_count: 1,
            assignments: vec![MarvinScheduleAssignment {
                linear_index: 0,
                destination_index: 0,
                period_index: 1,
                fraction: 1.0,
            }],
        };

        let error = compute_lp_bz_bound_artifacts(
            &problem,
            &lp_solution,
            Path::new("repo/datasets/benchmarks/marvin/references/marvin.LPpcpsp"),
            Path::new("repo"),
            1,
            0,
            "shell-bench-phase",
        )
        .expect_err("out of range period should be rejected");

        assert!(format!("{error}").contains("outside period range"));
    }
}
