//! Heurística determinista tipo TopoSort para CPIT a nivel de bloque.
//!
//! Implementa una variante del rounding "TopoSort" descrito en:
//!
//! - Chicoisne, Espinoza, Goycoolea, Moreno, Rubio (2012),
//!   "A New Algorithm for the Open-Pit Mine Production Scheduling Problem",
//!   Operations Research 60(3):517-528. doi 10.1287/opre.1120.1072 (literatura
//!   académica revisada por pares; referencia [R35] del roadmap).
//!
//! La idea central del paper es ordenar los bloques de forma compatible con la
//! topología del DAG de precedencias usando un score externo (típicamente el
//! tiempo esperado de extracción derivado de una relajación LP) y luego empacar
//! los bloques en periodos respetando capacidades por recurso.
//!
//! Este módulo NO calcula la relajación LP: recibe los scores como input
//! explícito (`ordering_scores`). Eso mantiene el core determinista y permite
//! alimentar el score desde una relajación LP propia (objetivo MR-213), desde
//! artefactos LP abiertos de MineLib, o desde cualquier política auditable.
//!
//! # Contratos
//!
//! - Solo se programan bloques presentes en `ordering_scores` (el "soporte").
//! - Si un bloque del soporte tiene un predecesor fuera del soporte, se descarta
//!   junto con sus sucesores (la clausura debe ser válida).
//! - Solo se refuerzan límites superiores de recursos (`upper limits`); el
//!   contrato no representa cotas inferiores por periodo.
//! - El descuento usa `1 / (1 + discount_rate)^period_index` con periodos
//!   0-based, igual que el auditor benchmark de soluciones MineLib del repo.

use std::cmp::Ordering as CmpOrdering;
use std::collections::{BTreeMap, BinaryHeap};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::precedence::{PrecedenceGraph, PrecedenceNode};

// ── Contratos de entrada ─────────────────────────────────────────────────────

/// Problema CPIT a nivel de bloque para la heurística TopoSort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpitToposortProblem {
    /// Número de periodos disponibles.
    pub period_count: usize,
    /// Tasa de descuento por periodo (>= 0).
    pub discount_rate: f64,
    /// Número de recursos con límite por periodo.
    pub resource_count: usize,
    /// Valor económico no descontado por bloque (índice lineal → valor).
    pub block_values: BTreeMap<usize, f64>,
    /// Consumo de recursos por bloque (índice lineal → vector de largo `resource_count`).
    pub block_resource_usage: BTreeMap<usize, Vec<f64>>,
    /// Límites superiores por periodo y recurso (`None` = sin límite).
    /// Dimensiones: `[period_count][resource_count]`.
    pub period_resource_upper_limits: Vec<Vec<Option<f64>>>,
}

/// Opciones explícitas de la heurística.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpitToposortOptions {
    /// Si es `true`, aplica un post-pass determinista que retrasa bloques de
    /// valor negativo hasta el periodo más tardío factible (sin violar
    /// precedencias ni capacidades) para mejorar el NPV.
    pub delay_negative_blocks: bool,
}

impl Default for CpitToposortOptions {
    fn default() -> Self {
        Self {
            delay_negative_blocks: true,
        }
    }
}

// ── Contratos de salida ──────────────────────────────────────────────────────

/// Asignación bloque → periodo producida por la heurística.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpitToposortAssignment {
    /// Índice lineal del bloque asignado.
    pub linear_index: usize,
    /// Periodo asignado (0-based).
    pub period_index: usize,
}

/// Schedule CPIT serializable producido por la heurística TopoSort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpitToposortSchedule {
    /// Asignaciones bloque → periodo ordenadas por (periodo, bloque).
    pub assignments: Vec<CpitToposortAssignment>,
    /// Cantidad de bloques programados.
    pub scheduled_block_count: usize,
    /// Bloques del soporte descartados por falta de capacidad.
    pub dropped_for_capacity_count: usize,
    /// Bloques descartados porque un predecesor (directo o transitivo) quedó
    /// fuera del soporte o fue descartado.
    pub dropped_for_predecessor_count: usize,
    /// Bloques de valor negativo retrasados por el post-pass.
    pub delayed_negative_block_count: usize,
    /// Objetivo no descontado del schedule.
    pub undiscounted_objective: f64,
    /// Objetivo descontado con `1/(1+r)^periodo`.
    pub discounted_objective: f64,
    /// Uso de recursos por periodo: `[period_count][resource_count]`.
    pub period_resource_usage: Vec<Vec<f64>>,
    /// Cantidad de periodos con al menos un bloque asignado.
    pub used_period_count: usize,
}

