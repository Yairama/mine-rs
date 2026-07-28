//! Transformación max-closure para pit final óptimo.
//!
//! Convierte un `EconomicBlockModel` + `PrecedenceGraph` en un problema
//! estructurado de max-closure / max-flow que puede resolverse exactamente.
//!
//! # Teoría
//!
//! El problema de pit final óptimo (Ultimate Pit Limit, UPL) puede formularse
//! como un **max-closure** en un grafo dirigido ponderado:
//!
//! - Cada bloque es un nodo con peso igual a su valor económico.
//! - Los arcos de precedencia se convierten en restricciones: si se extrae el
//!   sucesor, también hay que extraer el predecesor.
//! - El objetivo es seleccionar el subconjunto de bloques de máximo valor total
//!   que respete todas las precedencias.
//!
//! La transformación a max-flow sigue el esquema clásico de Hochbaum (1994):
//!
//! - Nodo fuente `s` conectado a todos los nodos con peso positivo `w > 0`
//!   con capacidad `w`.
//! - Nodo sumidero `t` conectado desde todos los nodos con peso negativo `w < 0`
//!   con capacidad `|w|`.
//! - Arcos de precedencia con capacidad infinita (representada por un valor
//!   suficientemente grande).
//!
//! El max-closure óptimo se obtiene de la s-t min-cut del grafo resultante.
//! Los nodos en el lado de `s` después del corte mínimo forman el pit óptimo.
//!
//! # Contenido de este módulo
//!
//! Este módulo produce el grafo de max-closure serializable como artefacto
//! auditable. La resolución del max-flow queda para el módulo `upl_solver`
//! (MR-156).

use std::collections::BTreeMap;

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::precedence::{PrecedenceGraph, PrecedenceNode};

// ── Nodos del grafo de max-closure ────────────────────────────────────────────

/// Identificador de nodo dentro del grafo de max-closure.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MaxClosureNodeId {
    /// Nodo fuente artificial (source).
    Source,
    /// Nodo sumidero artificial (sink).
    Sink,
    /// Bloque del modelo con su índice lineal.
    Block(usize),
}

/// Arco dirigido dentro del grafo de max-closure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxClosureArc {
    /// Nodo origen del arco.
    pub from: MaxClosureNodeId,
    /// Nodo destino del arco.
    pub to: MaxClosureNodeId,
    /// Capacidad del arco. Usar `f64::INFINITY` para arcos de precedencia.
    pub capacity: f64,
}

/// Tipo de arco en el grafo de max-closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaxClosureArcKind {
    /// Arco desde la fuente a un nodo con peso positivo.
    SourceArc,
    /// Arco desde un nodo con peso negativo al sumidero.
    SinkArc,
    /// Arco de precedencia entre dos bloques (capacidad infinita).
    PrecedenceArc,
}

/// Grafo de max-closure serializable para UPL.
///
/// Contiene el grafo completo listo para resolución por max-flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxClosureGraph {
    /// Número de nodos de bloque.
    pub block_count: usize,
    /// Pesos de bloques indexados por índice lineal (positivo = ganancia, negativo = costo).
    pub block_weights: BTreeMap<usize, f64>,
    /// Arcos del grafo (source/sink arcs + precedence arcs).
    pub arcs: Vec<MaxClosureArc>,
    /// Suma de pesos positivos (cota superior del valor extractable).
    pub sum_positive_weights: f64,
    /// Suma de pesos negativos (cota inferior del costo inevitable).
    pub sum_negative_weights: f64,
}

impl MaxClosureGraph {
    /// Retorna los arcos de fuente.
    pub fn source_arcs(&self) -> impl Iterator<Item = &MaxClosureArc> {
        self.arcs
            .iter()
            .filter(|a| a.from == MaxClosureNodeId::Source)
    }

    /// Retorna los arcos de sumidero.
    pub fn sink_arcs(&self) -> impl Iterator<Item = &MaxClosureArc> {
        self.arcs.iter().filter(|a| a.to == MaxClosureNodeId::Sink)
    }

    /// Retorna los arcos de precedencia.
    pub fn precedence_arcs(&self) -> impl Iterator<Item = &MaxClosureArc> {
        self.arcs
            .iter()
            .filter(|a| a.from != MaxClosureNodeId::Source && a.to != MaxClosureNodeId::Sink)
    }
}

