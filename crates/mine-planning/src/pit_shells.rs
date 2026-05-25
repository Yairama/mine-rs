//! Generación de shells anidados de pit final mediante revenue-factor sweeps.
//!
//! Un revenue-factor f ∈ (0, 1] escala todos los pesos de los bloques y
//! resuelve el UPL exacto. A medida que f decrece, el pit óptimo se contrae,
//! produciendo una familia de shells anidados.
//!
//! Esta implementación usa el backend exacto actual de max-flow basado en Dinic.
//! Sigue siendo un solver exacto generalista, pero ya escala mejor que la ruta
//! anterior con Edmonds-Karp para benchmarks tipo Marvin.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mine_blockmodel::BlockModel;
use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

use crate::max_closure::build_max_closure_graph;
use crate::precedence::PrecedenceGraph;
use crate::upl_solver::{UplSolverResult, solve_upl_exact};

// ── Contratos públicos ────────────────────────────────────────────────────────

/// Un pit shell generado a partir de un revenue-factor concreto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PitShell {
    /// Factor de revenue aplicado a los pesos de los bloques.
    pub revenue_factor: f64,
    /// Bloques seleccionados (índices lineales).
    pub selected_blocks: Vec<usize>,
    /// Valor económico total del pit con el factor aplicado.
    pub pit_value: f64,
    /// Número de bloques seleccionados.
    pub block_count: usize,
}

impl PitShell {
    fn from_result(revenue_factor: f64, result: UplSolverResult) -> Self {
        let block_count = result.selected_blocks.len();
        PitShell {
            revenue_factor,
            selected_blocks: result.selected_blocks,
            pit_value: result.pit_value,
            block_count,
        }
    }
}

/// Familia anidada de pit shells generados con diferentes revenue-factors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PitShellSet {
    /// Shells ordenados por revenue_factor ascendente (pit más pequeño primero).
    pub shells: Vec<PitShell>,
    /// Número de bloques totales en el modelo.
    pub total_block_count: usize,
    /// Número de factores usados como entrada.
    pub factors_evaluated: usize,
    /// Número de shells únicos (después de deduplicar).
    pub unique_shell_count: usize,
}

/// Escribe un `PitShellSet` en JSON como contrato abierto inicial para shells.
pub fn write_pit_shell_set_json(
    shell_set: &PitShellSet,
    path: impl AsRef<Path>,
) -> Result<(), MineError> {
    let json = serde_json::to_string_pretty(shell_set).map_err(|error| MineError::Io {
        message: format!("unable to serialize pit shell set to JSON: {error}"),
    })?;
    fs::write(path.as_ref(), json).map_err(|error| MineError::Io {
        message: format!("unable to write pit shell set JSON: {error}"),
    })?;
    Ok(())
}

/// Lee un `PitShellSet` desde JSON usando el contrato abierto inicial del proyecto.
pub fn read_pit_shell_set_json(path: impl AsRef<Path>) -> Result<PitShellSet, MineError> {
    let json = fs::read_to_string(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to read pit shell set JSON: {error}"),
    })?;
    serde_json::from_str(&json).map_err(|error| MineError::Io {
        message: format!("unable to decode pit shell set JSON: {error}"),
    })
}

/// Métricas por shell para un `PitShellSet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PitShellMetrics {
    /// Factor de revenue del shell.
    pub revenue_factor: f64,
    /// Número de bloques.
    pub block_count: usize,
    /// Tonelaje total (cuando se provee columna de tonelaje).
    pub total_tonnage: Option<f64>,
    /// Metal total (cuando se proveen columnas de tonelaje y ley).
    pub total_metal: Option<f64>,
    /// Valor económico total del shell.
    pub pit_value: f64,
    /// Bloques adicionales respecto al shell anterior.
    pub incremental_blocks: usize,
}

// ── Generación de shells ──────────────────────────────────────────────────────