// ── Orden topológico guiado por score ────────────────────────────────────────

/// Clave de orden determinista: menor score primero, desempate por índice.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ReadyBlock {
    score: f64,
    linear_index: usize,
}

impl Eq for ReadyBlock {}

impl Ord for ReadyBlock {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // BinaryHeap es max-heap: invertimos para extraer el menor score primero.
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| other.linear_index.cmp(&self.linear_index))
    }
}

impl PartialOrd for ReadyBlock {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockStatus {
    Pending,
    Assigned(usize),
    DroppedCapacity,
    DroppedPredecessor,
}

// ── Solver ───────────────────────────────────────────────────────────────────

/// Resuelve CPIT con la heurística TopoSort guiada por `ordering_scores`.
///
/// # Parámetros
///
/// - `problem`: contrato CPIT a nivel de bloque (valores, recursos, límites).
/// - `precedence_graph`: DAG de precedencias; solo se procesan aristas
///   bloque → bloque.
/// - `ordering_scores`: score por bloque (menor = antes). Define además el
///   soporte de bloques candidatos a extracción; típicamente el tiempo esperado
///   de extracción de una relajación LP (Chicoisne et al. 2012, [R35]).
/// - `options`: opciones explícitas de la heurística.
///
/// # Errores
///
/// Retorna error si el problema está mal dimensionado, si los scores referencian
/// bloques sin valor declarado o si algún insumo numérico no es finito.
pub fn solve_cpit_with_toposort(
    problem: &CpitToposortProblem,
    precedence_graph: &PrecedenceGraph,
    ordering_scores: &BTreeMap<usize, f64>,
    options: &CpitToposortOptions,
) -> Result<CpitToposortSchedule, MineError> {
    validate_problem(problem)?;
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
                format!("ordering score references block {linear_index} without declared value"),
            ));
        }
    }

    // Mapeo soporte → índice contiguo para estructuras densas.
    let support: Vec<usize> = ordering_scores.keys().copied().collect();
    let position: BTreeMap<usize, usize> = support
        .iter()
        .enumerate()
        .map(|(pos, linear)| (*linear, pos))
        .collect();
    let n = support.len();

    // Adyacencia restringida al soporte. Una arista cuyo predecesor está fuera
    // del soporte invalida al sucesor (y a su descendencia).
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
                // Predecesor fuera del soporte: el sucesor no puede minarse.
                status[succ_pos] = BlockStatus::DroppedPredecessor;
            }
        }
    }

    // Kahn guiado por score: solo los bloques con todos sus predecesores
    // procesados quedan listos; el heap garantiza orden determinista.
    let mut heap: BinaryHeap<ReadyBlock> = BinaryHeap::new();
    for (pos, &linear) in support.iter().enumerate() {
        if indegree[pos] == 0 {
            heap.push(ReadyBlock {
                score: ordering_scores[&linear],
                linear_index: pos,
            });
        }
    }

    let mut usage = vec![vec![0.0_f64; problem.resource_count]; problem.period_count];
    let zero_usage = vec![0.0_f64; problem.resource_count];
    let mut pop_order: Vec<usize> = Vec::with_capacity(n);
    let mut dropped_for_capacity = 0usize;

    while let Some(ready) = heap.pop() {
        let pos = ready.linear_index;
        pop_order.push(pos);
        let linear = support[pos];

        let mut earliest_period = 0usize;
        let mut blocked_by_predecessor = status[pos] == BlockStatus::DroppedPredecessor;
        for &pred_pos in &preds[pos] {
            match status[pred_pos] {
                BlockStatus::Assigned(period) => {
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
            let block_usage = problem
                .block_resource_usage
                .get(&linear)
                .unwrap_or(&zero_usage);
            let assigned = (earliest_period..problem.period_count).find(|&period| {
                fits_in_period(
                    &usage[period],
                    block_usage,
                    &problem.period_resource_upper_limits[period],
                )
            });
            match assigned {
                Some(period) => {
                    for (resource, amount) in block_usage.iter().enumerate() {
                        usage[period][resource] += amount;
                    }
                    status[pos] = BlockStatus::Assigned(period);
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
                    linear_index: succ_pos,
                });
            }
        }
    }

    // Bloques nunca liberados por Kahn (ancestro fuera de soporte en ciclo de
    // descarte) cuentan como descartes por predecesor.
    for block_status in &mut status {
        if *block_status == BlockStatus::Pending {
            *block_status = BlockStatus::DroppedPredecessor;
        }
    }

    // Post-pass: retrasar bloques negativos al periodo más tardío factible.
    let mut delayed_negative = 0usize;
    if options.delay_negative_blocks {
        for &pos in pop_order.iter().rev() {
            let BlockStatus::Assigned(current_period) = status[pos] else {
                continue;
            };
            let linear = support[pos];
            let value = problem.block_values[&linear];
            if value >= 0.0 {
                continue;
            }

            let mut latest_allowed = problem.period_count - 1;
            for &succ_pos in &succs[pos] {
                if let BlockStatus::Assigned(succ_period) = status[succ_pos] {
                    latest_allowed = latest_allowed.min(succ_period);
                }
            }
            if latest_allowed <= current_period {
                continue;
            }

            let block_usage = problem
                .block_resource_usage
                .get(&linear)
                .unwrap_or(&zero_usage);
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
                status[pos] = BlockStatus::Assigned(period);
                delayed_negative += 1;
            }
        }
    }

    // Construcción del schedule final.
    let mut assignments: Vec<CpitToposortAssignment> = Vec::new();
    let mut undiscounted = 0.0_f64;
    let mut discounted = 0.0_f64;
    let mut dropped_for_predecessor = 0usize;
    for pos in 0..n {
        match status[pos] {
            BlockStatus::Assigned(period) => {
                let linear = support[pos];
                let value = problem.block_values[&linear];
                undiscounted += value;
                discounted += value / (1.0 + problem.discount_rate).powi(period as i32);
                assignments.push(CpitToposortAssignment {
                    linear_index: linear,
                    period_index: period,
                });
            }
            BlockStatus::DroppedPredecessor => dropped_for_predecessor += 1,
            BlockStatus::DroppedCapacity | BlockStatus::Pending => {}
        }
    }
    assignments.sort_by_key(|assignment| (assignment.period_index, assignment.linear_index));

    let used_period_count = (0..problem.period_count)
        .filter(|&period| {
            assignments
                .iter()
                .any(|assignment| assignment.period_index == period)
        })
        .count();

    Ok(CpitToposortSchedule {
        scheduled_block_count: assignments.len(),
        assignments,
        dropped_for_capacity_count: dropped_for_capacity,
        dropped_for_predecessor_count: dropped_for_predecessor,
        delayed_negative_block_count: delayed_negative,
        undiscounted_objective: undiscounted,
        discounted_objective: discounted,
        period_resource_usage: usage,
        used_period_count,
    })
}

