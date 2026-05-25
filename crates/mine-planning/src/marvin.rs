use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{ColumnId, MineError};

use crate::{PrecedenceEdge, PrecedenceGraph, PrecedenceNode};

/// Lee `marvin.prec` y lo normaliza a `PrecedenceGraph` usando los índices lineales del modelo.
pub fn read_marvin_precedence_graph(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<PrecedenceGraph, MineError> {
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;
    let file = File::open(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to open Marvin precedence file: {error}"),
    })?;
    let reader = BufReader::new(file);
    let mut nodes = BTreeSet::new();
    let mut edges = BTreeSet::new();

    for (line_offset, line_result) in reader.lines().enumerate() {
        let line_number = line_offset + 1;
        let line = line_result.map_err(|error| MineError::Io {
            message: format!("unable to read Marvin precedence line {line_number}: {error}"),
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 2 {
            return Err(MineError::validation(format!(
                "Marvin precedence line {line_number} must contain at least block id and predecessor count"
            )));
        }

        let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
        let predecessor_count = parse_usize_field(fields[1], line_number, "predecessor_count")?;
        if fields.len() != predecessor_count + 2 {
            return Err(MineError::validation(format!(
                "Marvin precedence line {line_number} declares {predecessor_count} predecessors but contains {} ids",
                fields.len().saturating_sub(2)
            )));
        }

        let successor_linear_index = map_block_id(source_block_id, &block_id_to_linear_index)?;
        let successor_node = PrecedenceNode::Block(successor_linear_index);
        nodes.insert(successor_node.clone());

        for predecessor_id_text in &fields[2..] {
            let predecessor_block_id =
                parse_i64_field(predecessor_id_text, line_number, "predecessor_block_id")?;
            let predecessor_linear_index =
                map_block_id(predecessor_block_id, &block_id_to_linear_index)?;
            let predecessor_node = PrecedenceNode::Block(predecessor_linear_index);
            nodes.insert(predecessor_node.clone());
            edges.insert(PrecedenceEdge::new(
                predecessor_node,
                successor_node.clone(),
            ));
        }
    }

    PrecedenceGraph::from_nodes_and_edges(nodes.into_iter().collect(), edges.into_iter().collect())
}

/// Lee `marvin_upit.sol` y lo normaliza como membresía de índices lineales.
pub fn read_marvin_upit_solution(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<Vec<usize>, MineError> {
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;
    let file = File::open(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to open Marvin upit solution file: {error}"),
    })?;
    let reader = BufReader::new(file);
    let mut selected = BTreeSet::new();

    for (line_offset, line_result) in reader.lines().enumerate() {
        let line_number = line_offset + 1;
        let line = line_result.map_err(|error| MineError::Io {
            message: format!("unable to read Marvin upit solution line {line_number}: {error}"),
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let source_block_id = parse_i64_field(trimmed, line_number, "block_id")?;
        selected.insert(map_block_id(source_block_id, &block_id_to_linear_index)?);
    }

    Ok(selected.into_iter().collect())
}

/// Lee `marvin.upit` y normaliza los valores objetivo por bloque.
///
/// Retorna un vector de `(linear_index, block_objective_value)` para todos los bloques
/// del modelo, en el mismo orden que el archivo.
pub fn read_marvin_upit_block_values(
    path: impl AsRef<Path>,
    model: &BlockModel,
) -> Result<Vec<(usize, f64)>, MineError> {
    let block_id_to_linear_index = source_block_id_to_linear_index(model)?;
    let file = File::open(path.as_ref()).map_err(|error| MineError::Io {
        message: format!("unable to open Marvin upit block values file: {error}"),
    })?;
    let reader = BufReader::new(file);
    let mut result = Vec::new();
    let mut in_data = false;

    for (line_offset, line_result) in reader.lines().enumerate() {
        let line_number = line_offset + 1;
        let line = line_result.map_err(|error| MineError::Io {
            message: format!("unable to read Marvin upit block values line {line_number}: {error}"),
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Header keywords
        if trimmed.starts_with("NAME:")
            || trimmed.starts_with("TYPE:")
            || trimmed.starts_with("NBLOCKS:")
            || trimmed.starts_with("OBJECTIVE_FUNCTION:")
        {
            in_data = true;
            continue;
        }
        if trimmed == "EOF" {
            break;
        }
        if !in_data {
            continue;
        }

        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 2 {
            return Err(MineError::validation(format!(
                "Marvin upit block values line {line_number} must contain block_id and value (got {} fields)",
                fields.len()
            )));
        }

        let source_block_id = parse_i64_field(fields[0], line_number, "block_id")?;
        let block_value = fields[1].parse::<f64>().map_err(|error| {
            MineError::validation(format!(
                "Marvin upit block values line {line_number} contains invalid float in block_value: {error}"
            ))
        })?;
        let linear_index = map_block_id(source_block_id, &block_id_to_linear_index)?;
        result.push((linear_index, block_value));
    }

    Ok(result)
}

fn source_block_id_to_linear_index(model: &BlockModel) -> Result<BTreeMap<i64, usize>, MineError> {
    let source_block_id = ColumnId::new("source_block_id")?;
    let Some(column) = model.column(&source_block_id) else {
        return Err(MineError::schema(
            "Marvin normalization requires `source_block_id` column in the block model",
        ));
    };
    let ColumnData::Integers(source_ids) = column else {
        return Err(MineError::schema(
            "Marvin normalization requires `source_block_id` to be an integer column",
        ));
    };

    let mut mapping = BTreeMap::new();
    for (row_index, source_id) in source_ids.iter().enumerate() {
        let linear_index = model.linear_index_at(row_index)?;
        if mapping.insert(*source_id, linear_index).is_some() {
            return Err(MineError::validation(format!(
                "duplicate Marvin source_block_id `{source_id}` found in model"
            )));
        }
    }

    Ok(mapping)
}

fn map_block_id(
    source_block_id: i64,
    block_id_to_linear_index: &BTreeMap<i64, usize>,
) -> Result<usize, MineError> {
    block_id_to_linear_index
        .get(&source_block_id)
        .copied()
        .ok_or_else(|| {
            MineError::validation(format!(
                "Marvin benchmark artifact references unknown source_block_id `{source_block_id}`"
            ))
        })
}

fn parse_i64_field(value: &str, line_number: usize, field_name: &str) -> Result<i64, MineError> {
    value.parse::<i64>().map_err(|error| {
        MineError::validation(format!(
            "Marvin benchmark line {line_number} contains invalid integer in {field_name}: {error}"
        ))
    })
}

fn parse_usize_field(
    value: &str,
    line_number: usize,
    field_name: &str,
) -> Result<usize, MineError> {
    value.parse::<usize>().map_err(|error| {
        MineError::validation(format!(
            "Marvin benchmark line {line_number} contains invalid usize in {field_name}: {error}"
        ))
    })
}
