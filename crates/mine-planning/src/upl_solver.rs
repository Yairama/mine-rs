//! Solver exacto de Ultimate Pit Limit (UPL) basado en max-flow.
//!
//! Implementa el algoritmo de Edmonds-Karp (Ford-Fulkerson con BFS) para
//! resolver el problema de max-closure / min-cut derivado de `MaxClosureGraph`.
//!
//! # Complejidad
//!
//! Edmonds-Karp tiene complejidad O(VE²). Para modelos con miles de bloques
//! y grafos de precedencia densos, se recomienda migrar a push-relabel
//! (O(V²√E)) en trabajos futuros.
//!
//! # Resultado
//!
//! El solver determina:
//! - El conjunto de bloques óptimos a extraer (el pit final).
//! - El valor económico total del pit (suma de pesos de bloques seleccionados).
//! - El max-flow (igual al costo del min-cut).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::max_closure::{MaxClosureGraph, MaxClosureNodeId};

// ── Resultado del solver ──────────────────────────────────────────────────────

/// Resultado del solver exacto de UPL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UplSolverResult {
    /// Bloques seleccionados para extracción (índices lineales).
    pub selected_blocks: Vec<usize>,
    /// Valor económico total del pit (suma de block_value de los bloques seleccionados).
    pub pit_value: f64,
    /// Valor del max-flow (igual al costo del min-cut).
    pub max_flow_value: f64,
    /// Suma de pesos positivos (cota superior teórica).
    pub upper_bound: f64,
    /// Número de bloques seleccionados.
    pub selected_block_count: usize,
    /// Número total de bloques en el modelo.
    pub total_block_count: usize,
}

// ── Estructura interna del grafo de flujo ─────────────────────────────────────

