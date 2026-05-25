//! Ejemplo ejecutable para cargar y perfilar `marvin.blocks`.
//!
//! Uso:
//!   cargo run -p marvin-inspect [dataset_path] [output_path]
//!
//! Si no se especifican argumentos, el dataset se toma desde `datasets/benchmarks/marvin/marvin.blocks`
//! y el reporte se escribe en `datasets/benchmarks/marvin/outputs/inspect-report.json`.

use std::env;
use std::fs;
use std::path::PathBuf;

use mine_sdk::{BlockModelValidationExt, ValidationOptions, read_marvin_blocks};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct MarvinInspectOutput {
    dataset_path: String,
    block_count: usize,
    grid_cell_count: usize,
    sparse: bool,
    missing_linear_indices: usize,
    summary: mine_sdk::ModelSummary,
    validation: mine_sdk::ValidationReport,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let marvin_dir = repo_root
        .join("datasets")
        .join("benchmarks")
        .join("marvin");
    let dataset_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| marvin_dir.join("marvin.blocks"));
    let output_path = env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| marvin_dir.join("outputs").join("inspect-report.json"));
    let model = read_marvin_blocks(&dataset_path)?;
    let validation =
        model.validate_with_options(&ValidationOptions::new().with_sparse_allowed(true));
    let output = MarvinInspectOutput {
        dataset_path: dataset_path.display().to_string(),
        block_count: model.block_count(),
        grid_cell_count: model.grid_cell_count(),
        sparse: model.is_sparse(),
        missing_linear_indices: model.missing_linear_indices().len(),
        summary: model.summary()?,
        validation,
    };

    let json = serde_json::to_string_pretty(&output)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, &json)?;
    eprintln!("inspect report written to {}", output_path.display());
    println!("{json}");

    Ok(())
}