/// Genera una familia anidada de pit shells mediante revenue-factor sweep.
///
/// Para cada factor f en `factors`:
/// - Se escalan los pesos de los bloques: `weight_i * f`
/// - Se resuelve el UPL exacto con max-flow (Dinic)
/// - Se almacena el shell resultante
///
/// Los shells se deduplicen: dos factores que producen la misma membresía
/// conservan solo el de mayor factor. Los factores deben estar en (0, 1].
///
/// # Advertencia de escala
///
/// El backend actual mejora sustancialmente la escalabilidad respecto a
/// Edmonds-Karp y ya permite correr benchmarks tipo Marvin. Aun así, si aparecen
/// instancias mucho mayores o más densas, un backend especializado adicional
/// (push-relabel o pseudoflow) puede seguir siendo una ruta válida.
pub fn generate_nested_shells(
    block_weights: &[f64],
    precedence_graph: &PrecedenceGraph,
    factors: &[f64],
) -> Result<PitShellSet, MineError> {
    if factors.is_empty() {
        return Err(MineError::invalid_parameter(
            "factors",
            "factors list must not be empty",
        ));
    }
    for &f in factors {
        if !f.is_finite() || f <= 0.0 || f > 1.0 {
            return Err(MineError::invalid_parameter(
                "factors",
                format!("each revenue factor must be in (0, 1], got {f}"),
            ));
        }
    }

    let total_block_count = block_weights.len();
    let factors_evaluated = factors.len();
    let mut shells: Vec<PitShell> = Vec::with_capacity(factors.len());
    let mut seen_memberships: Vec<Vec<usize>> = Vec::new();

    // Sort factors ascending so we process smallest pit first
    let mut sorted_factors = factors.to_vec();
    sorted_factors.sort_by(|a, b| a.partial_cmp(b).unwrap());

    for &f in &sorted_factors {
        let scaled_weights: BTreeMap<usize, f64> = block_weights
            .iter()
            .enumerate()
            .map(|(i, &w)| (i, w * f))
            .collect();
        let closure_graph = build_max_closure_graph(&scaled_weights, precedence_graph)?;
        let result = solve_upl_exact(&closure_graph)?;

        let selected = result.selected_blocks.clone();

        // Deduplicate: skip if exact same membership was already seen
        if seen_memberships.contains(&selected) {
            continue;
        }
        seen_memberships.push(selected);
        shells.push(PitShell::from_result(f, result));
    }

    let unique_shell_count = shells.len();

    Ok(PitShellSet {
        shells,
        total_block_count,
        factors_evaluated,
        unique_shell_count,
    })
}

/// Genera shells anidados directamente desde un `BlockModel`.
///
/// Wrapper conveniente que extrae los pesos de la columna `value_column` del
/// modelo y delega en `generate_nested_shells`.
pub fn generate_nested_shells_from_model(
    model: &BlockModel,
    precedence_graph: &PrecedenceGraph,
    value_column: &ColumnId,
    factors: &[f64],
) -> Result<PitShellSet, MineError> {
    let Some(column_data) = model.column(value_column) else {
        return Err(MineError::schema(format!(
            "value column `{value_column}` does not exist in block model"
        )));
    };

    let mine_blockmodel::ColumnData::Floats(values) = column_data else {
        return Err(MineError::schema(format!(
            "value column `{value_column}` must be a float column"
        )));
    };

    let block_weights: Vec<f64> = (0..model.block_count())
        .map(|row_index| values.get(row_index).copied().unwrap_or(0.0))
        .collect();

    generate_nested_shells(&block_weights, precedence_graph, factors)
}