// ── Transformación ────────────────────────────────────────────────────────────

/// Construye un `MaxClosureGraph` desde los pesos de bloques y el grafo de precedencias.
///
/// # Parámetros
///
/// - `block_weights`: mapa de índice lineal → valor económico del bloque.
///   Puede incluir valores positivos (ganancia) y negativos (costo de acceso).
/// - `precedence_graph`: DAG de precedencias entre bloques. Solo se procesan
///   las aristas cuyos dos extremos sean `PrecedenceNode::Block`.
///
/// # Contratos
///
/// - Los pesos de bloques deben ser finitos.
/// - El grafo de precedencias puede incluir nodos que no estén en `block_weights`;
///   se ignoran silenciosamente (pueden ser nodos de banco o fase).
///
/// # Errores
///
/// Retorna error si `block_weights` está vacío o contiene valores no finitos.
pub fn build_max_closure_graph(
    block_weights: &BTreeMap<usize, f64>,
    precedence_graph: &PrecedenceGraph,
) -> Result<MaxClosureGraph, MineError> {
    if block_weights.is_empty() {
        return Err(MineError::invalid_parameter(
            "block_weights",
            "max-closure graph requires at least one block weight",
        ));
    }

    for (idx, weight) in block_weights {
        if !weight.is_finite() {
            return Err(MineError::invalid_parameter(
                "block_weights",
                format!("block weight for index {idx} is not finite"),
            ));
        }
    }

    let block_count = block_weights.len();
    let mut arcs: Vec<MaxClosureArc> = Vec::new();

    let mut sum_positive_weights = 0.0_f64;
    let mut sum_negative_weights = 0.0_f64;

    // Source and sink arcs from block weights
    for (&linear_index, &weight) in block_weights {
        if weight > 0.0 {
            sum_positive_weights += weight;
            arcs.push(MaxClosureArc {
                from: MaxClosureNodeId::Source,
                to: MaxClosureNodeId::Block(linear_index),
                capacity: weight,
            });
        } else if weight < 0.0 {
            sum_negative_weights += weight;
            arcs.push(MaxClosureArc {
                from: MaxClosureNodeId::Block(linear_index),
                to: MaxClosureNodeId::Sink,
                capacity: weight.abs(),
            });
        }
        // zero-weight blocks get neither arc
    }

    // Precedence arcs: successor → predecessor with infinite capacity.
    // Direction follows Hochbaum (1994): arc u→v means "if u is selected, v must be selected".
    // Mining constraint: "if succ is in pit, pred must also be in pit" → arc succ → pred.
    for edge in precedence_graph.edges() {
        let (PrecedenceNode::Block(pred_idx), PrecedenceNode::Block(succ_idx)) =
            (edge.predecessor(), edge.successor())
        else {
            continue; // skip bench/phase nodes
        };

        // Only include if both endpoints are in block_weights
        if !block_weights.contains_key(pred_idx) || !block_weights.contains_key(succ_idx) {
            continue;
        }

        arcs.push(MaxClosureArc {
            from: MaxClosureNodeId::Block(*succ_idx),
            to: MaxClosureNodeId::Block(*pred_idx),
            capacity: f64::INFINITY,
        });
    }

    Ok(MaxClosureGraph {
        block_count,
        block_weights: block_weights.clone(),
        arcs,
        sum_positive_weights,
        sum_negative_weights,
    })
}

