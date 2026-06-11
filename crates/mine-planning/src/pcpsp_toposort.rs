//! Heurística determinista tipo TopoSort para PCPSP a nivel de bloque.
//!
//! Extiende la heurística CPIT de `cpit_toposort.rs` al caso multi-destino del
//! Precedence Constrained Production Scheduling Problem (PCPSP) de MineLib:
//! cada bloque tiene un valor y un consumo de recursos por destino, y la
//! decisión de destino se toma durante la construcción del schedule.
//!
//! Referencias (literatura académica revisada por pares):
//!
//! - Chicoisne, Espinoza, Goycoolea, Moreno, Rubio (2012), Operations Research
//!   60(3):517-528. doi 10.1287/opre.1120.1072 ([R35]): rounding TopoSort
//!   guiado por tiempos esperados de una relajación LP.
//! - Espinoza, Goycoolea, Moreno, Newman (2013), Annals of Operations Research
//!   206:93-114. doi 10.1007/s10479-012-1258-3 ([R29]): formulación PCPSP.
//!
//! # Regla de decisión de destino
//!
//! Al asignar un bloque, para cada destino se busca el primer periodo factible
//! `p_d >= earliest` (máximo periodo de sus predecesores) y se elige el destino
//! que maximiza el valor descontado `value_d / (1 + r)^{p_d}`, con desempate
//! determinista por periodo más temprano y luego por índice de destino. Esto
//! evita destruir valor enviando mineral a botadero cuando la planta se llena
//! en un periodo puntual: el destino de mayor valor puede ganar en `p_d + k`.
//!
//! # Contratos compartidos con la variante CPIT
//!
//! - Solo se programan bloques presentes en `ordering_scores` (el soporte).
//! - Predecesor fuera del soporte o descartado ⇒ el sucesor se descarta.
//! - Solo se refuerzan límites superiores de recursos por periodo.
//! - Descuento `1 / (1 + discount_rate)^period_index`, periodos 0-based.

use std::cmp::Ordering as CmpOrdering;
use std::collections::{BTreeMap, BinaryHeap};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::precedence::{PrecedenceGraph, PrecedenceNode};

// ── Contratos de entrada ─────────────────────────────────────────────────────

/// Problema PCPSP a nivel de bloque para la heurística TopoSort multi-destino.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcpspToposortProblem {
    /// Número de periodos disponibles.
    pub period_count: usize,
    /// Tasa de descuento por periodo (>= 0).
    pub discount_rate: f64,
    /// Número de destinos.
    pub destination_count: usize,
    /// Número de recursos con límite por periodo.
    pub resource_count: usize,
    /// Valor económico no descontado por bloque y destino
    /// (índice lineal → vector de largo `destination_count`).
    pub block_values: BTreeMap<usize, Vec<f64>>,
    /// Consumo de recursos por bloque, destino y recurso
    /// (índice lineal → matriz `[destination_count][resource_count]`).
    pub block_resource_usage: BTreeMap<usize, Vec<Vec<f64>>>,
    /// Límites superiores por periodo y recurso (`None` = sin límite).
    /// Dimensiones: `[period_count][resource_count]`.
    pub period_resource_upper_limits: Vec<Vec<Option<f64>>>,
}

/// Opciones explícitas de la heurística multi-destino.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcpspToposortOptions {
    /// Si es `true`, aplica un post-pass determinista que retrasa bloques con
    /// valor asignado negativo hasta el periodo más tardío factible sin violar
    /// precedencias ni capacidades (mismo criterio que la variante CPIT).
    pub delay_negative_blocks: bool,
}

impl Default for PcpspToposortOptions {
    fn default() -> Self {
        Self {
            delay_negative_blocks: true,
        }
    }
}

// ── Contratos de salida ──────────────────────────────────────────────────────

/// Asignación bloque → (destino, periodo) producida por la heurística.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcpspToposortAssignment {
    /// Índice lineal del bloque asignado.
    pub linear_index: usize,
    /// Destino asignado (0-based).
    pub destination_index: usize,
    /// Periodo asignado (0-based).
    pub period_index: usize,
}

/// Schedule PCPSP serializable producido por la heurística TopoSort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcpspToposortSchedule {
    /// Asignaciones ordenadas por (periodo, bloque).
    pub assignments: Vec<PcpspToposortAssignment>,
    /// Cantidad de bloques programados.
    pub scheduled_block_count: usize,
    /// Bloques del soporte descartados por falta de capacidad en todos los
    /// destinos y periodos elegibles.
    pub dropped_for_capacity_count: usize,
    /// Bloques descartados porque un predecesor quedó fuera del soporte o fue
    /// descartado.
    pub dropped_for_predecessor_count: usize,
    /// Bloques con valor negativo retrasados por el post-pass.
    pub delayed_negative_block_count: usize,
    /// Objetivo no descontado del schedule.
    pub undiscounted_objective: f64,
    /// Objetivo descontado con `1/(1+r)^periodo`.
    pub discounted_objective: f64,
    /// Uso de recursos por periodo: `[period_count][resource_count]`.
    pub period_resource_usage: Vec<Vec<f64>>,
    /// Cantidad de periodos con al menos un bloque asignado.
    pub used_period_count: usize,
    /// Cantidad de destinos con al menos un bloque asignado.
    pub used_destination_count: usize,
}

