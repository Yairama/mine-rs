use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

use mine_blockmodel::BlockModel;
use mine_core::MineError;
use mine_indexing::{GridIndex, ijk_to_linear, linear_to_ijk};
use serde::{Deserialize, Serialize};

/// Nodo soportado dentro de un grafo de precedencias de minado.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PrecedenceNode {
    /// Nodo asociado a un bloque identificado por índice lineal.
    Block(usize),
    /// Nodo asociado a un banco identificado por su número.
    Bench(i64),
    /// Nodo asociado a una fase identificada por su nombre.
    Phase(String),
}

/// Arista dirigida dentro de un grafo de precedencias.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PrecedenceEdge {
    predecessor: PrecedenceNode,
    successor: PrecedenceNode,
}

impl PrecedenceEdge {
    /// Construye una arista dirigida entre dos nodos.
    #[must_use]
    pub fn new(predecessor: PrecedenceNode, successor: PrecedenceNode) -> Self {
        Self {
            predecessor,
            successor,
        }
    }

    /// Nodo que debe ocurrir antes.
    #[must_use]
    pub fn predecessor(&self) -> &PrecedenceNode {
        &self.predecessor
    }

    /// Nodo que depende del predecessor.
    #[must_use]
    pub fn successor(&self) -> &PrecedenceNode {
        &self.successor
    }
}

/// DAG serializable de precedencias entre bloques, bancos o fases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecedenceGraph {
    nodes: Vec<PrecedenceNode>,
    edges: Vec<PrecedenceEdge>,
}

impl PrecedenceGraph {
    /// Construye un grafo validando self-loops y ciclos.
    pub fn new(edges: Vec<PrecedenceEdge>) -> Result<Self, MineError> {
        let nodes = edges
            .iter()
            .flat_map(|edge| [edge.predecessor.clone(), edge.successor.clone()])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        Self::from_nodes_and_edges(nodes, edges)
    }

    /// Construye un grafo validando nodos, self-loops y ciclos.
    pub fn from_nodes_and_edges(
        nodes: Vec<PrecedenceNode>,
        edges: Vec<PrecedenceEdge>,
    ) -> Result<Self, MineError> {
        let nodes = nodes
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let node_set = nodes.iter().cloned().collect::<BTreeSet<_>>();
        let edges = edges
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        for edge in &edges {
            if edge.predecessor == edge.successor {
                return Err(MineError::Planning {
                    message:
                        "precedence edge cannot reference the same node as predecessor and successor"
                            .to_owned(),
                });
            }

            if !node_set.contains(edge.predecessor()) || !node_set.contains(edge.successor()) {
                return Err(MineError::Planning {
                    message: "precedence edge references a node outside the declared graph"
                        .to_owned(),
                });
            }
        }

        ensure_acyclic(&nodes, &edges)?;

        Ok(Self { nodes, edges })
    }

    /// Nodos incluidos en el grafo.
    #[must_use]
    pub fn nodes(&self) -> &[PrecedenceNode] {
        &self.nodes
    }

    /// Aristas dirigidas del grafo.
    #[must_use]
    pub fn edges(&self) -> &[PrecedenceEdge] {
        &self.edges
    }

    /// Sucesores directos de un nodo.
    #[must_use]
    pub fn successors(&self, node: &PrecedenceNode) -> Vec<PrecedenceNode> {
        self.edges
            .iter()
            .filter(|edge| edge.predecessor() == node)
            .map(|edge| edge.successor().clone())
            .collect()
    }

    /// Predecesores directos de un nodo.
    #[must_use]
    pub fn predecessors(&self, node: &PrecedenceNode) -> Vec<PrecedenceNode> {
        self.edges
            .iter()
            .filter(|edge| edge.successor() == node)
            .map(|edge| edge.predecessor().clone())
            .collect()
    }
}

/// Escribe un `PrecedenceGraph` en JSON como formato abierto inicial para examples y roundtrips.
pub fn write_precedence_graph_json(
    graph: &PrecedenceGraph,
    path: impl AsRef<Path>,
) -> Result<(), MineError> {
    let json = serde_json::to_string_pretty(graph).map_err(|error| MineError::Io {
        message: format!("unable to serialize precedence graph to JSON: {error}"),
    })?;
    fs::write(path.as_ref(), json).map_err(|error| MineError::Io {
        message: format!("unable to write precedence graph JSON: {error}"),
    })?;
    Ok(())
}

/// Lee un `PrecedenceGraph` desde JSON usando el contrato abierto inicial del proyecto.
pub fn read_precedence_graph_json(path: impl AsRef<Path>) -> Result<PrecedenceGraph, MineError> {
    let json = fs::read_to_string(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to read precedence graph JSON: {error}"),
    })?;
    serde_json::from_str(&json).map_err(|error| MineError::Io {
        message: format!("unable to decode precedence graph JSON: {error}"),
    })
}

/// Offset explícito de un bloque predecesor respecto del bloque actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PrecedenceOffset {
    di: isize,
    dj: isize,
    dk: isize,
}