/// Verifica si un conjunto de bloques forma una clausura válida respecto del grafo.
///
/// Un conjunto `C` es una clausura si para todo bloque en `C`, todos sus
/// predecesores directos (en el grafo de precedencias) también están en `C`.
///
/// # Resultado
///
/// Retorna `Ok(())` si la clausura es válida; `Err` si alguna precedencia es violada.
pub fn verify_closure(
    selected_blocks: &[usize],
    precedence_graph: &PrecedenceGraph,
) -> Result<(), MineError> {
    let selected_set: std::collections::BTreeSet<usize> = selected_blocks.iter().copied().collect();

    for edge in precedence_graph.edges() {
        let (PrecedenceNode::Block(pred_idx), PrecedenceNode::Block(succ_idx)) =
            (edge.predecessor(), edge.successor())
        else {
            continue;
        };

        if selected_set.contains(succ_idx) && !selected_set.contains(pred_idx) {
            return Err(MineError::Planning {
                message: format!(
                    "closure violation: block {succ_idx} is selected but its \
                     predecessor {pred_idx} is not"
                ),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::precedence::{PrecedenceEdge, PrecedenceGraph, PrecedenceNode};

    fn two_block_graph() -> PrecedenceGraph {
        // Block 1 must come before block 0
        PrecedenceGraph::new(vec![PrecedenceEdge::new(
            PrecedenceNode::Block(1),
            PrecedenceNode::Block(0),
        )])
        .expect("graph should be valid")
    }

    #[test]
    fn source_arcs_connect_to_positive_blocks() {
        let weights = BTreeMap::from([(0usize, 10.0_f64), (1usize, -5.0_f64)]);
        let graph = two_block_graph();
        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("should build closure graph");

        assert_eq!(closure_graph.block_count, 2);
        assert_eq!(closure_graph.source_arcs().count(), 1);
        let sa = closure_graph
            .source_arcs()
            .next()
            .expect("source arc should exist");
        assert_eq!(sa.to, MaxClosureNodeId::Block(0));
        assert!((sa.capacity - 10.0).abs() < 1e-9);
    }

    #[test]
    fn sink_arcs_connect_from_negative_blocks() {
        let weights = BTreeMap::from([(0usize, 10.0_f64), (1usize, -5.0_f64)]);
        let graph = two_block_graph();
        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("should build closure graph");

        assert_eq!(closure_graph.sink_arcs().count(), 1);
        let ta = closure_graph
            .sink_arcs()
            .next()
            .expect("sink arc should exist");
        assert_eq!(ta.from, MaxClosureNodeId::Block(1));
        assert!((ta.capacity - 5.0).abs() < 1e-9);
    }

    #[test]
    fn precedence_arcs_have_infinite_capacity() {
        let weights = BTreeMap::from([(0usize, 10.0_f64), (1usize, -5.0_f64)]);
        let graph = two_block_graph();
        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("should build closure graph");

        let prec_arcs: Vec<_> = closure_graph.precedence_arcs().collect();
        assert_eq!(prec_arcs.len(), 1);
        assert!(prec_arcs[0].capacity.is_infinite());
        // Hochbaum direction: succ → pred (block 0 → block 1, because 1 is pred of 0)
        assert_eq!(prec_arcs[0].from, MaxClosureNodeId::Block(0));
        assert_eq!(prec_arcs[0].to, MaxClosureNodeId::Block(1));
    }

    #[test]
    fn sum_positive_and_negative_weights_are_correct() {
        let weights = BTreeMap::from([
            (0usize, 10.0_f64),
            (1usize, -5.0_f64),
            (2usize, 8.0_f64),
            (3usize, 0.0_f64),
        ]);
        let graph = PrecedenceGraph::new(vec![]).expect("empty graph should be valid");
        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("should build closure graph");

        assert!((closure_graph.sum_positive_weights - 18.0).abs() < 1e-9);
        assert!((closure_graph.sum_negative_weights - (-5.0)).abs() < 1e-9);
    }

    #[test]
    fn verify_valid_closure_returns_ok() {
        let graph = two_block_graph();
        // Select both blocks: valid closure (1 is pred of 0, both selected)
        assert!(verify_closure(&[0, 1], &graph).is_ok());
    }

    #[test]
    fn verify_invalid_closure_returns_error() {
        let graph = two_block_graph();
        // Select only block 0 but block 1 is its predecessor: violation
        let result = verify_closure(&[0], &graph);
        assert!(result.is_err());
    }

    #[test]
    fn build_max_closure_graph_rejects_empty_weights() {
        let weights = BTreeMap::new();
        let graph = PrecedenceGraph::new(vec![]).expect("empty graph should be valid");
        assert!(build_max_closure_graph(&weights, &graph).is_err());
    }

    #[test]
    fn bench_and_phase_nodes_are_ignored_in_precedence_arcs() {
        let weights = BTreeMap::from([(0usize, 5.0_f64)]);
        let graph = PrecedenceGraph::new(vec![PrecedenceEdge::new(
            PrecedenceNode::Bench(100),
            PrecedenceNode::Block(0),
        )])
        .expect("graph should be valid");
        let closure_graph =
            build_max_closure_graph(&weights, &graph).expect("should build closure graph");

        // Bench→Block arc should be ignored (bench is not a block node)
        assert_eq!(closure_graph.precedence_arcs().count(), 0);
    }
}
