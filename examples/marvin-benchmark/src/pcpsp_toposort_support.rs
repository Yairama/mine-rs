//! Soporte benchmark-side para el candidato PCPSP TopoSort multi-destino
//! (MR-212, primer hito).
//!
//! Adapta el contrato MineLib PCPSP normalizado (`MarvinScheduleProblem`) al
//! problema del solver core `mine_planning::solve_pcpsp_with_toposort` y
//! verifica factibilidad de precedencias del schedule resultante.
//!
//! Referencias: Chicoisne et al. (2012), doi 10.1287/opre.1120.1072 ([R35]);
//! Espinoza et al. (2013), doi 10.1007/s10479-012-1258-3 ([R29]).

use std::collections::{BTreeMap, BTreeSet};

use mine_sdk::{
    CpitToposortProblem, MineError, PcpspToposortProblem, PcpspToposortSchedule, PrecedenceGraph,
    PrecedenceNode,
};
use serde::Serialize;

use crate::marvin_support::{
    MarvinScheduleAssignment, MarvinScheduleProblem, MarvinScheduleProblemKind,
};

/// Métricas de alineación temporal y de ruteo de un candidato contra la
/// solución de referencia MineLib (insumo del gate de promoción MR-206).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemporalAlignmentSummary {
    /// Bloques presentes en candidato y referencia.
    pub shared_block_count: usize,
    /// Bloques con |delta de periodo| < 0.5 respecto al periodo esperado de la
    /// referencia (media ponderada por fracciones).
    pub exact_period_match_count: usize,
    /// Bloques extraídos antes que en la referencia (delta < -0.5).
    pub earlier_than_reference_count: usize,
    /// Bloques extraídos después que en la referencia (delta > 0.5).
    pub later_than_reference_count: usize,
    /// Media de |delta de periodo| sobre los bloques compartidos.
    pub mean_absolute_period_delta: f64,
    /// Máximo |delta de periodo| observado.
    pub max_absolute_period_delta: f64,
    /// Jaccard de membresía `(bloque, periodo, destino)` usando la asignación
    /// de mayor fracción de la referencia.
    pub period_destination_jaccard: f64,
}

/// Compara el schedule candidato contra las asignaciones de la solución de
/// referencia (posiblemente fraccionales) y resume drift temporal y ruteo.
#[allow(dead_code)] // usado por el bin `pcpsp_toposort`; no por todos los bins que incluyen este módulo
#[must_use]
pub fn summarize_temporal_alignment(
    candidate: &PcpspToposortSchedule,
    reference_assignments: &[MarvinScheduleAssignment],
) -> TemporalAlignmentSummary {
    // Periodo esperado (media ponderada por fracción) y celda (periodo,
    // destino) de mayor fracción por bloque de la referencia.
    let mut weighted_period_sum: BTreeMap<usize, f64> = BTreeMap::new();
    let mut fraction_sum: BTreeMap<usize, f64> = BTreeMap::new();
    let mut argmax_cell: BTreeMap<usize, (f64, usize, usize)> = BTreeMap::new();
    for assignment in reference_assignments {
        *weighted_period_sum
            .entry(assignment.linear_index)
            .or_insert(0.0) += assignment.fraction * assignment.period_index as f64;
        *fraction_sum.entry(assignment.linear_index).or_insert(0.0) += assignment.fraction;
        let entry = argmax_cell
            .entry(assignment.linear_index)
            .or_insert((f64::NEG_INFINITY, 0, 0));
        if assignment.fraction > entry.0 {
            *entry = (
                assignment.fraction,
                assignment.period_index,
                assignment.destination_index,
            );
        }
    }

    let mut shared_block_count = 0usize;
    let mut exact_period_match_count = 0usize;
    let mut earlier_than_reference_count = 0usize;
    let mut later_than_reference_count = 0usize;
    let mut absolute_delta_sum = 0.0_f64;
    let mut max_absolute_period_delta = 0.0_f64;
    for assignment in &candidate.assignments {
        let Some(&total_fraction) = fraction_sum.get(&assignment.linear_index) else {
            continue;
        };
        if total_fraction <= 1.0e-12 {
            continue;
        }
        shared_block_count += 1;
        let expected_period = weighted_period_sum[&assignment.linear_index] / total_fraction;
        let delta = assignment.period_index as f64 - expected_period;
        let absolute_delta = delta.abs();
        absolute_delta_sum += absolute_delta;
        max_absolute_period_delta = max_absolute_period_delta.max(absolute_delta);
        if absolute_delta < 0.5 {
            exact_period_match_count += 1;
        } else if delta < 0.0 {
            earlier_than_reference_count += 1;
        } else {
            later_than_reference_count += 1;
        }
    }

    let candidate_cells: BTreeSet<(usize, usize, usize)> = candidate
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.linear_index,
                assignment.period_index,
                assignment.destination_index,
            )
        })
        .collect();
    let reference_cells: BTreeSet<(usize, usize, usize)> = argmax_cell
        .iter()
        .map(|(linear, (_, period, destination))| (*linear, *period, *destination))
        .collect();
    let intersection = candidate_cells.intersection(&reference_cells).count();
    let union = candidate_cells.union(&reference_cells).count();
    let period_destination_jaccard = if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    };

    TemporalAlignmentSummary {
        shared_block_count,
        exact_period_match_count,
        earlier_than_reference_count,
        later_than_reference_count,
        mean_absolute_period_delta: if shared_block_count == 0 {
            0.0
        } else {
            absolute_delta_sum / shared_block_count as f64
        },
        max_absolute_period_delta,
        period_destination_jaccard,
    }
}

/// Reexpresa un problema CPIT mono-destino como `PcpspToposortProblem` con un
/// solo destino, para reutilizar la misma ruta de bound/candidato multi-destino
/// (CPIT es el caso particular `destination_count = 1`).
#[allow(dead_code)] // usado por el bin `pcpsp_bound`; no por todos los bins que incluyen este módulo
#[must_use]
pub fn pcpsp_problem_from_cpit_toposort(problem: &CpitToposortProblem) -> PcpspToposortProblem {
    PcpspToposortProblem {
        period_count: problem.period_count,
        discount_rate: problem.discount_rate,
        destination_count: 1,
        resource_count: problem.resource_count,
        block_values: problem
            .block_values
            .iter()
            .map(|(linear, value)| (*linear, vec![*value]))
            .collect(),
        block_resource_usage: problem
            .block_resource_usage
            .iter()
            .map(|(linear, usage)| (*linear, vec![usage.clone()]))
            .collect(),
        period_resource_upper_limits: problem.period_resource_upper_limits.clone(),
    }
}

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
