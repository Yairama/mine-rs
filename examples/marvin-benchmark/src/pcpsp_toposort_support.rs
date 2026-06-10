//! Soporte benchmark-side para el candidato PCPSP TopoSort multi-destino
//! (MR-212, primer hito).
//!
//! Adapta el contrato MineLib PCPSP normalizado (`MarvinScheduleProblem`) al
//! problema del solver core `mine_planning::solve_pcpsp_with_toposort` y
//! verifica factibilidad de precedencias del schedule resultante.
//!
//! Referencias: Chicoisne et al. (2012), doi 10.1287/opre.1120.1072 ([R35]);
//! Espinoza et al. (2013), doi 10.1007/s10479-012-1258-3 ([R29]).

use std::collections::BTreeMap;

use mine_sdk::{
    MineError, PcpspToposortProblem, PcpspToposortSchedule, PrecedenceGraph, PrecedenceNode,
};

use crate::marvin_support::{MarvinScheduleProblem, MarvinScheduleProblemKind};

/// Convierte el contrato MineLib PCPSP al problema del solver core.
///
/// Retorna además la lista de relaciones de recurso no reforzadas (`G`
/// completa y el lado inferior de `E`) para auditoría explícita.
///
/// # Errores
///
/// Falla si el problema no es PCPSP, si declara restricciones laterales
/// generales (no representadas por la heurística) o si usa relaciones de
/// recurso no soportadas.
pub fn build_pcpsp_toposort_problem_from_minelib(
    problem: &MarvinScheduleProblem,
) -> Result<(PcpspToposortProblem, Vec<String>), MineError> {
    if problem.kind != MarvinScheduleProblemKind::Pcpsp {
        return Err(MineError::validation(
            "pcpsp toposort harness requires a PCPSP problem".to_owned(),
        ));
    }
    if problem.destination_count == 0 {
        return Err(MineError::validation(
            "PCPSP problem must declare at least one destination".to_owned(),
        ));
    }
    if problem.general_constraint_count > 0 {
        return Err(MineError::validation(format!(
            "PCPSP problem declares {} general side constraints; the toposort heuristic \
             cannot enforce them and would overclaim feasibility",
            problem.general_constraint_count
        )));
    }

    let destination_count = problem.destination_count;
    let mut block_values: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for term in &problem.objective_terms {
        if term.destination_index >= destination_count {
            return Err(MineError::validation(format!(
                "PCPSP objective term for block {} references destination {} outside 0..{}",
                term.linear_index,
                term.destination_index,
                destination_count.saturating_sub(1)
            )));
        }
        block_values
            .entry(term.linear_index)
            .or_insert_with(|| vec![0.0; destination_count])[term.destination_index] =
            term.objective_value;
    }

    let mut block_resource_usage: BTreeMap<usize, Vec<Vec<f64>>> = BTreeMap::new();
    for coefficient in &problem.resource_coefficients {
        if coefficient.destination_index >= destination_count {
            return Err(MineError::validation(format!(
                "PCPSP resource coefficient for block {} references destination {} outside 0..{}",
                coefficient.linear_index,
                coefficient.destination_index,
                destination_count.saturating_sub(1)
            )));
        }
        let matrix = block_resource_usage
            .entry(coefficient.linear_index)
            .or_insert_with(|| {
                vec![vec![0.0; problem.resource_constraint_count]; destination_count]
            });
        matrix[coefficient.destination_index][coefficient.resource_index] = coefficient.coefficient;
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
        PcpspToposortProblem {
            period_count: problem.period_count,
            discount_rate: problem.discount_rate,
            destination_count,
            resource_count: problem.resource_constraint_count,
            block_values,
            block_resource_usage,
            period_resource_upper_limits: limits,
        },
        unenforced_relations,
    ))
}

/// Verifica que el schedule PCPSP respete todas las precedencias bloque →
/// bloque (formulación "by-period"). Retorna la cantidad de aristas
/// verificadas.
///
/// # Errores
///
/// Falla con detalle del bloque/periodo ante violaciones de precedencia
/// temporal o de clausura.
pub fn verify_pcpsp_schedule_precedence(
    schedule: &PcpspToposortSchedule,
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
