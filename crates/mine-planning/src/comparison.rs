use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{PrecedenceEdge, PrecedenceGraph, PrecedenceNode, UpitPrototypeReport};

/// Reporte serializable de comparación entre dos grafos de precedencias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrecedenceGraphComparisonReport {
    /// Cantidad de nodos en el grafo de referencia.
    pub reference_node_count: usize,
    /// Cantidad de nodos en el grafo candidato.
    pub candidate_node_count: usize,
    /// Nodos presentes en ambos grafos.
    pub shared_nodes: usize,
    /// Nodos exclusivos de la referencia.
    pub reference_only_nodes: Vec<PrecedenceNode>,
    /// Nodos exclusivos del candidato.
    pub candidate_only_nodes: Vec<PrecedenceNode>,
    /// Cantidad de aristas en el grafo de referencia.
    pub reference_edge_count: usize,
    /// Cantidad de aristas en el grafo candidato.
    pub candidate_edge_count: usize,
    /// Aristas presentes en ambos grafos.
    pub shared_edges: usize,
    /// Aristas exclusivas de la referencia.
    pub reference_only_edges: Vec<PrecedenceEdge>,
    /// Aristas exclusivas del candidato.
    pub candidate_only_edges: Vec<PrecedenceEdge>,
    /// Jaccard sobre nodos.
    pub node_jaccard_index: f64,
    /// Jaccard sobre aristas.
    pub edge_jaccard_index: f64,
}

/// Reporte serializable de comparación entre dos membresías de bloques.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockMembershipComparisonReport {
    /// Cantidad de bloques en la referencia.
    pub reference_block_count: usize,
    /// Cantidad de bloques en el candidato.
    pub candidate_block_count: usize,
    /// Bloques presentes en ambos conjuntos.
    pub shared_blocks: usize,
    /// Bloques exclusivos de la referencia.
    pub reference_only_blocks: Vec<usize>,
    /// Bloques exclusivos del candidato.
    pub candidate_only_blocks: Vec<usize>,
    /// Jaccard de membresía.
    pub jaccard_index: f64,
}

/// Tolerancias explícitas para comparar una métrica numérica.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NumericMetricTolerance {
    /// Tolerancia absoluta permitida.
    pub absolute: Option<f64>,
    /// Tolerancia relativa permitida, expresada como fracción.
    pub relative: Option<f64>,
}

/// Resultado serializable para una métrica numérica compartida.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumericMetricComparison {
    /// Nombre de la métrica comparada.
    pub metric: String,
    /// Valor de referencia.
    pub reference_value: f64,
    /// Valor candidato.
    pub candidate_value: f64,
    /// Diferencia absoluta.
    pub absolute_difference: f64,
    /// Diferencia relativa respecto de la referencia cuando es definible.
    pub relative_difference: Option<f64>,
    /// Tolerancia absoluta aplicada.
    pub absolute_tolerance: Option<f64>,
    /// Tolerancia relativa aplicada.
    pub relative_tolerance: Option<f64>,
    /// Si la métrica cae dentro de las tolerancias configuradas.
    pub within_tolerance: bool,
}

/// Reporte serializable para comparar métricas numéricas nombradas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumericMetricComparisonReport {
    /// Métricas presentes en ambos lados.
    pub shared_metrics: Vec<NumericMetricComparison>,
    /// Métricas faltantes en el candidato.
    pub reference_only_metrics: Vec<String>,
    /// Métricas faltantes en la referencia.
    pub candidate_only_metrics: Vec<String>,
}

/// Compara dos grafos de precedencias usando igualdad exacta de nodos y aristas.
#[must_use]
pub fn compare_precedence_graphs(
    reference: &PrecedenceGraph,
    candidate: &PrecedenceGraph,
) -> PrecedenceGraphComparisonReport {
    let reference_nodes = reference.nodes().iter().cloned().collect::<BTreeSet<_>>();
    let candidate_nodes = candidate.nodes().iter().cloned().collect::<BTreeSet<_>>();
    let reference_edges = reference.edges().iter().cloned().collect::<BTreeSet<_>>();
    let candidate_edges = candidate.edges().iter().cloned().collect::<BTreeSet<_>>();
    let shared_nodes = reference_nodes.intersection(&candidate_nodes).count();
    let shared_edges = reference_edges.intersection(&candidate_edges).count();

    PrecedenceGraphComparisonReport {
        reference_node_count: reference_nodes.len(),
        candidate_node_count: candidate_nodes.len(),
        shared_nodes,
        reference_only_nodes: reference_nodes
            .difference(&candidate_nodes)
            .cloned()
            .collect(),
        candidate_only_nodes: candidate_nodes
            .difference(&reference_nodes)
            .cloned()
            .collect(),
        reference_edge_count: reference_edges.len(),
        candidate_edge_count: candidate_edges.len(),
        shared_edges,
        reference_only_edges: reference_edges
            .difference(&candidate_edges)
            .cloned()
            .collect(),
        candidate_only_edges: candidate_edges
            .difference(&reference_edges)
            .cloned()
            .collect(),
        node_jaccard_index: jaccard(shared_nodes, reference_nodes.len(), candidate_nodes.len()),
        edge_jaccard_index: jaccard(shared_edges, reference_edges.len(), candidate_edges.len()),
    }
}