/// Resuelve el UPL exacto usando max-flow de Edmonds-Karp sobre `MaxClosureGraph`.
///
/// # Nota sobre capacidad "infinita"
///
/// Los arcos de precedencia tienen capacidad `f64::INFINITY`. Internamente se
/// reemplazan por una cota superior finita suficientemente grande:
/// `sum_positive_weights + 1.0` asegura que estos arcos nunca sean saturados
/// por el min-cut.
///
/// # Errores
///
/// Retorna error si `closure_graph` tiene 0 bloques (sin pesos).
pub fn solve_upl_exact(closure_graph: &MaxClosureGraph) -> Result<UplSolverResult, MineError> {
    if closure_graph.block_count == 0 {
        return Err(MineError::invalid_parameter(
            "closure_graph",
            "UPL solver requires at least one block",
        ));
    }

    // Capacidad sustituta para arcos de precedencia "infinita"
    let inf_capacity = closure_graph.sum_positive_weights + 1.0;

    // Mapeo de MaxClosureNodeId a índices contiguos
    let mut node_ids: Vec<MaxClosureNodeId> = Vec::new();
    let mut node_map: BTreeMap<String, usize> = BTreeMap::new();

    let source_key = "source".to_owned();
    let sink_key = "sink".to_owned();

    node_ids.push(MaxClosureNodeId::Source);
    node_map.insert(source_key.clone(), 0);
    node_ids.push(MaxClosureNodeId::Sink);
    node_map.insert(sink_key.clone(), 1);

    for (&linear_index, _) in &closure_graph.block_weights {
        let key = format!("block:{linear_index}");
        if !node_map.contains_key(&key) {
            let idx = node_ids.len();
            node_ids.push(MaxClosureNodeId::Block(linear_index));
            node_map.insert(key, idx);
        }
    }

    let n = node_ids.len();
    let source = 0_usize;
    let sink = 1_usize;

    let node_key = |id: &MaxClosureNodeId| -> String {
        match id {
            MaxClosureNodeId::Source => "source".to_owned(),
            MaxClosureNodeId::Sink => "sink".to_owned(),
            MaxClosureNodeId::Block(i) => format!("block:{i}"),
        }
    };

    // Build residual graph using adjacency lists with arc indices
    // Each arc stores (to, capacity, rev_arc_index)
    let mut graph: Vec<Vec<(usize, f64, usize)>> = vec![Vec::new(); n];

    let add_arc = |graph: &mut Vec<Vec<(usize, f64, usize)>>, u: usize, v: usize, cap: f64| {
        let u_len = graph[u].len();
        let v_len = graph[v].len();
        graph[u].push((v, cap, v_len)); // forward arc
        graph[v].push((u, 0.0, u_len)); // backward arc (residual)
    };

    for arc in &closure_graph.arcs {
        let from_key = node_key(&arc.from);
        let to_key = node_key(&arc.to);
        let from_idx = node_map[&from_key];
        let to_idx = node_map[&to_key];
        let cap = if arc.capacity.is_infinite() {
            inf_capacity
        } else {
            arc.capacity
        };
        add_arc(&mut graph, from_idx, to_idx, cap);
    }

    // Edmonds-Karp max-flow
    let mut total_flow = 0.0_f64;
    loop {
        // BFS to find augmenting path
        let mut parent: Vec<Option<(usize, usize)>> = vec![None; n]; // (from_node, arc_index)
        let mut visited = vec![false; n];
        visited[source] = true;
        let mut queue = VecDeque::new();
        queue.push_back(source);

        while let Some(u) = queue.pop_front() {
            if u == sink {
                break;
            }
            for (arc_idx, &(v, cap, _)) in graph[u].iter().enumerate() {
                if !visited[v] && cap > 1e-10 {
                    visited[v] = true;
                    parent[v] = Some((u, arc_idx));
                    queue.push_back(v);
                    if v == sink {
                        break;
                    }
                }
            }
        }

        if !visited[sink] {
            break; // no augmenting path
        }

        // Find bottleneck capacity along the path
        let mut bottleneck = f64::INFINITY;
        let mut node = sink;
        while let Some((prev, arc_idx)) = parent[node] {
            let cap = graph[prev][arc_idx].1;
            bottleneck = bottleneck.min(cap);
            node = prev;
        }

        // Augment flow along the path
        node = sink;
        while let Some((prev, arc_idx)) = parent[node] {
            let rev_idx = graph[prev][arc_idx].2;
            graph[prev][arc_idx].1 -= bottleneck;
            graph[node][rev_idx].1 += bottleneck;
            node = prev;
        }

        total_flow += bottleneck;
    }

    // Find min-cut: BFS from source in residual graph
    let mut reachable = vec![false; n];
    reachable[source] = true;
    let mut queue = VecDeque::new();
    queue.push_back(source);
    while let Some(u) = queue.pop_front() {
        for &(v, cap, _) in &graph[u] {
            if !reachable[v] && cap > 1e-10 {
                reachable[v] = true;
                queue.push_back(v);
            }
        }
    }

    // Selected blocks: block nodes reachable from source in residual graph
    let mut selected_blocks: Vec<usize> = Vec::new();
    let mut pit_value = 0.0_f64;
    for (node_idx, node_id) in node_ids.iter().enumerate() {
        if let MaxClosureNodeId::Block(linear_index) = node_id {
            if reachable[node_idx] {
                selected_blocks.push(*linear_index);
                if let Some(&w) = closure_graph.block_weights.get(linear_index) {
                    pit_value += w;
                }
            }
        }
    }

    selected_blocks.sort_unstable();

    let selected_block_count = selected_blocks.len();
    let total_block_count = closure_graph.block_count;

    Ok(UplSolverResult {
        selected_blocks,
        pit_value,
        max_flow_value: total_flow,
        upper_bound: closure_graph.sum_positive_weights,
        selected_block_count,
        total_block_count,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        max_closure::build_max_closure_graph,
        precedence::{PrecedenceEdge, PrecedenceGraph, PrecedenceNode},
    };

    /// Grafo de 3 bloques: 0→1→2 (2 debe extraerse antes que 1, 1 antes que 0)
    /// Pesos: 0=+10, 1=-3, 2=-2. Óptimo: extraer {0, 1, 2} con valor 10-3-2=5.
    #[test]
    fn solver_selects_all_blocks_when_positive_net_value() {
        let weights = BTreeMap::from([
            (0usize, 10.0_f64),
            (1usize, -3.0_f64),
            (2usize, -2.0_f64),
        ]);
        let graph = PrecedenceGraph::new(vec![
            PrecedenceEdge::new(PrecedenceNode::Block(2), PrecedenceNode::Block(1)),
            PrecedenceEdge::new(PrecedenceNode::Block(1), PrecedenceNode::Block(0)),
        ])
        .expect("graph should be valid");

        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("closure graph should build");
        let result = solve_upl_exact(&closure_graph).expect("solver should succeed");

        assert_eq!(result.selected_blocks, vec![0, 1, 2]);
        assert!((result.pit_value - 5.0).abs() < 1e-6);
    }

    /// Bloques sin precedencia. Solo se extrae el bloque positivo.
    #[test]
    fn solver_excludes_isolated_negative_blocks() {
        let weights = BTreeMap::from([
            (0usize, 8.0_f64),
            (1usize, -100.0_f64), // very negative, no precedence constraint
        ]);
        let graph = PrecedenceGraph::new(vec![]).expect("empty graph should be valid");

        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("closure graph should build");
        let result = solve_upl_exact(&closure_graph).expect("solver should succeed");

        assert_eq!(result.selected_blocks, vec![0]);
        assert!((result.pit_value - 8.0).abs() < 1e-6);
    }

    /// Bloque positivo sin precedencia + bloque positivo con precedencia desde costo bajo.
    #[test]
    fn solver_includes_ancestor_when_profitable_net() {
        // Block 1 (positive) requires block 2 (small cost)
        // Block 3 is isolated positive
        let weights = BTreeMap::from([
            (1usize, 20.0_f64),
            (2usize, -3.0_f64), // predecessor of 1, small cost
            (3usize, 5.0_f64),  // isolated positive
        ]);
        let graph = PrecedenceGraph::new(vec![PrecedenceEdge::new(
            PrecedenceNode::Block(2),
            PrecedenceNode::Block(1),
        )])
        .expect("graph should be valid");

        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("closure graph should build");
        let result = solve_upl_exact(&closure_graph).expect("solver should succeed");

        // All three should be selected: net value = 20 - 3 + 5 = 22
        assert_eq!(result.selected_blocks, vec![1, 2, 3]);
        assert!((result.pit_value - 22.0).abs() < 1e-6);
    }

    /// Todos los bloques son negativos → pit vacío.
    #[test]
    fn solver_returns_empty_pit_when_all_blocks_negative() {
        let weights = BTreeMap::from([(0usize, -5.0_f64), (1usize, -3.0_f64)]);
        let graph = PrecedenceGraph::new(vec![]).expect("empty graph should be valid");

        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("closure graph should build");
        let result = solve_upl_exact(&closure_graph).expect("solver should succeed");

        assert!(result.selected_blocks.is_empty());
        assert!((result.pit_value - 0.0).abs() < 1e-9);
    }

    /// Verifica que pit_value = upper_bound - max_flow_value (identidad del max-closure).
    #[test]
    fn pit_value_equals_upper_bound_minus_max_flow() {
        let weights = BTreeMap::from([
            (0usize, 10.0_f64),
            (1usize, -3.0_f64),
            (2usize, -2.0_f64),
        ]);
        let graph = PrecedenceGraph::new(vec![
            PrecedenceEdge::new(PrecedenceNode::Block(2), PrecedenceNode::Block(1)),
            PrecedenceEdge::new(PrecedenceNode::Block(1), PrecedenceNode::Block(0)),
        ])
        .expect("graph should be valid");

        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("closure graph should build");
        let result = solve_upl_exact(&closure_graph).expect("solver should succeed");

        let expected_pit_value = result.upper_bound - result.max_flow_value;
        assert!((result.pit_value - expected_pit_value).abs() < 1e-6);
    }
}