impl PrecedenceOffset {
    /// Construye un offset validando que el predecesor apunte a un bloque por encima del actual.
    pub fn new(di: isize, dj: isize, dk: isize) -> Result<Self, MineError> {
        if dk <= 0 {
            return Err(MineError::invalid_parameter(
                "dk",
                "precedence offset dk must be greater than zero",
            ));
        }

        Ok(Self { di, dj, dk })
    }

    /// Desplazamiento sobre el eje `i`.
    #[must_use]
    pub const fn di(&self) -> isize {
        self.di
    }

    /// Desplazamiento sobre el eje `j`.
    #[must_use]
    pub const fn dj(&self) -> isize {
        self.dj
    }

    /// Desplazamiento sobre el eje `k`.
    #[must_use]
    pub const fn dk(&self) -> isize {
        self.dk
    }
}

/// Plantilla explícita para derivar precedencias bloque a bloque.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockPrecedenceTemplate {
    predecessor_offsets: Vec<PrecedenceOffset>,
}

impl BlockPrecedenceTemplate {
    /// Construye una plantilla validando offsets y evitando duplicados.
    pub fn new(predecessor_offsets: Vec<PrecedenceOffset>) -> Result<Self, MineError> {
        if predecessor_offsets.is_empty() {
            return Err(MineError::invalid_parameter(
                "predecessor_offsets",
                "block precedence template must contain at least one offset",
            ));
        }

        Ok(Self {
            predecessor_offsets: predecessor_offsets
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
    }

    /// Offsets de bloques que deben ocurrir antes del bloque actual.
    #[must_use]
    pub fn predecessor_offsets(&self) -> &[PrecedenceOffset] {
        &self.predecessor_offsets
    }
}

/// Genera un grafo de precedencias bloque a bloque usando una plantilla explícita de offsets.
pub fn build_block_precedence_graph(
    model: &BlockModel,
    template: &BlockPrecedenceTemplate,
) -> Result<PrecedenceGraph, MineError> {
    let materialized_linear_indices = (0..model.block_count())
        .map(|row_index| model.linear_index_at(row_index))
        .collect::<Result<Vec<_>, _>>()?;
    let materialized_set = materialized_linear_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let nodes = materialized_linear_indices
        .iter()
        .copied()
        .map(PrecedenceNode::Block)
        .collect::<Vec<_>>();
    let mut edges = Vec::new();

    for linear_index in materialized_linear_indices {
        let grid_index = linear_to_ijk(model.grid(), linear_index)?;

        for offset in template.predecessor_offsets() {
            let Some(predecessor_i) = apply_offset(grid_index.i(), offset.di()) else {
                continue;
            };
            let Some(predecessor_j) = apply_offset(grid_index.j(), offset.dj()) else {
                continue;
            };
            let Some(predecessor_k) = apply_offset(grid_index.k(), offset.dk()) else {
                continue;
            };

            if predecessor_i >= model.grid().shape().nx()
                || predecessor_j >= model.grid().shape().ny()
                || predecessor_k >= model.grid().shape().nz()
            {
                continue;
            }

            let predecessor_linear = ijk_to_linear(
                model.grid(),
                GridIndex::new(predecessor_i, predecessor_j, predecessor_k),
            )?;

            if materialized_set.contains(&predecessor_linear) {
                edges.push(PrecedenceEdge::new(
                    PrecedenceNode::Block(predecessor_linear),
                    PrecedenceNode::Block(linear_index),
                ));
            }
        }
    }

    PrecedenceGraph::from_nodes_and_edges(nodes, edges)
}

fn ensure_acyclic(nodes: &[PrecedenceNode], edges: &[PrecedenceEdge]) -> Result<(), MineError> {
    let mut indegree = nodes
        .iter()
        .cloned()
        .map(|node| (node, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut adjacency = BTreeMap::<PrecedenceNode, Vec<PrecedenceNode>>::new();

    for edge in edges {
        adjacency
            .entry(edge.predecessor.clone())
            .or_default()
            .push(edge.successor.clone());
        *indegree
            .get_mut(edge.successor())
            .expect("successor should exist in indegree map") += 1;
    }

    let mut queue = indegree
        .iter()
        .filter_map(|(node, indegree)| (*indegree == 0).then_some(node.clone()))
        .collect::<VecDeque<_>>();
    let mut visited = 0_usize;

    while let Some(node) = queue.pop_front() {
        visited += 1;

        if let Some(successors) = adjacency.get(&node) {
            for successor in successors {
                let successor_indegree = indegree
                    .get_mut(successor)
                    .expect("successor should exist in indegree map");
                *successor_indegree -= 1;

                if *successor_indegree == 0 {
                    queue.push_back(successor.clone());
                }
            }
        }
    }

    if visited == nodes.len() {
        Ok(())
    } else {
        Err(MineError::Planning {
            message: "precedence graph contains a cycle and is not a valid DAG".to_owned(),
        })
    }
}

fn apply_offset(value: usize, offset: isize) -> Option<usize> {
    if offset >= 0 {
        value.checked_add(offset as usize)
    } else {
        value.checked_sub(offset.unsigned_abs())
    }
}
