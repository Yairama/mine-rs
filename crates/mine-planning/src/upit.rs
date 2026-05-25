use std::collections::BTreeSet;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

use crate::{PrecedenceGraph, PrecedenceNode};

/// Reporte serializable de un `upit` experimental generado con heurística abierta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpitPrototypeReport {
    /// Columna de valor usada para la heurística.
    pub value_column: ColumnId,
    /// Columna de tonelaje usada para resumir masa cuando se provee.
    pub tonnage_column: Option<ColumnId>,
    /// Bloques seleccionados por la heurística.
    pub selected_linear_indices: Vec<usize>,
    /// Cantidad total de bloques seleccionados.
    pub block_count: usize,
    /// Valor total agregado de la selección.
    pub total_value: f64,
    /// Tonelaje total agregado cuando se provee columna.
    pub total_tonnage: Option<f64>,
    /// Heurística aplicada.
    pub heuristic: String,
    /// Limitaciones conocidas del prototipo.
    pub limitations: Vec<String>,
}

/// Construye un `upit` experimental seleccionando bloques de valor positivo y cerrando la
/// selección por precedencias.
pub fn build_upit_prototype(
    model: &BlockModel,
    precedence_graph: &PrecedenceGraph,
    value_column: &ColumnId,
    tonnage_column: Option<&ColumnId>,
) -> Result<UpitPrototypeReport, MineError> {
    let value_values = float_column(model, value_column, "value")?;
    let tonnage_values = tonnage_column
        .map(|column_id| float_column(model, column_id, "tonnage"))
        .transpose()?;
    let predecessor_edges = precedence_graph
        .edges()
        .iter()
        .filter_map(|edge| match (edge.predecessor(), edge.successor()) {
            (PrecedenceNode::Block(predecessor), PrecedenceNode::Block(successor)) => {
                Some((*predecessor, *successor))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::<usize>::new();

    for (row_index, value) in value_values.iter().enumerate() {
        if *value > 0.0 {
            let linear_index = model.linear_index_at(row_index)?;
            select_with_predecessors(linear_index, &predecessor_edges, &mut selected);
        }
    }

    let selected_linear_indices = selected.into_iter().collect::<Vec<_>>();
    let mut total_value = 0.0;
    let mut total_tonnage = 0.0;

    for linear_index in &selected_linear_indices {
        let row_index = row_index_for_linear_index(model, *linear_index)?;
        total_value += value_values[row_index];

        if let Some(tonnage_values) = tonnage_values {
            total_tonnage += tonnage_values[row_index];
        }
    }

    Ok(UpitPrototypeReport {
        value_column: value_column.clone(),
        tonnage_column: tonnage_column.cloned(),
        block_count: selected_linear_indices.len(),
        selected_linear_indices,
        total_value,
        total_tonnage: tonnage_column.map(|_| total_tonnage),
        heuristic: "positive-block-closure".to_owned(),
        limitations: vec![
            "This prototype is a deterministic closure heuristic, not an exact maximum-closure or Lerchs-Grossmann solver.".to_owned(),
            "It selects every positive-value block and recursively adds required predecessors, so profitable subsets are not optimized globally.".to_owned(),
            "Economic interpretation depends entirely on the caller-provided value column semantics.".to_owned(),
        ],
    })
}

fn float_column<'a>(
    model: &'a BlockModel,
    column_id: &ColumnId,
    purpose: &str,
) -> Result<&'a [f64], MineError> {
    let Some(column_data) = model.column(column_id) else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` does not exist in block model storage"
        )));
    };

    let ColumnData::Floats(values) = column_data else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` must be a float column"
        )));
    };

    Ok(values)
}

fn select_with_predecessors(
    successor: usize,
    predecessor_edges: &[(usize, usize)],
    selected: &mut BTreeSet<usize>,
) {
    if !selected.insert(successor) {
        return;
    }

    for (predecessor, edge_successor) in predecessor_edges {
        if *edge_successor == successor {
            select_with_predecessors(*predecessor, predecessor_edges, selected);
        }
    }
}

fn row_index_for_linear_index(model: &BlockModel, linear_index: usize) -> Result<usize, MineError> {
    for row_index in 0..model.block_count() {
        if model.linear_index_at(row_index)? == linear_index {
            return Ok(row_index);
        }
    }

    Err(MineError::validation(format!(
        "linear index `{linear_index}` is not materialized in the block model"
    )))
}
