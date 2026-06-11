//! Bound superior Lagrangiano para CPIT/PCPSP con cobertura completa de
//! precedencias (MR-213).
//!
//! Relaja las restricciones de capacidad por periodo con multiplicadores
//! `π_{t,r} >= 0`. El subproblema interno resultante es un **max-closure
//! exacto** sobre el grafo tiempo-expandido (formulación "by-period" con
//! variables acumulativas `y_{b,t}`), que se resuelve con el mismo backend
//! Dinic del UPL. Para cualquier `π >= 0`, `L(π)` es un bound superior válido
//! del óptimo entero y del óptimo LP; el dual Lagrangiano coincide con el
//! valor de la relajación LP porque el subproblema interno tiene la propiedad
//! de integralidad (los politopos de clausura son integrales).
//!
//! Referencias:
//!
//! - Geoffrion (1974), "Lagrangean relaxation for integer programming",
//!   Mathematical Programming Study 2:82-114. doi 10.1007/BFb0120690
//!   (literatura académica; dual Lagrangiano = bound LP bajo propiedad de
//!   integralidad del subproblema).
//! - Dagdelen & Johnson (1986), "Optimum open pit mine production scheduling
//!   by Lagrangian parameterization", Proc. 19th APCOM, 127-142 (literatura
//!   de práctica/conferencia; primera aplicación minera de esta relajación).
//! - Lambert et al. ([R23], doi 10.1287/inte.2013.0731) y Bienstock &
//!   Zuckerberg ([R34] vía Muñoz et al., doi 10.1007/s10589-017-9946-1):
//!   contexto de formulaciones CPIT/PCPSP y del algoritmo BZ que resuelve la
//!   misma relajación LP por descomposición especializada; esta ruta
//!   Lagrangiana alcanza el mismo valor de bound en el límite, con cobertura
//!   del 100% de las precedencias en cada iteración.
//!
//! # Multi-destino
//!
//! Con capacidades relajadas, la elección de destino se separa por bloque y
//! periodo: `v_{b,t} = max_d (value_{b,d} · δ^t − Σ_r π_{t,r} · q_{b,d,r})`.
//! CPIT es el caso particular con un destino.

use std::collections::BTreeMap;

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::max_closure::{MaxClosureArc, MaxClosureGraph, MaxClosureNodeId};
use crate::pcpsp_toposort::{
    PcpspToposortAssignment, PcpspToposortProblem, validate_pcpsp_problem,
};
use crate::precedence::{PrecedenceGraph, PrecedenceNode};
use crate::upl_solver::solve_upl_exact;

// ── Contratos ────────────────────────────────────────────────────────────────

/// Opciones del método de subgradiente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LagrangianBoundOptions {
    /// Número de iteraciones de subgradiente (cada una resuelve un max-closure
    /// tiempo-expandido completo).
    pub max_iterations: usize,
    /// Escala inicial del paso de subgradiente (clásico `µ` de Held-Karp).
    pub initial_step_scale: f64,
    /// Iteraciones consecutivas sin mejora antes de reducir `µ` a la mitad.
    pub step_halving_patience: usize,
    /// Cota inferior conocida (ej. un candidato factible) usada por la regla
    /// de paso `s_k = µ (L(π_k) − LB) / ‖g_k‖²`. Sin hint se usa `0.0`.
    pub lower_bound_hint: Option<f64>,
}

impl Default for LagrangianBoundOptions {
    fn default() -> Self {
        Self {
            max_iterations: 30,
            initial_step_scale: 1.0,
            step_halving_patience: 2,
            lower_bound_hint: None,
        }
    }
}

/// Registro por iteración del subgradiente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LagrangianIterationRecord {
    /// Índice de iteración (0-based).
    pub iteration: usize,
    /// Bound `L(π)` de la iteración.
    pub bound: f64,
    /// Escala de paso vigente.
    pub step_scale: f64,
    /// Máxima violación absoluta de capacidad del subproblema interno.
    pub max_capacity_violation: f64,
}