fn fits_in_period(current_usage: &[f64], block_usage: &[f64], limits: &[Option<f64>]) -> bool {
    block_usage.iter().enumerate().all(|(resource, amount)| {
        limits[resource].is_none_or(|limit| current_usage[resource] + amount <= limit + 1.0e-9)
    })
}

fn validate_problem(problem: &CpitToposortProblem) -> Result<(), MineError> {
    if problem.period_count == 0 {
        return Err(MineError::invalid_parameter(
            "period_count",
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
            "must contain at least one block value",
        ));
    }
    for (linear_index, value) in &problem.block_values {
        if !value.is_finite() {
            return Err(MineError::invalid_parameter(
                "block_values",
                format!("block value for index {linear_index} is not finite"),
            ));
        }
    }
    for (linear_index, usage) in &problem.block_resource_usage {
        if usage.len() != problem.resource_count {
            return Err(MineError::invalid_parameter(
                "block_resource_usage",
                format!(
                    "resource usage for block {linear_index} has {} entries, expected {}",
                    usage.len(),
                    problem.resource_count
                ),
            ));
        }
        for amount in usage {
            if !amount.is_finite() || *amount < 0.0 {
                return Err(MineError::invalid_parameter(
                    "block_resource_usage",
                    format!("resource usage for block {linear_index} must be finite and >= 0"),
                ));
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
