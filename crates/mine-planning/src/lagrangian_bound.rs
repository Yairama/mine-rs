//! Bound superior Lagrangiano para CPIT/PCPSP con cobertura completa de
//! precedencias (MR-213).
//!
//! Relaja las restricciones de capacidad por periodo con multiplicadores
//! `π_{t,r} >= 0`. El subproblema interno resultante es un **max-closure
//! exacto** sobre el grafo tiempo-expandido (formulación "by-period" con
//! variables acumulativas `y_{b,t}`), que se resuelve con un Dinic denso
//! especializado de índices contiguos: la topología (cadena temporal +
//! precedencias expandidas) se construye una sola vez y entre iteraciones solo
//! cambian las capacidades fuente/sumidero derivadas de los pesos ajustados.
//! Para cualquier `π >= 0`, `L(π)` es un bound superior válido del óptimo
//! entero y del óptimo LP; el dual Lagrangiano coincide con el valor de la
//! relajación LP porque el subproblema interno tiene la propiedad de
//! integralidad (los politopos de clausura son integrales).
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

use std::collections::{BTreeMap, VecDeque};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::pcpsp_toposort::{
    PcpspToposortAssignment, PcpspToposortProblem, validate_pcpsp_problem,
};
use crate::precedence::{PrecedenceGraph, PrecedenceNode};

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
    /// de paso `s_k = µ (L_best − LB) / ‖g_k‖²`. Sin hint se usa `0.0`.
    pub lower_bound_hint: Option<f64>,
    /// Multiplicadores iniciales para warm-start (ej. `best_multipliers` de
    /// una corrida previa). Deben tener exactamente un valor `>= 0` por par
    /// `(periodo, recurso)` acotado, en el mismo orden canónico (periodo
    /// ascendente, recurso ascendente). Sin warm-start se parte de `π = 0`.
    pub initial_multipliers: Option<Vec<f64>>,
}

