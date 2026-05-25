//! Ejemplo ejecutable para generar y serializar `prec` abierto desde `marvin.blocks`.

use std::env;
use std::fs;
use std::path::PathBuf;

use mine_sdk::{
    BlockPrecedenceTemplate, PrecedenceOffset, build_block_precedence_graph, read_marvin_blocks,
    read_precedence_graph_json, write_precedence_graph_json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct MarvinPrecOutput {
    dataset_path: String,
    output_path: String,
    node_count: usize,
    edge_count: usize,
    predecessor_offsets: Vec<(isize, isize, isize)>,
    roundtrip_matches: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_path = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("datasets")
            .join("benchmarks")
            .join("marvin")
            .join("marvin.blocks")
    });
    let output_path = env::args_os().nth(2).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("marvin")
            .join("marvin.prec.json")
    });

    let model = read_marvin_blocks(&dataset_path)?;
    let offsets = vec![
        PrecedenceOffset::new(0, 0, 1)?,
        PrecedenceOffset::new(-1, 0, 1)?,
        PrecedenceOffset::new(1, 0, 1)?,
        PrecedenceOffset::new(0, -1, 1)?,
        PrecedenceOffset::new(0, 1, 1)?,
    ];
    let template = BlockPrecedenceTemplate::new(offsets)?;
    let graph = build_block_precedence_graph(&model, &template)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    write_precedence_graph_json(&graph, &output_path)?;
    let roundtrip = read_precedence_graph_json(&output_path)?;
    let output = MarvinPrecOutput {
        dataset_path: dataset_path.display().to_string(),
        output_path: output_path.display().to_string(),
        node_count: graph.nodes().len(),
        edge_count: graph.edges().len(),
        predecessor_offsets: template
            .predecessor_offsets()
            .iter()
            .map(|offset| (offset.di(), offset.dj(), offset.dk()))
            .collect(),
        roundtrip_matches: roundtrip == graph,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}