// ── Orden topológico guiado por score ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
struct ReadyBlock {
    score: f64,
    position: usize,
}

impl Eq for ReadyBlock {}

impl Ord for ReadyBlock {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // BinaryHeap es max-heap: invertimos para extraer el menor score primero.
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| other.position.cmp(&self.position))
    }
}

impl PartialOrd for ReadyBlock {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BlockStatus {
    Pending,
    Assigned {
        destination: usize,
        period: usize,
        value: f64,
    },
    DroppedCapacity,
    DroppedPredecessor,
}

// ── Solver ───────────────────────────────────────────────────────────────────

/// Resuelve PCPSP con la heurística TopoSort multi-destino guiada por
/// `ordering_scores`.
///
/// # Parámetros
///
/// - `problem`: contrato PCPSP a nivel de bloque (valores y recursos por
///   destino, límites por periodo).
/// - `precedence_graph`: DAG de precedencias; solo se procesan aristas
///   bloque → bloque.
/// - `ordering_scores`: score por bloque (menor = antes), típicamente el tiempo
///   esperado de extracción de una relajación LP ([R35]). Define el soporte.
/// - `options`: opciones explícitas de la heurística.
///
/// # Errores
///
/// Retorna error si el problema está mal dimensionado, si los scores
/// referencian bloques sin valores declarados o si algún insumo numérico no es
/// finito.
pub fn solve_pcpsp_with_toposort(
    problem: &PcpspToposortProblem,
    precedence_graph: &PrecedenceGraph,
    ordering_scores: &BTreeMap<usize, f64>,
    options: &PcpspToposortOptions,
) -> Result<PcpspToposortSchedule, MineError> {
    validate_pcpsp_problem(problem)?;
    if ordering_scores.is_empty() {
        return Err(MineError::invalid_parameter(
            "ordering_scores",
            "toposort heuristic requires at least one ordering score",
        ));
    }
    for (linear_index, score) in ordering_scores {
        if !score.is_finite() {
            return Err(MineError::invalid_parameter(
                "ordering_scores",
                format!("ordering score for block {linear_index} is not finite"),
            ));
        }
        if !problem.block_values.contains_key(linear_index) {
            return Err(MineError::invalid_parameter(
                "ordering_scores",
                format!("ordering score references block {linear_index} without declared values"),
            ));
        }
    }

    let support: Vec<usize> = ordering_scores.keys().copied().collect();
    let position: BTreeMap<usize, usize> = support
        .iter()
        .enumerate()
        .map(|(pos, linear)| (*linear, pos))
        .collect();
    let n = support.len();

    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree: Vec<usize> = vec![0; n];
    let mut status: Vec<BlockStatus> = vec![BlockStatus::Pending; n];

    for edge in precedence_graph.edges() {
        let (PrecedenceNode::Block(pred_idx), PrecedenceNode::Block(succ_idx)) =
            (edge.predecessor(), edge.successor())
        else {
            continue;
        };
        let Some(&succ_pos) = position.get(succ_idx) else {
            continue;
        };
        match position.get(pred_idx) {
            Some(&pred_pos) => {
                preds[succ_pos].push(pred_pos);
                succs[pred_pos].push(succ_pos);
                indegree[succ_pos] += 1;
            }
            None => {
                status[succ_pos] = BlockStatus::DroppedPredecessor;
            }
        }
    }

    let mut heap: BinaryHeap<ReadyBlock> = BinaryHeap::new();
    for (pos, &linear) in support.iter().enumerate() {
        if indegree[pos] == 0 {
            heap.push(ReadyBlock {
                score: ordering_scores[&linear],
                position: pos,
            });
        }
    }

    let mut usage = vec![vec![0.0_f64; problem.resource_count]; problem.period_count];
    let mut pop_order: Vec<usize> = Vec::with_capacity(n);
    let mut dropped_for_capacity = 0usize;

    while let Some(ready) = heap.pop() {
        let pos = ready.position;
        pop_order.push(pos);
        let linear = support[pos];

        let mut earliest_period = 0usize;
        let mut blocked_by_predecessor = status[pos] == BlockStatus::DroppedPredecessor;
        for &pred_pos in &preds[pos] {
            match status[pred_pos] {
                BlockStatus::Assigned { period, .. } => {
                    earliest_period = earliest_period.max(period);
                }
                _ => {
                    blocked_by_predecessor = true;
                }
            }
        }

        if blocked_by_predecessor {
            status[pos] = BlockStatus::DroppedPredecessor;
        } else {
            let values = &problem.block_values[&linear];
            let usage_matrix = problem.block_resource_usage.get(&linear);
            let mut best: Option<(f64, usize, usize, f64)> = None; // (descontado, periodo, destino, valor)
            for (destination, &value) in values.iter().enumerate() {
                let destination_usage = usage_matrix.map(|matrix| matrix[destination].as_slice());
                let feasible_period = (earliest_period..problem.period_count).find(|&period| {
                    destination_usage.is_none_or(|block_usage| {
                        fits_in_period(
                            &usage[period],
                            block_usage,
                            &problem.period_resource_upper_limits[period],
                        )
                    })
                });
                let Some(period) = feasible_period else {
                    continue;
                };
                let discounted = value / (1.0 + problem.discount_rate).powi(period as i32);
                let candidate = (discounted, period, destination, value);
                best = match best {
                    None => Some(candidate),
                    Some(current) => {
                        // Mayor valor descontado; desempate por periodo más
                        // temprano y luego por índice de destino menor.
                        let better = discounted > current.0 + f64::EPSILON
                            || (discounted >= current.0 - f64::EPSILON
                                && (period < current.1
                                    || (period == current.1 && destination < current.2)));
                        if better {
                            Some(candidate)
                        } else {
                            Some(current)
                        }
                    }
                };
            }

            match best {
                Some((_, period, destination, value)) => {
                    if let Some(matrix) = usage_matrix {
                        for (resource, amount) in matrix[destination].iter().enumerate() {
                            usage[period][resource] += amount;
                        }
                    }
                    status[pos] = BlockStatus::Assigned {
                        destination,
                        period,
                        value,
                    };
                }
                None => {
                    status[pos] = BlockStatus::DroppedCapacity;
                    dropped_for_capacity += 1;
                }
            }
        }

        for &succ_pos in &succs[pos] {
            indegree[succ_pos] -= 1;
            if indegree[succ_pos] == 0 {
                let succ_linear = support[succ_pos];
                heap.push(ReadyBlock {
                    score: ordering_scores[&succ_linear],
                    position: succ_pos,
                });
            }
        }
    }

    for block_status in &mut status {
        if *block_status == BlockStatus::Pending {
            *block_status = BlockStatus::DroppedPredecessor;
        }
    }

    // Post-pass: retrasar bloques con valor asignado negativo, conservando el
    // destino elegido.
    let mut delayed_negative = 0usize;
    if options.delay_negative_blocks {
        for &pos in pop_order.iter().rev() {
            let BlockStatus::Assigned {
                destination,
                period: current_period,
                value,
            } = status[pos]
            else {
                continue;
            };
            if value >= 0.0 {
                continue;
            }

            let linear = support[pos];
            let mut latest_allowed = problem.period_count - 1;
            for &succ_pos in &succs[pos] {
                if let BlockStatus::Assigned { period, .. } = status[succ_pos] {
                    latest_allowed = latest_allowed.min(period);
                }
            }
            if latest_allowed <= current_period {
                continue;
            }

            let Some(matrix) = problem.block_resource_usage.get(&linear) else {
                // Sin consumo de recursos: mover siempre es factible.
                status[pos] = BlockStatus::Assigned {
                    destination,
                    period: latest_allowed,
                    value,
                };
                delayed_negative += 1;
                continue;
            };
            let block_usage = matrix[destination].as_slice();
            let target = (current_period + 1..=latest_allowed).rev().find(|&period| {
                fits_in_period(
                    &usage[period],
                    block_usage,
                    &problem.period_resource_upper_limits[period],
                )
            });
            if let Some(period) = target {
                for (resource, amount) in block_usage.iter().enumerate() {
                    usage[current_period][resource] -= amount;
                    usage[period][resource] += amount;
                }
                status[pos] = BlockStatus::Assigned {
                    destination,
                    period,
                    value,
                };
                delayed_negative += 1;
            }
        }
    }

    // Construcción del schedule final.
    let mut assignments: Vec<PcpspToposortAssignment> = Vec::new();
    let mut undiscounted = 0.0_f64;
    let mut discounted = 0.0_f64;
    let mut dropped_for_predecessor = 0usize;
    for pos in 0..n {
        match status[pos] {
            BlockStatus::Assigned {
                destination,
                period,
                value,
            } => {
                undiscounted += value;
                discounted += value / (1.0 + problem.discount_rate).powi(period as i32);
                assignments.push(PcpspToposortAssignment {
                    linear_index: support[pos],
                    destination_index: destination,
                    period_index: period,
                });
            }
            BlockStatus::DroppedPredecessor => dropped_for_predecessor += 1,
            BlockStatus::DroppedCapacity | BlockStatus::Pending => {}
        }
    }
    assignments.sort_by_key(|assignment| (assignment.period_index, assignment.linear_index));

    let mut used_periods = vec![false; problem.period_count];
    let mut used_destinations = vec![false; problem.destination_count];
    for assignment in &assignments {
        used_periods[assignment.period_index] = true;
        used_destinations[assignment.destination_index] = true;
    }

    Ok(PcpspToposortSchedule {
        scheduled_block_count: assignments.len(),
        assignments,
        dropped_for_capacity_count: dropped_for_capacity,
        dropped_for_predecessor_count: dropped_for_predecessor,
        delayed_negative_block_count: delayed_negative,
        undiscounted_objective: undiscounted,
        discounted_objective: discounted,
        period_resource_usage: usage,
        used_period_count: used_periods.iter().filter(|used| **used).count(),
        used_destination_count: used_destinations.iter().filter(|used| **used).count(),
    })
}

fn fits_in_period(current_usage: &[f64], block_usage: &[f64], limits: &[Option<f64>]) -> bool {
    block_usage.iter().enumerate().all(|(resource, amount)| {
        limits[resource].is_none_or(|limit| current_usage[resource] + amount <= limit + 1.0e-9)
    })
}

pub(crate) fn validate_pcpsp_problem(problem: &PcpspToposortProblem) -> Result<(), MineError> {
    if problem.period_count == 0 {
        return Err(MineError::invalid_parameter(
            "period_count",
            "must be greater than zero",
        ));
    }
    if problem.destination_count == 0 {
        return Err(MineError::invalid_parameter(
            "destination_count",
            "must be greater than zero",
        ));
    }
    if !problem.discount_rate.is_finite() || problem.discount_rate < 0.0 {
        return Err(MineError::invalid_parameter(
            "discount_rate",
            "must be finite and non-negative",
        ));
    }
    if problem.block_values.is_empty() {
        return Err(MineError::invalid_parameter(
            "block_values",
            "must contain at least one block value vector",
        ));
    }
    for (linear_index, values) in &problem.block_values {
        if values.len() != problem.destination_count {
            return Err(MineError::invalid_parameter(
                "block_values",
                format!(
                    "block {linear_index} declares {} destination values, expected {}",
                    values.len(),
                    problem.destination_count
                ),
            ));
        }
        for value in values {
            if !value.is_finite() {
                return Err(MineError::invalid_parameter(
                    "block_values",
                    format!("block {linear_index} has a non-finite destination value"),
                ));
            }
        }
    }
    for (linear_index, matrix) in &problem.block_resource_usage {
        if matrix.len() != problem.destination_count {
            return Err(MineError::invalid_parameter(
                "block_resource_usage",
                format!(
                    "block {linear_index} declares {} destination usage rows, expected {}",
                    matrix.len(),
                    problem.destination_count
                ),
            ));
        }
        for row in matrix {
            if row.len() != problem.resource_count {
                return Err(MineError::invalid_parameter(
                    "block_resource_usage",
                    format!(
                        "block {linear_index} declares {} resource entries, expected {}",
                        row.len(),
                        problem.resource_count
                    ),
                ));
            }
            for amount in row {
                if !amount.is_finite() || *amount < 0.0 {
                    return Err(MineError::invalid_parameter(
                        "block_resource_usage",
                        format!("resource usage for block {linear_index} must be finite and >= 0"),
                    ));
                }
            }
        }
    }
    if problem.period_resource_upper_limits.len() != problem.period_count {
        return Err(MineError::invalid_parameter(
            "period_resource_upper_limits",
            format!(
                "expected {} period rows, found {}",
                problem.period_count,
                problem.period_resource_upper_limits.len()
            ),
        ));
    }
    for (period, limits) in problem.period_resource_upper_limits.iter().enumerate() {
        if limits.len() != problem.resource_count {
            return Err(MineError::invalid_parameter(
                "period_resource_upper_limits",
                format!(
                    "period {period} declares {} resource limits, expected {}",
                    limits.len(),
                    problem.resource_count
                ),
            ));
        }
        for limit in limits.iter().flatten() {
            if !limit.is_finite() || *limit < 0.0 {
                return Err(MineError::invalid_parameter(
                    "period_resource_upper_limits",
                    format!("period {period} has a non-finite or negative resource limit"),
                ));
            }
        }
    }
    Ok(())
}