/// Resultado del bound Lagrangiano.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LagrangianBoundResult {
    /// Mejor (menor) bound superior válido encontrado.
    pub best_bound: f64,
    /// Iteración donde se alcanzó el mejor bound.
    pub best_iteration: usize,
    /// Traza por iteración (auditable y determinista).
    pub iteration_records: Vec<LagrangianIterationRecord>,
    /// Cantidad de multiplicadores activos (pares periodo × recurso acotados).
    pub multiplier_count: usize,
    /// Nodos del grafo tiempo-expandido (bloques × periodos).
    pub expanded_node_count: usize,
    /// Arcos de precedencia tiempo-expandidos (cobertura 100% por iteración).
    pub expanded_precedence_arc_count: usize,
    /// Solución interna de la mejor iteración (puede violar capacidades: es
    /// una relajación). Útil como fuente self-contained de scores de orden
    /// para las heurísticas TopoSort.
    pub best_inner_assignments: Vec<PcpspToposortAssignment>,
}

// ── Cómputo ──────────────────────────────────────────────────────────────────

/// Calcula un bound superior Lagrangiano para un `PcpspToposortProblem`.
///
/// Cada iteración resuelve el subproblema interno exacto con **todas** las
/// precedencias (sin checkpoints parciales) y actualiza los multiplicadores
/// por subgradiente proyectado. El bound reportado es el mínimo sobre las
/// iteraciones, válido para el óptimo entero y para la relajación LP.
///
/// # Errores
///
/// Retorna error si el problema está mal dimensionado (misma validación que
/// las heurísticas TopoSort) o si las opciones no son positivas/finitas.
pub fn compute_pcpsp_lagrangian_bound(
    problem: &PcpspToposortProblem,
    precedence_graph: &PrecedenceGraph,
    options: &LagrangianBoundOptions,
) -> Result<LagrangianBoundResult, MineError> {
    validate_pcpsp_problem(problem)?;
    if options.max_iterations == 0 {
        return Err(MineError::invalid_parameter(
            "max_iterations",
            "must be greater than zero",
        ));
    }
    if !options.initial_step_scale.is_finite() || options.initial_step_scale <= 0.0 {
        return Err(MineError::invalid_parameter(
            "initial_step_scale",
            "must be finite and greater than zero",
        ));
    }
    if let Some(hint) = options.lower_bound_hint
        && !hint.is_finite()
    {
        return Err(MineError::invalid_parameter(
            "lower_bound_hint",
            "must be finite when provided",
        ));
    }

    let period_count = problem.period_count;
    let blocks: Vec<usize> = problem.block_values.keys().copied().collect();
    let block_position: BTreeMap<usize, usize> = blocks
        .iter()
        .enumerate()
        .map(|(position, linear)| (*linear, position))
        .collect();
    let node_count = blocks.len() * period_count;

    // Precedencias bloque→bloque restringidas al soporte del problema,
    // expresadas como posiciones contiguas. Cobertura completa: toda arista
    // con ambos extremos en el problema se expande a los `period_count` arcos.
    let support_edges: Vec<(usize, usize)> = precedence_graph
        .edges()
        .iter()
        .filter_map(|edge| {
            let (PrecedenceNode::Block(pred_idx), PrecedenceNode::Block(succ_idx)) =
                (edge.predecessor(), edge.successor())
            else {
                return None;
            };
            match (block_position.get(pred_idx), block_position.get(succ_idx)) {
                (Some(&pred_pos), Some(&succ_pos)) => Some((pred_pos, succ_pos)),
                _ => None,
            }
        })
        .collect();
    let expanded_precedence_arc_count = support_edges.len() * period_count;

    // Multiplicadores solo para pares (periodo, recurso) con límite superior.
    let mut bounded_pairs: Vec<(usize, usize, f64)> = Vec::new();
    for (period, limits) in problem.period_resource_upper_limits.iter().enumerate() {
        for (resource, limit) in limits.iter().enumerate() {
            if let Some(limit) = limit {
                bounded_pairs.push((period, resource, *limit));
            }
        }
    }
    let mut multipliers = vec![0.0_f64; bounded_pairs.len()];
    // Índice (periodo, recurso) → posición del multiplicador.
    let multiplier_position: BTreeMap<(usize, usize), usize> = bounded_pairs
        .iter()
        .enumerate()
        .map(|(position, (period, resource, _))| ((*period, *resource), position))
        .collect();

    let discount_factors: Vec<f64> = (0..period_count)
        .map(|period| 1.0 / (1.0 + problem.discount_rate).powi(period as i32))
        .collect();

    let lower_bound = options.lower_bound_hint.unwrap_or(0.0);
    let mut step_scale = options.initial_step_scale;
    let mut non_improving_streak = 0usize;
    let mut best_bound = f64::INFINITY;
    let mut best_iteration = 0usize;
    let mut best_inner_assignments: Vec<PcpspToposortAssignment> = Vec::new();
    let mut iteration_records: Vec<LagrangianIterationRecord> =
        Vec::with_capacity(options.max_iterations);

    for iteration in 0..options.max_iterations {
        // 1. Valores ajustados por multiplicadores y mejor destino por (b, t).
        //    v[b][t] = max_d (value_{b,d} · δ^t − Σ_r π_{t,r} q_{b,d,r})
        let mut adjusted_values = vec![0.0_f64; node_count];
        let mut chosen_destination = vec![0usize; node_count];
        for (position, linear) in blocks.iter().enumerate() {
            let values = &problem.block_values[linear];
            let usage_matrix = problem.block_resource_usage.get(linear);
            for period in 0..period_count {
                let mut best_value = f64::NEG_INFINITY;
                let mut best_destination = 0usize;
                for (destination, &value) in values.iter().enumerate() {
                    let mut adjusted = value * discount_factors[period];
                    if let Some(matrix) = usage_matrix {
                        for (resource, amount) in matrix[destination].iter().enumerate() {
                            if let Some(&multiplier_index) =
                                multiplier_position.get(&(period, resource))
                            {
                                adjusted -= multipliers[multiplier_index] * amount;
                            }
                        }
                    }
                    if adjusted > best_value {
                        best_value = adjusted;
                        best_destination = destination;
                    }
                }
                let node = position * period_count + period;
                adjusted_values[node] = best_value;
                chosen_destination[node] = best_destination;
            }
        }

        // 2. Max-closure tiempo-expandido con pesos telescopados:
        //    w(b,t) = v(b,t) − v(b,t+1); w(b,T−1) = v(b,T−1).
        let mut node_weights: BTreeMap<usize, f64> = BTreeMap::new();
        for position in 0..blocks.len() {
            for period in 0..period_count {
                let node = position * period_count + period;
                let weight = if period + 1 < period_count {
                    adjusted_values[node] - adjusted_values[node + 1]
                } else {
                    adjusted_values[node]
                };
                node_weights.insert(node, weight);
            }
        }

        let mut arcs: Vec<MaxClosureArc> =
            Vec::with_capacity(node_count + expanded_precedence_arc_count + node_count);
        let mut sum_positive = 0.0_f64;
        let mut sum_negative = 0.0_f64;
        for (&node, &weight) in &node_weights {
            if weight > 0.0 {
                sum_positive += weight;
                arcs.push(MaxClosureArc {
                    from: MaxClosureNodeId::Source,
                    to: MaxClosureNodeId::Block(node),
                    capacity: weight,
                });
            } else if weight < 0.0 {
                sum_negative += weight;
                arcs.push(MaxClosureArc {
                    from: MaxClosureNodeId::Block(node),
                    to: MaxClosureNodeId::Sink,
                    capacity: weight.abs(),
                });
            }
        }
        // Cadena temporal: y_{b,t} = 1 ⇒ y_{b,t+1} = 1.
        for position in 0..blocks.len() {
            for period in 0..period_count.saturating_sub(1) {
                let node = position * period_count + period;
                arcs.push(MaxClosureArc {
                    from: MaxClosureNodeId::Block(node),
                    to: MaxClosureNodeId::Block(node + 1),
                    capacity: f64::INFINITY,
                });
            }
        }
        // Precedencias por periodo: y_{b,t} = 1 ⇒ y_{pred,t} = 1.
        for (pred_pos, succ_pos) in &support_edges {
            for period in 0..period_count {
                arcs.push(MaxClosureArc {
                    from: MaxClosureNodeId::Block(succ_pos * period_count + period),
                    to: MaxClosureNodeId::Block(pred_pos * period_count + period),
                    capacity: f64::INFINITY,
                });
            }
        }

        let closure_graph = MaxClosureGraph {
            block_count: node_count,
            block_weights: node_weights,
            arcs,
            sum_positive_weights: sum_positive,
            sum_negative_weights: sum_negative,
        };
        let inner = solve_upl_exact(&closure_graph)?;

        // 3. Decodificación: el cierre sobre la cadena temporal es un sufijo
        //    [t*, T-1]; el bloque se extrae en t* con el destino elegido.
        let mut earliest_selected: BTreeMap<usize, usize> = BTreeMap::new();
        for node in &inner.selected_blocks {
            let position = node / period_count;
            let period = node % period_count;
            earliest_selected
                .entry(position)
                .and_modify(|current| *current = (*current).min(period))
                .or_insert(period);
        }
        let mut inner_assignments: Vec<PcpspToposortAssignment> =
            Vec::with_capacity(earliest_selected.len());
        for (&position, &period) in &earliest_selected {
            let node = position * period_count + period;
            inner_assignments.push(PcpspToposortAssignment {
                linear_index: blocks[position],
                destination_index: chosen_destination[node],
                period_index: period,
            });
        }

        // 4. Bound L(π) = Σ π·Q + valor del cierre.
        let multiplier_term: f64 = bounded_pairs
            .iter()
            .enumerate()
            .map(|(index, (_, _, limit))| multipliers[index] * limit)
            .sum();
        let bound = multiplier_term + inner.pit_value;

        // 5. Subgradiente: g_{t,r} = uso interno − límite.
        let mut usage = vec![vec![0.0_f64; problem.resource_count]; period_count];
        for assignment in &inner_assignments {
            if let Some(matrix) = problem.block_resource_usage.get(&assignment.linear_index) {
                for (resource, amount) in matrix[assignment.destination_index].iter().enumerate() {
                    usage[assignment.period_index][resource] += amount;
                }
            }
        }
        let mut gradient = vec![0.0_f64; bounded_pairs.len()];
        let mut gradient_norm_squared = 0.0_f64;
        let mut max_violation = 0.0_f64;
        for (index, (period, resource, limit)) in bounded_pairs.iter().enumerate() {
            let violation = usage[*period][*resource] - limit;
            gradient[index] = violation;
            gradient_norm_squared += violation * violation;
            max_violation = max_violation.max(violation.max(0.0));
        }

        if bound < best_bound - 1.0e-9 {
            best_bound = bound;
            best_iteration = iteration;
            best_inner_assignments = inner_assignments;
            non_improving_streak = 0;
        } else {
            non_improving_streak += 1;
            if non_improving_streak >= options.step_halving_patience {
                step_scale *= 0.5;
                non_improving_streak = 0;
            }
        }

        iteration_records.push(LagrangianIterationRecord {
            iteration,
            bound,
            step_scale,
            max_capacity_violation: max_violation,
        });

        // Subproblema factible respecto a capacidades y sin violaciones: el
        // bound coincide con un schedule factible ⇒ no hay gap dual restante.
        if gradient_norm_squared <= 1.0e-18 {
            break;
        }

        // 6. Paso proyectado de subgradiente. Se usa el mejor bound conocido
        //    (no el de la iteración) en la regla de Held-Karp para amortiguar
        //    la oscilación de las primeras iteraciones.
        let step = step_scale * (best_bound - lower_bound).max(0.0) / gradient_norm_squared;
        for (index, gradient_value) in gradient.iter().enumerate() {
            multipliers[index] = (multipliers[index] + step * gradient_value).max(0.0);
        }
    }

    Ok(LagrangianBoundResult {
        best_bound,
        best_iteration,
        iteration_records,
        multiplier_count: bounded_pairs.len(),
        expanded_node_count: node_count,
        expanded_precedence_arc_count,
        best_inner_assignments,
    })
}
