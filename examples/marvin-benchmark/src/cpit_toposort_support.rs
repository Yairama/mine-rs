//! Soporte benchmark-side para el candidato CPIT TopoSort (MR-211).
//!
//! Adapta el contrato MineLib CPIT normalizado (`MarvinScheduleProblem`) al
//! problema del solver core `mine_planning::solve_cpit_with_toposort`, deriva
//! los scores de orden desde la relajación LP abierta (`*.LPcpit`) y verifica
//! factibilidad de precedencias del schedule resultante.
//!
//! Método de referencia: Chicoisne et al. (2012), Operations Research
//! 60(3):517-528, doi 10.1287/opre.1120.1072 ([R35]).

use std::collections::BTreeMap;

use mine_sdk::{
    CpitToposortProblem, CpitToposortSchedule, MineError, PrecedenceGraph, PrecedenceNode,
};

use crate::marvin_support::{
    MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleProblemKind,
};

/// Fracción LP mínima para considerar un bloque dentro del soporte.
pub const LP_SUPPORT_MINIMUM_FRACTION: f64 = 1.0e-9;

/// Score por bloque: periodo esperado de extracción según la relajación LP.
///
/// Para cada bloque calcula `sum(periodo * fraccion) / sum(fraccion)` sobre las
/// asignaciones fraccionales de la relajación; los bloques sin soporte LP
/// quedan fuera (no se programan).
#[allow(dead_code)] // usado por bins toposort; no por todos los bins que incluyen este módulo
#[must_use]
pub fn build_expected_period_scores(
    assignments: &[MarvinScheduleAssignment],
) -> BTreeMap<usize, f64> {
    let mut weighted_period_sum: BTreeMap<usize, f64> = BTreeMap::new();
    let mut fraction_sum: BTreeMap<usize, f64> = BTreeMap::new();
    for assignment in assignments {
        *weighted_period_sum
            .entry(assignment.linear_index)
            .or_insert(0.0) += assignment.fraction * assignment.period_index as f64;
        *fraction_sum.entry(assignment.linear_index).or_insert(0.0) += assignment.fraction;
    }

    weighted_period_sum
        .into_iter()
        .filter_map(|(linear_index, weighted_sum)| {
            let total_fraction = fraction_sum.get(&linear_index).copied().unwrap_or(0.0);
            if total_fraction <= LP_SUPPORT_MINIMUM_FRACTION {
                return None;
            }
            Some((linear_index, weighted_sum / total_fraction))
        })
        .collect()
}

/// Convierte el contrato MineLib CPIT al problema del solver core.
///
/// Retorna además la lista de relaciones de recurso que la heurística no
/// refuerza (`G` completa y el lado inferior de `E`), para auditoría explícita.
///
/// # Errores
///
/// Falla si el problema no es CPIT mono-destino o declara relaciones de
/// restricción no soportadas.
#[allow(dead_code)] // usado por el bin `cpit_toposort`; no por todos los bins que incluyen este módulo
pub fn build_toposort_problem_from_minelib_cpit(
    problem: &MarvinScheduleProblem,
) -> Result<(CpitToposortProblem, Vec<String>), MineError> {
    if problem.kind != MarvinScheduleProblemKind::Cpit {
        return Err(MineError::validation(
            "toposort harness requires a CPIT problem".to_owned(),
        ));
    }
    if problem.destination_count != 1 {
        return Err(MineError::validation(format!(
            "CPIT problem declares {} destinations; the toposort harness only supports 1",
            problem.destination_count
        )));
    }

    let mut block_values: BTreeMap<usize, f64> = BTreeMap::new();
    for term in &problem.objective_terms {
        if term.destination_index != 0 {
            return Err(MineError::validation(format!(
                "CPIT objective term for block {} references destination {}",
                term.linear_index, term.destination_index
            )));
        }
        block_values.insert(term.linear_index, term.objective_value);
    }

    let mut block_resource_usage: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for coefficient in &problem.resource_coefficients {
        if coefficient.destination_index != 0 {
            return Err(MineError::validation(format!(
                "CPIT resource coefficient for block {} references destination {}",
                coefficient.linear_index, coefficient.destination_index
            )));
        }
        let usage = block_resource_usage
            .entry(coefficient.linear_index)
            .or_insert_with(|| vec![0.0; problem.resource_constraint_count]);
        usage[coefficient.resource_index] = coefficient.coefficient;
    }

    let mut limits = vec![vec![None; problem.resource_constraint_count]; problem.period_count];
    let mut unenforced_relations: Vec<String> = Vec::new();
    for limit in &problem.resource_constraint_limits {
        match limit.relation {
            'L' | 'E' => {
                limits[limit.period_index][limit.resource_index] = Some(limit.limit);
                if limit.relation == 'E' {
                    unenforced_relations.push(format!(
                        "resource {} period {} uses relation `E`; only its upper side is enforced",
                        limit.resource_index, limit.period_index
                    ));
                }
            }
            'G' => {
                unenforced_relations.push(format!(
                    "resource {} period {} uses relation `G` (lower bound); not enforced by the toposort heuristic",
                    limit.resource_index, limit.period_index
                ));
            }
            other => {
                return Err(MineError::validation(format!(
                    "unsupported resource constraint relation `{other}`"
                )));
            }
        }
    }

    Ok((
        CpitToposortProblem {
            period_count: problem.period_count,
            discount_rate: problem.discount_rate,
            resource_count: problem.resource_constraint_count,
            block_values,
            block_resource_usage,
            period_resource_upper_limits: limits,
        },
        unenforced_relations,
    ))
}

/// Verifica que el schedule respete todas las precedencias bloque → bloque:
/// si el sucesor está programado, el predecesor debe estarlo en un periodo
/// menor o igual (formulación "by-period" de CPIT).
///
/// Retorna la cantidad de aristas verificadas.
///
/// # Errores
///
/// Falla con detalle del bloque/periodo si encuentra una violación de
/// precedencia temporal o de clausura.
#[allow(dead_code)] // usado por el bin `cpit_toposort`; no por todos los bins que incluyen este módulo
pub fn verify_schedule_precedence(
    schedule: &CpitToposortSchedule,
    precedence_graph: &PrecedenceGraph,
) -> Result<usize, MineError> {
    let assigned_periods: BTreeMap<usize, usize> = schedule
        .assignments
        .iter()
        .map(|assignment| (assignment.linear_index, assignment.period_index))
        .collect();

    let mut verified_edges = 0usize;
    for edge in precedence_graph.edges() {
        let (PrecedenceNode::Block(pred_idx), PrecedenceNode::Block(succ_idx)) =
            (edge.predecessor(), edge.successor())
        else {
            continue;
        };
        let Some(&succ_period) = assigned_periods.get(succ_idx) else {
            continue;
        };
        match assigned_periods.get(pred_idx) {
            Some(&pred_period) if pred_period <= succ_period => {
                verified_edges += 1;
            }
            Some(&pred_period) => {
                return Err(MineError::validation(format!(
                    "precedence violation: block {pred_idx} scheduled at period {pred_period} \
                     after its successor {succ_idx} at period {succ_period}"
                )));
            }
            None => {
                return Err(MineError::validation(format!(
                    "closure violation: block {succ_idx} is scheduled but its predecessor \
                     {pred_idx} is not"
                )));
            }
        }
    }
    Ok(verified_edges)
}