impl Default for LagrangianBoundOptions {
    fn default() -> Self {
        Self {
            max_iterations: 30,
            initial_step_scale: 1.0,
            step_halving_patience: 2,
            lower_bound_hint: None,
            initial_multipliers: None,
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
    /// Multiplicadores de la mejor iteración, en el orden canónico de pares
    /// `(periodo, recurso)` acotados. Permiten warm-start de corridas futuras.
    pub best_multipliers: Vec<f64>,
}

// ── Dinic denso sobre índices contiguos ──────────────────────────────────────
//
// Nodos: 0 = fuente, 1 = sumidero, 2 + k = nodo expandido k (k = bloque ×
// periodos + periodo). Cada nodo expandido tiene exactamente un arco de fuente
// y uno de sumidero (capacidad 0 cuando no aplica), por lo que la topología es
// estática entre iteraciones y solo se reescriben capacidades.

const SOURCE: usize = 0;
const SINK: usize = 1;
const RESIDUAL_EPSILON: f64 = 1.0e-10;

struct DenseMaxClosure {
    /// Arcos pareados: el arco `e` y su reverso `e ^ 1` son consecutivos.
    edge_to: Vec<u32>,
    edge_cap: Vec<f64>,
    /// CSR: índices de arcos salientes por nodo.
    adjacency_start: Vec<u32>,
    adjacency_edges: Vec<u32>,
    /// Índice del arco fuente→nodo y nodo→sumidero por nodo expandido.
    source_edge_of_node: Vec<u32>,
    sink_edge_of_node: Vec<u32>,
    /// Cantidad de arcos estáticos de capacidad "infinita".
    static_edge_indices: Vec<u32>,
    levels: Vec<i32>,
    next_edge: Vec<u32>,
    queue: VecDeque<usize>,
}

impl DenseMaxClosure {
    /// Construye la topología estática: cadena temporal + precedencias
    /// expandidas + un par fuente/sumidero por nodo expandido.
    fn new(expanded_node_count: usize, static_arcs: &[(u32, u32)]) -> Self {
        let node_count = expanded_node_count + 2;
        let edge_count = 2 * (2 * expanded_node_count + static_arcs.len());
        let mut edge_to = Vec::with_capacity(edge_count);
        let mut edge_cap = Vec::with_capacity(edge_count);
        let mut degree = vec![0u32; node_count];

        let push_edge = |from: usize,
                         to: usize,
                         edge_to: &mut Vec<u32>,
                         edge_cap: &mut Vec<f64>,
                         degree: &mut [u32]|
         -> u32 {
            let index = edge_to.len() as u32;
            edge_to.push(to as u32);
            edge_cap.push(0.0);
            edge_to.push(from as u32);
            edge_cap.push(0.0);
            degree[from] += 1;
            degree[to] += 1;
            index
        };

        let mut source_edge_of_node = Vec::with_capacity(expanded_node_count);
        let mut sink_edge_of_node = Vec::with_capacity(expanded_node_count);
        for node in 0..expanded_node_count {
            source_edge_of_node.push(push_edge(
                SOURCE,
                node + 2,
                &mut edge_to,
                &mut edge_cap,
                &mut degree,
            ));
            sink_edge_of_node.push(push_edge(
                node + 2,
                SINK,
                &mut edge_to,
                &mut edge_cap,
                &mut degree,
            ));
        }
        let mut static_edge_indices = Vec::with_capacity(static_arcs.len());
        for (from, to) in static_arcs {
            static_edge_indices.push(push_edge(
                *from as usize + 2,
                *to as usize + 2,
                &mut edge_to,
                &mut edge_cap,
                &mut degree,
            ));
        }

        // CSR de adyacencia.
        let mut adjacency_start = vec![0u32; node_count + 1];
        for node in 0..node_count {
            adjacency_start[node + 1] = adjacency_start[node] + degree[node];
        }
        let mut cursor = adjacency_start.clone();
        let mut adjacency_edges = vec![0u32; edge_to.len()];
        for edge in 0..edge_to.len() as u32 {
            // El nodo origen del arco `edge` es el destino de su reverso.
            let from = edge_to[(edge ^ 1) as usize] as usize;
            adjacency_edges[cursor[from] as usize] = edge;
            cursor[from] += 1;
        }

        Self {
            edge_to,
            edge_cap,
            adjacency_start,
            adjacency_edges,
            source_edge_of_node,
            sink_edge_of_node,
            static_edge_indices,
            levels: vec![-1; node_count],
            next_edge: vec![0; node_count],
            queue: VecDeque::with_capacity(node_count),
        }
    }

    /// Reescribe capacidades para los pesos actuales y resuelve. Retorna el
    /// valor del cierre máximo y marca `selected` (por nodo expandido).
    fn solve(&mut self, node_weights: &[f64], selected: &mut [bool]) -> f64 {
        let mut sum_positive = 0.0_f64;
        for &weight in node_weights {
            if weight > 0.0 {
                sum_positive += weight;
            }
        }
        let infinite_capacity = sum_positive + 1.0;

        for (node, &weight) in node_weights.iter().enumerate() {
            let source_edge = self.source_edge_of_node[node] as usize;
            let sink_edge = self.sink_edge_of_node[node] as usize;
            self.edge_cap[source_edge] = if weight > 0.0 { weight } else { 0.0 };
            self.edge_cap[source_edge ^ 1] = 0.0;
            self.edge_cap[sink_edge] = if weight < 0.0 { -weight } else { 0.0 };
            self.edge_cap[sink_edge ^ 1] = 0.0;
        }
        for &edge in &self.static_edge_indices {
            self.edge_cap[edge as usize] = infinite_capacity;
            self.edge_cap[(edge ^ 1) as usize] = 0.0;
        }

        // Dinic: niveles BFS + blocking flow DFS iterativo.
        let mut max_flow = 0.0_f64;
        loop {
            // BFS de niveles.
            self.levels.fill(-1);
            self.levels[SOURCE] = 0;
            self.queue.clear();
            self.queue.push_back(SOURCE);
            while let Some(node) = self.queue.pop_front() {
                let start = self.adjacency_start[node] as usize;
                let end = self.adjacency_start[node + 1] as usize;
                for &edge in &self.adjacency_edges[start..end] {
                    let to = self.edge_to[edge as usize] as usize;
                    if self.edge_cap[edge as usize] > RESIDUAL_EPSILON && self.levels[to] < 0 {
                        self.levels[to] = self.levels[node] + 1;
                        self.queue.push_back(to);
                    }
                }
            }
            if self.levels[SINK] < 0 {
                break;
            }

            self.next_edge.fill(0);
            // DFS iterativo de blocking flow.
            let mut path_edges: Vec<u32> = Vec::with_capacity(64);
            let mut node = SOURCE;
            loop {
                if node == SINK {
                    // Empuja el cuello de botella.
                    let mut bottleneck = f64::INFINITY;
                    for &edge in &path_edges {
                        bottleneck = bottleneck.min(self.edge_cap[edge as usize]);
                    }
                    for &edge in &path_edges {
                        self.edge_cap[edge as usize] -= bottleneck;
                        self.edge_cap[(edge ^ 1) as usize] += bottleneck;
                    }
                    max_flow += bottleneck;
                    // Retrocede hasta el primer arco saturado.
                    let mut keep = 0usize;
                    for (depth, &edge) in path_edges.iter().enumerate() {
                        if self.edge_cap[edge as usize] <= RESIDUAL_EPSILON {
                            keep = depth;
                            break;
                        }
                    }
                    path_edges.truncate(keep);
                    node = if keep == 0 {
                        SOURCE
                    } else {
                        self.edge_to[(path_edges[keep - 1]) as usize] as usize
                    };
                    continue;
                }

                let start = self.adjacency_start[node] as usize;
                let end = self.adjacency_start[node + 1] as usize;
                let mut advanced = false;
                while (self.next_edge[node] as usize) < end - start {
                    let edge = self.adjacency_edges[start + self.next_edge[node] as usize];
                    let to = self.edge_to[edge as usize] as usize;
                    if self.edge_cap[edge as usize] > RESIDUAL_EPSILON
                        && self.levels[to] == self.levels[node] + 1
                    {
                        path_edges.push(edge);
                        node = to;
                        advanced = true;
                        break;
                    }
                    self.next_edge[node] += 1;
                }
                if advanced {
                    continue;
                }
                // Sin salida: poda el nivel y retrocede.
                self.levels[node] = -1;
                match path_edges.pop() {
                    Some(edge) => {
                        node = self.edge_to[(edge ^ 1) as usize] as usize;
                    }
                    None => break,
                }
            }
        }

        // Min-cut: alcanzables desde la fuente en el grafo residual.
        selected.fill(false);
        self.levels.fill(-1);
        self.levels[SOURCE] = 0;
        self.queue.clear();
        self.queue.push_back(SOURCE);
        while let Some(node) = self.queue.pop_front() {
            let start = self.adjacency_start[node] as usize;
            let end = self.adjacency_start[node + 1] as usize;
            for &edge in &self.adjacency_edges[start..end] {
                let to = self.edge_to[edge as usize] as usize;
                if self.edge_cap[edge as usize] > RESIDUAL_EPSILON && self.levels[to] < 0 {
                    self.levels[to] = 0;
                    if to >= 2 {
                        selected[to - 2] = true;
                    }
                    self.queue.push_back(to);
                }
            }
        }

        // Identidad de verificación: closure = sum_positive − max_flow.
        let mut closure_value = 0.0_f64;
        for (node, &weight) in node_weights.iter().enumerate() {
            if selected[node] {
                closure_value += weight;
            }
        }
        debug_assert!(
            (closure_value - (sum_positive - max_flow)).abs()
                <= 1.0e-6 * sum_positive.abs().max(1.0),
            "max-closure identity violated: closure {closure_value}, sum_positive {sum_positive}, max_flow {max_flow}"
        );
        closure_value
    }
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
/// las heurísticas TopoSort), si las opciones no son positivas/finitas o si el
/// warm-start no coincide con los pares acotados del problema.
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

    // Topología estática del grafo expandido (se construye una sola vez):
    // cadena temporal y_{b,t} ⇒ y_{b,t+1} y precedencias y_{b,t} ⇒ y_{pred,t}.
    let mut static_arcs: Vec<(u32, u32)> =
        Vec::with_capacity(node_count + expanded_precedence_arc_count);
    for position in 0..blocks.len() {
        for period in 0..period_count.saturating_sub(1) {
            let node = position * period_count + period;
            static_arcs.push((node as u32, (node + 1) as u32));
        }
    }
    for (pred_pos, succ_pos) in &support_edges {
        for period in 0..period_count {
            static_arcs.push((
                (succ_pos * period_count + period) as u32,
                (pred_pos * period_count + period) as u32,
            ));
        }
    }
    let mut dense_solver = DenseMaxClosure::new(node_count, &static_arcs);
    let mut selected = vec![false; node_count];

    // Multiplicadores solo para pares (periodo, recurso) con límite superior.
    let mut bounded_pairs: Vec<(usize, usize, f64)> = Vec::new();
    for (period, limits) in problem.period_resource_upper_limits.iter().enumerate() {
        for (resource, limit) in limits.iter().enumerate() {
            if let Some(limit) = limit {
                bounded_pairs.push((period, resource, *limit));
            }
        }
    }
    let mut multipliers = match &options.initial_multipliers {
        None => vec![0.0_f64; bounded_pairs.len()],
        Some(initial) => {
            if initial.len() != bounded_pairs.len() {
                return Err(MineError::invalid_parameter(
                    "initial_multipliers",
                    format!(
                        "expected {} multipliers for the bounded (period, resource) pairs, got {}",
                        bounded_pairs.len(),
                        initial.len()
                    ),
                ));
            }
            for value in initial {
                if !value.is_finite() || *value < 0.0 {
                    return Err(MineError::invalid_parameter(
                        "initial_multipliers",
                        "multipliers must be finite and non-negative",
                    ));
                }
            }
            initial.clone()
        }
    };
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
    let mut best_multipliers = multipliers.clone();
    let mut iteration_records: Vec<LagrangianIterationRecord> =
        Vec::with_capacity(options.max_iterations);

    let mut adjusted_values = vec![0.0_f64; node_count];
    let mut chosen_destination = vec![0usize; node_count];
    let mut node_weights = vec![0.0_f64; node_count];

    for iteration in 0..options.max_iterations {
        // 1. Valores ajustados por multiplicadores y mejor destino por (b, t).
        for (position, linear) in blocks.iter().enumerate() {
            let values = &problem.block_values[linear];
            let usage_matrix = problem.block_resource_usage.get(linear);
            for (period, &discount_factor) in discount_factors.iter().enumerate() {
                let mut best_value = f64::NEG_INFINITY;
                let mut best_destination = 0usize;
                for (destination, &value) in values.iter().enumerate() {
                    let mut adjusted = value * discount_factor;
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

        // 2. Pesos telescopados del max-closure tiempo-expandido:
        //    w(b,t) = v(b,t) − v(b,t+1); w(b,T−1) = v(b,T−1).
        for position in 0..blocks.len() {
            for period in 0..period_count {
                let node = position * period_count + period;
                node_weights[node] = if period + 1 < period_count {
                    adjusted_values[node] - adjusted_values[node + 1]
                } else {
                    adjusted_values[node]
                };
            }
        }

        let closure_value = dense_solver.solve(&node_weights, &mut selected);

        // 3. Decodificación: el cierre sobre la cadena temporal es un sufijo
        //    [t*, T-1]; el bloque se extrae en t* con el destino elegido.
        let mut inner_assignments: Vec<PcpspToposortAssignment> = Vec::new();
        for (position, linear) in blocks.iter().enumerate() {
            let base = position * period_count;
            for period in 0..period_count {
                if selected[base + period] {
                    inner_assignments.push(PcpspToposortAssignment {
                        linear_index: *linear,
                        destination_index: chosen_destination[base + period],
                        period_index: period,
                    });
                    break;
                }
            }
        }

        // 4. Bound L(π) = Σ π·Q + valor del cierre.
        let multiplier_term: f64 = bounded_pairs
            .iter()
            .enumerate()
            .map(|(index, (_, _, limit))| multipliers[index] * limit)
            .sum();
        let bound = multiplier_term + closure_value;

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
            best_multipliers.clone_from(&multipliers);
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
        best_multipliers,
    })
}
