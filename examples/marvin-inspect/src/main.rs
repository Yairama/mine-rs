//! Ejemplo ejecutable para cargar y perfilar `marvin.blocks`.

use std::env;
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
    let dataset_path = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("datasets")
            .join("benchmarks")
            .join("marvin")
            .join("marvin.blocks")
    });
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

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}