/// Compara dos membresías de bloques usando igualdad exacta de índices lineales.
#[must_use]
pub fn compare_block_memberships(
    reference: &[usize],
    candidate: &[usize],
) -> BlockMembershipComparisonReport {
    let reference_blocks = reference.iter().copied().collect::<BTreeSet<_>>();
    let candidate_blocks = candidate.iter().copied().collect::<BTreeSet<_>>();
    let shared_blocks = reference_blocks.intersection(&candidate_blocks).count();

    BlockMembershipComparisonReport {
        reference_block_count: reference_blocks.len(),
        candidate_block_count: candidate_blocks.len(),
        shared_blocks,
        reference_only_blocks: reference_blocks
            .difference(&candidate_blocks)
            .copied()
            .collect(),
        candidate_only_blocks: candidate_blocks
            .difference(&reference_blocks)
            .copied()
            .collect(),
        jaccard_index: jaccard(
            shared_blocks,
            reference_blocks.len(),
            candidate_blocks.len(),
        ),
    }
}

/// Compara dos reportes `upit` usando la membresía seleccionada.
#[must_use]
pub fn compare_upit_reports(
    reference: &UpitPrototypeReport,
    candidate: &UpitPrototypeReport,
) -> BlockMembershipComparisonReport {
    compare_block_memberships(
        &reference.selected_linear_indices,
        &candidate.selected_linear_indices,
    )
}

/// Compara métricas numéricas nombradas con tolerancias explícitas por métrica.
#[must_use]
pub fn compare_named_numeric_metrics(
    reference: &BTreeMap<String, f64>,
    candidate: &BTreeMap<String, f64>,
    tolerances: &BTreeMap<String, NumericMetricTolerance>,
) -> NumericMetricComparisonReport {
    let reference_metric_names = reference.keys().cloned().collect::<BTreeSet<_>>();
    let candidate_metric_names = candidate.keys().cloned().collect::<BTreeSet<_>>();
    let shared_metrics = reference_metric_names
        .intersection(&candidate_metric_names)
        .map(|metric| {
            let reference_value = reference
                .get(metric)
                .expect("shared metric should exist in reference map");
            let candidate_value = candidate
                .get(metric)
                .expect("shared metric should exist in candidate map");
            let tolerance = tolerances
                .get(metric)
                .copied()
                .unwrap_or(NumericMetricTolerance {
                    absolute: None,
                    relative: None,
                });
            let absolute_difference = (candidate_value - reference_value).abs();
            let relative_difference = if *reference_value == 0.0 {
                (*candidate_value == 0.0).then_some(0.0)
            } else {
                Some(absolute_difference / reference_value.abs())
            };
            let within_absolute_tolerance = tolerance
                .absolute
                .is_none_or(|absolute| absolute_difference <= absolute);
            let within_relative_tolerance = tolerance.relative.is_none_or(|relative| {
                relative_difference.is_some_and(|difference| difference <= relative)
            });

            NumericMetricComparison {
                metric: metric.clone(),
                reference_value: *reference_value,
                candidate_value: *candidate_value,
                absolute_difference,
                relative_difference,
                absolute_tolerance: tolerance.absolute,
                relative_tolerance: tolerance.relative,
                within_tolerance: within_absolute_tolerance && within_relative_tolerance,
            }
        })
        .collect();

    NumericMetricComparisonReport {
        shared_metrics,
        reference_only_metrics: reference_metric_names
            .difference(&candidate_metric_names)
            .cloned()
            .collect(),
        candidate_only_metrics: candidate_metric_names
            .difference(&reference_metric_names)
            .cloned()
            .collect(),
    }
}

fn jaccard(shared: usize, left_count: usize, right_count: usize) -> f64 {
    let union = left_count + right_count - shared;

    if union == 0 {
        1.0
    } else {
        shared as f64 / union as f64
    }
}
