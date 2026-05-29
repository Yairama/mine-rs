#[path = "../src/benchmark_blocks_support.rs"]
mod benchmark_blocks_support;

use std::path::PathBuf;

use benchmark_blocks_support::read_benchmark_blocks;
use mine_sdk::{ColumnId, MetadataValue};

fn benchmark_blocks_path(instance: &str, file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("datasets")
        .join("benchmarks")
        .join(instance)
        .join(file_name)
}

#[test]
fn load_staged_marvin_blocks_as_sparse_model() {
    let model = read_benchmark_blocks(benchmark_blocks_path("marvin", "marvin.blocks"), "marvin")
        .expect("marvin.blocks should load");

    assert!(model.is_sparse());
    assert_eq!(model.block_count(), 53_271);
    assert_eq!(model.grid_cell_count(), 62_220);
    assert_eq!(model.grid().shape().nx(), 61);
    assert_eq!(model.grid().shape().ny(), 60);
    assert_eq!(model.grid().shape().nz(), 17);
    assert_eq!(model.missing_linear_indices().len(), 8_949);
    assert!(
        model
            .column(&ColumnId::new("source_block_id").expect("column id should be valid"))
            .is_some()
    );
    assert!(
        model
            .column(&ColumnId::new("field_7").expect("column id should be valid"))
            .is_some()
    );
    assert_eq!(
        model
            .metadata()
            .get("benchmark_family")
            .expect("metadata should contain benchmark family"),
        &MetadataValue::Text("marvin".to_owned())
    );
}

#[test]
fn load_staged_mclaughlin_limit_blocks_as_sparse_model() {
    let model = read_benchmark_blocks(
        benchmark_blocks_path("mclaughlin-limit", "mclaughlin_limit.blocks"),
        "mclaughlin-limit",
    )
    .expect("mclaughlin_limit.blocks should load");

    assert!(model.is_sparse());
    assert_eq!(model.block_count(), 112_687);
    assert_eq!(
        model
            .metadata()
            .get("benchmark_family")
            .expect("metadata should contain benchmark family"),
        &MetadataValue::Text("mclaughlin-limit".to_owned())
    );
}

#[test]
#[ignore = "heavy benchmark fixture"]
fn load_staged_mclaughlin_blocks_as_sparse_model() {
    let model = read_benchmark_blocks(
        benchmark_blocks_path("mclaughlin", "mclaughlin.blocks"),
        "mclaughlin",
    )
    .expect("mclaughlin.blocks should load");

    assert!(model.is_sparse());
    assert_eq!(model.block_count(), 2_140_342);
    assert_eq!(
        model
            .metadata()
            .get("benchmark_family")
            .expect("metadata should contain benchmark family"),
        &MetadataValue::Text("mclaughlin".to_owned())
    );
}
