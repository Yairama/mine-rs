//! Tests de integración para la carga de `marvin.blocks`.

use std::path::PathBuf;

use mine_core::ColumnId;
use mine_io::read_marvin_blocks;

fn marvin_blocks_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("datasets")
        .join("benchmarks")
        .join("marvin")
        .join("marvin.blocks")
}

#[test]
fn load_staged_marvin_blocks_as_sparse_model() {
    let model = read_marvin_blocks(marvin_blocks_path()).expect("marvin.blocks should load");

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
        &mine_core::MetadataValue::Text("marvin".to_owned())
    );
}