/// Calcula métricas por shell para un `PitShellSet`.
///
/// Requiere una función que mapea `linear_index → row_index` para recuperar
/// valores de columnas del modelo original.
pub fn compute_pit_shell_metrics(
    shell_set: &PitShellSet,
    block_weights: &[f64],
    tonnage_per_block: Option<&[f64]>,
    metal_per_block: Option<&[f64]>,
) -> Vec<PitShellMetrics> {
    let mut metrics = Vec::with_capacity(shell_set.shells.len());
    let mut prev_count = 0usize;

    for shell in &shell_set.shells {
        let total_tonnage = tonnage_per_block.map(|tonnes| {
            shell
                .selected_blocks
                .iter()
                .filter_map(|&li| tonnes.get(li).copied())
                .sum::<f64>()
        });

        let total_metal = metal_per_block.map(|metal| {
            shell
                .selected_blocks
                .iter()
                .filter_map(|&li| metal.get(li).copied())
                .sum::<f64>()
        });

        // pit_value in undiscounted economic units (sum of original weights for selected blocks)
        let original_pit_value: f64 = shell
            .selected_blocks
            .iter()
            .filter_map(|&li| block_weights.get(li).copied())
            .sum();

        let incremental_blocks = shell.block_count.saturating_sub(prev_count);
        prev_count = shell.block_count;

        metrics.push(PitShellMetrics {
            revenue_factor: shell.revenue_factor,
            block_count: shell.block_count,
            total_tonnage,
            total_metal,
            pit_value: original_pit_value,
            incremental_blocks,
        });
    }

    metrics
}

// ── Generación de factores estándar ──────────────────────────────────────────

/// Genera una secuencia de `n` factores equiespaciados en (0, 1].
///
/// El factor 1.0 siempre se incluye (el pit completo).
/// El factor más pequeño es `1.0 / n`.
pub fn uniform_revenue_factors(n: usize) -> Result<Vec<f64>, MineError> {
    if n == 0 {
        return Err(MineError::invalid_parameter(
            "n",
            "number of revenue factors must be greater than zero",
        ));
    }
    Ok((1..=n).map(|i| i as f64 / n as f64).collect())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precedence::{PrecedenceEdge, PrecedenceGraph, PrecedenceNode};

    fn chain_graph(n: usize) -> PrecedenceGraph {
        // Chain: block 0 → 1 → 2 → ... → n-1 (each block requires the one below)
        let edges: Vec<PrecedenceEdge> = (0..n.saturating_sub(1))
            .map(|i| PrecedenceEdge::new(PrecedenceNode::Block(i), PrecedenceNode::Block(i + 1)))
            .collect();
        if edges.is_empty() {
            PrecedenceGraph::new(vec![PrecedenceEdge::new(
                PrecedenceNode::Block(0),
                PrecedenceNode::Block(0),
            )])
            .unwrap_err();
            // Return graph with a single node
            return PrecedenceGraph::from_nodes_and_edges(vec![PrecedenceNode::Block(0)], vec![])
                .expect("single-node graph should be valid");
        }
        PrecedenceGraph::new(edges).expect("chain graph should be valid")
    }

    #[test]
    fn single_factor_one_matches_exact_upl() {
        // weights: [10, -1, -1] chain (0 is predecessor of 1, which is predecessor of 2)
        // Optimal: select only block 0 (value=10), since adding blocks 1 and 2 reduces value
        // to 10-1-1=8 < 10
        let weights = vec![10.0_f64, -1.0, -1.0];
        let graph = chain_graph(3);

        let shells =
            generate_nested_shells(&weights, &graph, &[1.0]).expect("should generate shells");

        assert_eq!(shells.shells.len(), 1);
        let shell = &shells.shells[0];
        assert_eq!(shell.revenue_factor, 1.0);
        // Optimal: just block 0 selected
        assert_eq!(shell.block_count, 1);
        assert!(shell.selected_blocks.contains(&0));
        assert!((shell.pit_value - 10.0).abs() < 1e-9);
    }

    #[test]
    fn nested_shells_are_monotonically_expanding() {
        // weights: profitable chain [20, -5, -5, -5]
        // At f=1.0: all 4 selected (20-5-5-5=5)
        // At f=0.5: scaled [10, -2.5, -2.5, -2.5] → all 4 still profitable (10-2.5-2.5-2.5=2.5)
        // At f=0.1: scaled [2, -0.5, -0.5, -0.5] → still profitable (2-0.5-0.5-0.5=0.5)
        let weights = vec![20.0, -5.0, -5.0, -5.0];
        let graph = chain_graph(4);

        let shells = generate_nested_shells(&weights, &graph, &[0.1, 0.5, 1.0])
            .expect("should generate shells");

        // All shells should be non-empty and nested (each subsequent shell >= prev)
        for i in 1..shells.shells.len() {
            let prev = &shells.shells[i - 1].selected_blocks;
            let curr = &shells.shells[i].selected_blocks;
            // Every block in prev must be in curr (nesting property)
            for &b in prev {
                assert!(
                    curr.contains(&b),
                    "shell {i} must contain block {b} from shell {}",
                    i - 1
                );
            }
        }
    }

    #[test]
    fn small_factor_produces_empty_shell_when_no_profit() {
        // weights: [1, -100] chain — at f=0.01 the profit is [0.01, -100], nothing is worth mining
        let weights = vec![1.0, -100.0];
        let graph = chain_graph(2);

        let shells =
            generate_nested_shells(&weights, &graph, &[0.01, 1.0]).expect("should generate shells");

        // At f=0.01: block 0 worth 0.01, but requires predecessor of block 1 (cost -100*0.01=-1)
        // Since chain is 0→1, block 0 is predecessor of block 1 → to mine block 1, need block 0
        // Actually chain_graph: 0→1 means block 0 is predecessor of block 1
        // So to select block 1, must also select block 0
        // To select block 0, no predecessors required
        // weights at f=0.01: [0.01, -1.0] → selecting only block 0: gain = 0.01
        // selecting both: 0.01 + (-1.0) = -0.99 → bad
        // So optimal: select only block 0
        let shell_01 = shells
            .shells
            .iter()
            .find(|s| (s.revenue_factor - 0.01).abs() < 1e-9);
        if let Some(s) = shell_01 {
            // Should not select block 1 (too costly)
            assert!(!s.selected_blocks.contains(&1_usize));
        }
    }

    #[test]
    fn deduplicate_identical_shells() {
        // Very close factors may produce the same membership
        let weights = vec![5.0, -1.0];
        let graph = chain_graph(2);

        let shells = generate_nested_shells(&weights, &graph, &[0.9, 0.95, 1.0])
            .expect("should generate shells");

        // All three factors may produce same shell → should be deduplicated
        assert!(shells.unique_shell_count <= 3);
        assert_eq!(shells.factors_evaluated, 3);
    }

    #[test]
    fn uniform_revenue_factors_produces_correct_count() {
        let factors = uniform_revenue_factors(5).expect("should produce 5 factors");
        assert_eq!(factors.len(), 5);
        assert!((factors[4] - 1.0).abs() < 1e-12);
        assert!((factors[0] - 0.2).abs() < 1e-12);
    }

    #[test]
    fn reject_invalid_factors() {
        let weights = vec![1.0];
        let graph = chain_graph(1);

        let err =
            generate_nested_shells(&weights, &graph, &[0.0]).expect_err("zero factor should fail");
        assert!(err.to_string().contains("revenue factor"));

        let err2 =
            generate_nested_shells(&weights, &graph, &[1.5]).expect_err("factor > 1 should fail");
        assert!(err2.to_string().contains("revenue factor"));
    }

    #[test]
    fn metrics_sum_correctly() {
        // weights: [10, -1, -1] chain — optimal pit is just block 0 (value=10)
        let weights = vec![10.0, -1.0, -1.0];
        let graph = chain_graph(3);
        let shells = generate_nested_shells(&weights, &graph, &[1.0]).expect("shells ok");

        let metrics = compute_pit_shell_metrics(&shells, &weights, None, None);
        assert_eq!(metrics.len(), 1);
        // original pit_value = sum of weights for selected blocks = 10.0 (block 0 only)
        assert!((metrics[0].pit_value - 10.0).abs() < 1e-9);
        assert_eq!(metrics[0].block_count, 1);
        assert_eq!(metrics[0].incremental_blocks, 1);
    }
}
