//! Tests de integración para los flujos públicos de `mine-io`.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ::parquet::arrow::ArrowWriter;
use ::parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use arrow::array::{ArrayRef, BooleanArray, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use mine_blockmodel::{
    BlockModel, ColumnData, ExperimentalVariogram, SpatialSample, VariogramDirection,
    VariogramLagConfig, build_experimental_variogram,
};
use mine_core::{
    BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema, ColumnSchemaSet,
    Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata, MineError,
};
use mine_io::{
    CsvIndexColumns, CsvReadOptions, CsvWriteOptions, SchemaInferenceHints,
    SchemaInferenceWarningCode, VtuWriteOptions, VulcanBooleanFormat, VulcanCoordinateColumns,
    VulcanCsvWriteOptions, block_model_from_record_batch, block_model_to_record_batch,
    experimental_variogram_from_record_batch, experimental_variogram_to_record_batch,
    infer_csv_schema, infer_parquet_schema, read_block_model_csv, read_block_model_parquet,
    read_experimental_variogram_json, read_experimental_variogram_parquet, write_block_model_csv,
    write_block_model_parquet, write_block_model_vtu, write_block_model_vulcan_csv,
    write_experimental_variogram_json, write_experimental_variogram_parquet,
};

const PARQUET_GRID_METADATA_KEY: &str = "mine_rs:grid_definition";
const PARQUET_SCHEMA_METADATA_KEY: &str = "mine_rs:column_schema";
const PARQUET_MODEL_METADATA_KEY: &str = "mine_rs:model_metadata";

#[test]
fn csv_roundtrip_preserves_basic_data() {
    let model = sample_model();
    let path = temporary_csv_path("roundtrip");
    let options = CsvWriteOptions::new(
        CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
        Some(vec![
            ColumnId::new("bench").expect("column should be valid"),
            ColumnId::new("active").expect("column should be valid"),
            ColumnId::new("cu").expect("column should be valid"),
            ColumnId::new("phase").expect("column should be valid"),
            ColumnId::new("tonnes").expect("column should be valid"),
        ]),
    )
    .expect("write options should be valid");

    write_block_model_csv(&model, &path, &options).expect("CSV write should succeed");
    let decoded = read_block_model_csv(
        &path,
        &CsvReadOptions::new(
            model.grid().clone(),
            model.schema().clone(),
            model.metadata().clone(),
            CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
        ),
    )
    .expect("CSV read should succeed");
    let csv = fs::read_to_string(&path).expect("CSV file should exist");

    assert_eq!(decoded, model);
    assert!(csv.starts_with("i,j,k,bench,active,cu,phase,tonnes"));

    let _ = fs::remove_file(path);
}

#[test]
fn read_csv_reports_missing_schema_column() {
    let model = sample_model();
    let path = temporary_csv_path("missing-column");
    fs::write(
        &path,
        "i,j,k,cu,tonnes\n0,0,0,0.7,10.0\n1,0,0,0.9,12.0\n0,0,1,1.1,11.0\n1,0,1,1.3,13.0\n",
    )
    .expect("fixture should be written");

    let error = read_block_model_csv(
        &path,
        &CsvReadOptions::new(
            model.grid().clone(),
            model.schema().clone(),
            Metadata::new(),
            CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
        ),
    )
    .expect_err("missing column should fail");

    assert_eq!(
        error,
        MineError::schema("column `active` is missing from CSV header")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn write_csv_uses_materialized_sparse_rows() {
    let model = sparse_model();
    let path = temporary_csv_path("sparse-write");
    let options = CsvWriteOptions::new(
        CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
        Some(vec![
            ColumnId::new("bench").expect("column should be valid"),
            ColumnId::new("cu").expect("column should be valid"),
        ]),
    )
    .expect("write options should be valid");

    write_block_model_csv(&model, &path, &options).expect("sparse CSV write should succeed");
    let csv = fs::read_to_string(&path).expect("CSV file should exist");

    assert!(csv.contains("0,0,0,10,0.7"));
    assert!(csv.contains("1,0,1,13,1.3"));
    assert!(!csv.contains("1,0,0,"));

    let _ = fs::remove_file(path);
}

#[test]
fn read_csv_reports_duplicate_and_missing_blocks() {
    let model = sample_model();
    let duplicate_path = temporary_csv_path("duplicate-block");
    fs::write(
        &duplicate_path,
        "i,j,k,active,bench,cu,phase,tonnes\n0,0,0,true,10,0.7,A,10.0\n0,0,0,false,11,0.9,B,12.0\n0,0,1,true,12,1.1,A,11.0\n1,0,1,false,13,1.3,B,13.0\n",
    )
    .expect("fixture should be written");

    let duplicate_error = read_block_model_csv(
        &duplicate_path,
        &CsvReadOptions::new(
            model.grid().clone(),
            model.schema().clone(),
            Metadata::new(),
            CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
        ),
    )
    .expect_err("duplicate block should fail");

    assert_eq!(
        duplicate_error,
        MineError::validation("CSV row 3 duplicates block index (0, 0, 0)")
    );

    let missing_path = temporary_csv_path("missing-block");
    fs::write(
        &missing_path,
        "i,j,k,active,bench,cu,phase,tonnes\n0,0,0,true,10,0.7,A,10.0\n1,0,0,false,11,0.9,B,12.0\n1,0,1,false,13,1.3,B,13.0\n",
    )
    .expect("fixture should be written");

    let missing_error = read_block_model_csv(
        &missing_path,
        &CsvReadOptions::new(
            model.grid().clone(),
            model.schema().clone(),
            Metadata::new(),
            CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
        ),
    )
    .expect_err("missing block should fail");

    assert_eq!(
        missing_error,
        MineError::validation("CSV data is missing block index (0, 0, 1)")
    );

    let _ = fs::remove_file(duplicate_path);
    let _ = fs::remove_file(missing_path);
}

#[test]
fn write_vulcan_csv_applies_aliases_and_boolean_format() {
    let model = sample_model();
    let path = temporary_csv_path("vulcan-export");
    let options = VulcanCsvWriteOptions::new(
        VulcanCoordinateColumns::new("xworld", "yworld", "zworld")
            .expect("coordinate columns should be valid"),
        Some(CsvIndexColumns::new("ix", "iy", "iz").expect("index columns should be valid")),
        Some(vec![
            ColumnId::new("bench").expect("column should be valid"),
            ColumnId::new("active").expect("column should be valid"),
            ColumnId::new("cu").expect("column should be valid"),
        ]),
        BTreeMap::from([
            (
                ColumnId::new("bench").expect("column should be valid"),
                "bench_rl".to_owned(),
            ),
            (
                ColumnId::new("active").expect("column should be valid"),
                "is_active".to_owned(),
            ),
        ]),
        VulcanBooleanFormat::ZeroOne,
    )
    .expect("Vulcan options should be valid");

    write_block_model_vulcan_csv(&model, &path, &options).expect("Vulcan CSV write should succeed");
    let csv = fs::read_to_string(&path).expect("CSV file should exist");

    assert!(csv.starts_with("xworld,yworld,zworld,ix,iy,iz,bench_rl,is_active,cu"));
    assert!(csv.contains("105,205,302.5,0,0,0,10,1,0.7"));

    let _ = fs::remove_file(path);
}

#[test]
fn parquet_roundtrip_preserves_schema_and_metadata() {
    let model = sample_model();
    let path = temporary_parquet_path("roundtrip");

    write_block_model_parquet(&model, &path).expect("Parquet write should succeed");
    let decoded = read_block_model_parquet(&path).expect("Parquet read should succeed");
    let standard_schema = ParquetRecordBatchReaderBuilder::try_new(
        File::open(&path).expect("Parquet file should exist"),
    )
    .expect("Parquet file should be readable")
    .schema()
    .clone();

    assert_eq!(decoded, model);
    assert!(standard_schema.field_with_name("i").is_ok());
    assert!(standard_schema.field_with_name("cu").is_ok());

    let _ = fs::remove_file(path);
}

#[test]
fn block_model_to_record_batch_rejects_sparse_models() {
    let error =
        block_model_to_record_batch(&sparse_model()).expect_err("sparse record batch should fail");

    assert_eq!(
        error,
        MineError::validation(
            "block_model_to_record_batch does not support sparse block models yet"
        )
    );
}

#[test]
fn read_parquet_reports_missing_embedded_metadata() {
    let path = temporary_parquet_path("missing-metadata");
    let schema = Arc::new(Schema::new(vec![
        Field::new("i", DataType::Int64, false),
        Field::new("j", DataType::Int64, false),
        Field::new("k", DataType::Int64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![0_i64])) as ArrayRef,
            Arc::new(Int64Array::from(vec![0_i64])) as ArrayRef,
            Arc::new(Int64Array::from(vec![0_i64])) as ArrayRef,
        ],
    )
    .expect("batch should be valid");
    let mut writer = ArrowWriter::try_new(
        File::create(&path).expect("file should be created"),
        schema,
        None,
    )
    .expect("writer should be created");
    writer.write(&batch).expect("batch should be written");
    writer.close().expect("writer should be closed");

    let error = read_block_model_parquet(&path).expect_err("missing metadata should fail clearly");

    assert_eq!(
        error,
        MineError::schema("Arrow schema is missing `grid definition` metadata")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn read_parquet_reports_duplicate_block_indices() {
    let model = sample_model();
    let path = temporary_parquet_path("duplicate-block");
    let batch = fixture_parquet_batch(
        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("i", DataType::Int64, false),
                Field::new("j", DataType::Int64, false),
                Field::new("k", DataType::Int64, false),
                Field::new("active", DataType::Boolean, false),
                Field::new("bench", DataType::Int64, false),
                Field::new("cu", DataType::Float64, false),
                Field::new("phase", DataType::Utf8, false),
                Field::new("tonnes", DataType::Float64, false),
            ],
            parquet_fixture_metadata(&model),
        )),
        vec![
            Arc::new(Int64Array::from(vec![0_i64, 0, 0, 1])) as ArrayRef,
            Arc::new(Int64Array::from(vec![0_i64, 0, 0, 0])) as ArrayRef,
            Arc::new(Int64Array::from(vec![0_i64, 0, 1, 1])) as ArrayRef,
            Arc::new(BooleanArray::from(vec![true, false, true, false])) as ArrayRef,
            Arc::new(Int64Array::from(vec![10_i64, 11, 12, 13])) as ArrayRef,
            Arc::new(Float64Array::from(vec![0.7_f64, 0.9, 1.1, 1.3])) as ArrayRef,
            Arc::new(StringArray::from(vec!["A", "B", "A", "B"])) as ArrayRef,
            Arc::new(Float64Array::from(vec![10.0_f64, 12.0, 11.0, 13.0])) as ArrayRef,
        ],
    );

    write_parquet_fixture(&path, batch);

    let error = read_block_model_parquet(&path).expect_err("duplicate block should fail clearly");

    assert_eq!(
        error,
        MineError::validation("Parquet row 2 duplicates block index (0, 0, 0)")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn write_vtu_exports_geometry_and_selected_attributes() {
    let model = sample_model();
    let path = temporary_vtu_path("selected-columns");
    let options = VtuWriteOptions::new(Some(vec![
        ColumnId::new("active").expect("column should be valid"),
        ColumnId::new("bench").expect("column should be valid"),
        ColumnId::new("cu").expect("column should be valid"),
    ]))
    .expect("VTU options should be valid");

    write_block_model_vtu(&model, &path, &options).expect("VTU write should succeed");
    let xml = fs::read_to_string(&path).expect("VTU file should exist");

    assert!(xml.contains("<VTKFile type=\"UnstructuredGrid\""));
    assert!(xml.contains("NumberOfCells=\"4\""));
    assert!(xml.contains("NumberOfPoints=\"32\""));
    assert!(xml.contains("Name=\"active\""));
    assert!(xml.contains("Name=\"bench\""));
    assert!(xml.contains("Name=\"cu\""));
    assert!(!xml.contains("Name=\"phase\""));
    assert!(xml.contains("Name=\"types\" format=\"ascii\">12 12 12 12 "));
    assert!(xml.contains("100 200 300"));

    let _ = fs::remove_file(path);
}

#[test]
fn write_vtu_rejects_text_columns_and_rotated_grids() {
    let text_path = temporary_vtu_path("text-column");
    let text_error = write_block_model_vtu(
        &sample_model(),
        &text_path,
        &VtuWriteOptions::new(Some(vec![
            ColumnId::new("phase").expect("column should be valid"),
        ]))
        .expect("VTU options should be valid"),
    )
    .expect_err("text column should fail");

    assert_eq!(
        text_error,
        MineError::schema(
            "column `phase` has logical type `text`, which is not supported by the VTU exporter"
        )
    );

    let rotated_path = temporary_vtu_path("rotated-grid");
    let rotated_error = write_block_model_vtu(
        &rotated_model(),
        &rotated_path,
        &VtuWriteOptions::new(None).expect("VTU options should be valid"),
    )
    .expect_err("rotated grid should fail");

    assert_eq!(
        rotated_error,
        MineError::grid("write_block_model_vtu does not support rotated grids yet")
    );

    let sparse_error = write_block_model_vtu(
        &sparse_model(),
        temporary_csv_path("sparse-vtu").with_extension("vtu"),
        &VtuWriteOptions::new(None).expect("VTU options should be valid"),
    )
    .expect_err("sparse VTU should fail clearly");

    assert_eq!(
        sparse_error,
        MineError::validation("write_block_model_vtu does not support sparse block models yet")
    );
}

#[test]
fn record_batch_roundtrip_preserves_model() {
    let model = sample_model();
    let batch = block_model_to_record_batch(&model).expect("record batch should be built");
    let decoded =
        block_model_from_record_batch(&batch).expect("record batch should reconstruct model");

    assert_eq!(decoded, model);
}

#[test]
fn record_batch_reports_missing_embedded_metadata() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("i", DataType::Int64, false),
            Field::new("j", DataType::Int64, false),
            Field::new("k", DataType::Int64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0_i64])) as ArrayRef,
            Arc::new(Int64Array::from(vec![0_i64])) as ArrayRef,
            Arc::new(Int64Array::from(vec![0_i64])) as ArrayRef,
        ],
    )
    .expect("batch should be valid");

    let error =
        block_model_from_record_batch(&batch).expect_err("missing metadata should fail clearly");

    assert_eq!(
        error,
        MineError::schema("Arrow schema is missing `grid definition` metadata")
    );
}

#[test]
fn infer_csv_schema_emits_controlled_warnings_for_critical_columns() {
    let path = temporary_csv_path("infer-csv");
    fs::write(
        &path,
        "i,j,k,cu,tonnes,phase\n0,0,0,0.7,10.0,A\n1,0,0,0.9,12.0,B\n",
    )
    .expect("fixture should be written");

    let inferred = infer_csv_schema(
        &path,
        &SchemaInferenceHints {
            index_columns: Some(
                CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
            ),
            ..SchemaInferenceHints::default()
        },
    )
    .expect("schema inference should succeed");

    assert_eq!(inferred.schema().len(), 3);
    assert_eq!(
        inferred
            .schema()
            .get(&ColumnId::new("cu").expect("column should be valid"))
            .expect("column should exist")
            .mining_role(),
        ColumnMiningRole::Other
    );
    assert!(inferred.warnings().iter().any(|warning| {
        warning.code == SchemaInferenceWarningCode::TonnageColumnRequiresConfirmation
            && warning
                .column
                .as_ref()
                .is_some_and(|column| column.as_str() == "tonnes")
    }));
    assert!(inferred.warnings().iter().any(|warning| {
        warning.code == SchemaInferenceWarningCode::GradeColumnRequiresConfirmation
            && warning
                .column
                .as_ref()
                .is_some_and(|column| column.as_str() == "cu")
    }));

    let _ = fs::remove_file(path);
}

#[test]
fn infer_csv_schema_applies_explicit_hints_to_roles() {
    let path = temporary_csv_path("infer-csv-hints");
    fs::write(
        &path,
        "i,j,k,cu,tonnes,phase\n0,0,0,0.7,10.0,A\n1,0,0,0.9,12.0,B\n",
    )
    .expect("fixture should be written");

    let inferred = infer_csv_schema(
        &path,
        &SchemaInferenceHints {
            index_columns: Some(
                CsvIndexColumns::new("i", "j", "k").expect("index columns should be valid"),
            ),
            grade_columns: vec![ColumnId::new("cu").expect("column should be valid")],
            tonnage_column: Some(ColumnId::new("tonnes").expect("column should be valid")),
            phase_column: Some(ColumnId::new("phase").expect("column should be valid")),
            ..SchemaInferenceHints::default()
        },
    )
    .expect("schema inference should succeed");

    assert!(inferred.warnings().is_empty());
    assert_eq!(
        inferred
            .schema()
            .get(&ColumnId::new("cu").expect("column should be valid"))
            .expect("column should exist")
            .mining_role(),
        ColumnMiningRole::Grade
    );
    assert_eq!(
        inferred
            .schema()
            .get(&ColumnId::new("tonnes").expect("column should be valid"))
            .expect("column should exist")
            .mining_role(),
        ColumnMiningRole::Tonnage
    );
    assert_eq!(
        inferred
            .schema()
            .get(&ColumnId::new("phase").expect("column should be valid"))
            .expect("column should exist")
            .mining_role(),
        ColumnMiningRole::Phase
    );

    let _ = fs::remove_file(path);
}

#[test]
fn infer_parquet_schema_recovers_embedded_schema() {
    let model = sample_model();
    let path = temporary_parquet_path("infer-parquet");

    write_block_model_parquet(&model, &path).expect("Parquet write should succeed");
    let inferred = infer_parquet_schema(&path, &SchemaInferenceHints::default())
        .expect("schema inference should succeed");

    assert_eq!(inferred.schema(), model.schema());
    assert!(inferred.warnings().is_empty());

    let _ = fs::remove_file(path);
}

#[test]
fn variogram_json_roundtrip_preserves_lag_rows() {
    let variogram = sample_variogram();
    let path = temporary_json_path("variogram-roundtrip");

    write_experimental_variogram_json(&variogram, &path)
        .expect("variogram JSON write should succeed");
    let decoded =
        read_experimental_variogram_json(&path).expect("variogram JSON read should succeed");
    let json = fs::read_to_string(&path).expect("JSON file should exist");

    assert_eq!(decoded, variogram);
    assert!(json.contains("\"lag_index\""));
    assert!(json.contains("\"semivariance\""));

    let _ = fs::remove_file(path);
}

#[test]
fn variogram_parquet_roundtrip_preserves_lag_rows() {
    let variogram = sample_variogram();
    let path = temporary_parquet_path("variogram-roundtrip");

    write_experimental_variogram_parquet(&variogram, &path)
        .expect("variogram Parquet write should succeed");
    let decoded =
        read_experimental_variogram_parquet(&path).expect("variogram Parquet read should succeed");
    let batch = experimental_variogram_to_record_batch(&variogram)
        .expect("variogram batch should be built");
    let decoded_from_batch = experimental_variogram_from_record_batch(&batch)
        .expect("variogram batch should reconstruct");

    assert_eq!(decoded, variogram);
    assert_eq!(decoded_from_batch, variogram);

    let _ = fs::remove_file(path);
}

fn sample_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(100.0, 200.0, 300.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 5.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 2).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");
    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("active").expect("column should be valid"),
            ColumnLogicalType::Boolean,
            None,
            false,
            ColumnMiningRole::Other,
        ),
        ColumnSchema::new(
            ColumnId::new("bench").expect("column should be valid"),
            ColumnLogicalType::Integer,
            None,
            false,
            ColumnMiningRole::Bench,
        ),
        ColumnSchema::new(
            ColumnId::new("cu").expect("column should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu").expect("unit should be valid")),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("phase").expect("column should be valid"),
            ColumnLogicalType::Text,
            None,
            false,
            ColumnMiningRole::Phase,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes").expect("column should be valid"),
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t").expect("unit should be valid")),
            false,
            ColumnMiningRole::Tonnage,
        ),
    ])
    .expect("schema should be valid");
    let metadata = Metadata::from_entries([(
        "site".to_owned(),
        mine_core::MetadataValue::Text("demo".to_owned()),
    )])
    .expect("metadata should be valid");
    let mut columns = BTreeMap::new();
    columns.insert(
        ColumnId::new("active").expect("column should be valid"),
        ColumnData::Booleans(vec![true, false, true, false]),
    );
    columns.insert(
        ColumnId::new("bench").expect("column should be valid"),
        ColumnData::Integers(vec![10, 11, 12, 13]),
    );
    columns.insert(
        ColumnId::new("cu").expect("column should be valid"),
        ColumnData::Floats(vec![0.7, 0.9, 1.1, 1.3]),
    );
    columns.insert(
        ColumnId::new("phase").expect("column should be valid"),
        ColumnData::Texts(vec![
            "A".to_owned(),
            "B".to_owned(),
            "A".to_owned(),
            "B".to_owned(),
        ]),
    );
    columns.insert(
        ColumnId::new("tonnes").expect("column should be valid"),
        ColumnData::Floats(vec![10.0, 12.0, 11.0, 13.0]),
    );

    BlockModel::new(grid, schema, metadata, columns).expect("sample model should be valid")
}

fn rotated_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(100.0, 200.0, 300.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 5.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 2).expect("shape should be valid"),
        Some(15.0),
    )
    .expect("grid should be valid");
    let model = sample_model();
    let columns = ["active", "bench", "cu", "phase", "tonnes"]
        .into_iter()
        .map(|name| {
            let column_id = ColumnId::new(name).expect("column should be valid");
            let column_data = model
                .column(&column_id)
                .expect("column should exist in sample model")
                .clone();
            (column_id, column_data)
        })
        .collect::<BTreeMap<_, _>>();

    BlockModel::new(
        grid,
        model.schema().clone(),
        model.metadata().clone(),
        columns,
    )
    .expect("rotated sample model should be valid")
}

fn sparse_model() -> BlockModel {
    let grid = GridDefinition::new(
        Coordinate3D::new(100.0, 200.0, 300.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 5.0).expect("dimensions should be valid"),
        GridShape::new(2, 1, 2).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid");
    let schema = sample_model().schema().clone();
    let metadata = sample_model().metadata().clone();
    let columns = BTreeMap::from([
        (
            ColumnId::new("active").expect("column should be valid"),
            ColumnData::Booleans(vec![true, false]),
        ),
        (
            ColumnId::new("bench").expect("column should be valid"),
            ColumnData::Integers(vec![10, 13]),
        ),
        (
            ColumnId::new("cu").expect("column should be valid"),
            ColumnData::Floats(vec![0.7, 1.3]),
        ),
        (
            ColumnId::new("phase").expect("column should be valid"),
            ColumnData::Texts(vec!["A".to_owned(), "B".to_owned()]),
        ),
        (
            ColumnId::new("tonnes").expect("column should be valid"),
            ColumnData::Floats(vec![10.0, 13.0]),
        ),
    ]);

    BlockModel::new_sparse(grid, schema, metadata, vec![0, 3], columns)
        .expect("sparse model should be valid")
}

fn sample_variogram() -> ExperimentalVariogram {
    let samples = vec![
        SpatialSample::new(
            "sample-01",
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column should be valid"), 0.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-02",
            Coordinate3D::new(1.0, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column should be valid"), 1.0)]),
        )
        .expect("sample should be valid"),
        SpatialSample::new(
            "sample-03",
            Coordinate3D::new(0.0, 1.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
            BTreeMap::from([(ColumnId::new("cu").expect("column should be valid"), 3.0)]),
        )
        .expect("sample should be valid"),
    ];

    build_experimental_variogram(
        &samples,
        &ColumnId::new("cu").expect("column should be valid"),
        &VariogramLagConfig::new(1.0, 1, 0.1).expect("lag config should be valid"),
        Some(
            &VariogramDirection::new(
                Coordinate3D::new(1.0, 0.0, 0.0).expect("direction should be valid"),
                10.0,
                Some(0.25),
            )
            .expect("direction should be valid"),
        ),
        Some("ore"),
    )
    .expect("sample variogram should be valid")
}

fn temporary_csv_path(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();

    std::env::temp_dir().join(format!("mine-rs-{label}-{timestamp}.csv"))
}

fn temporary_json_path(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();

    std::env::temp_dir().join(format!("mine-rs-{label}-{timestamp}.json"))
}

fn temporary_parquet_path(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();

    std::env::temp_dir().join(format!("mine-rs-{label}-{timestamp}.parquet"))
}

fn temporary_vtu_path(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();

    std::env::temp_dir().join(format!("mine-rs-{label}-{timestamp}.vtu"))
}

fn parquet_fixture_metadata(model: &BlockModel) -> HashMap<String, String> {
    HashMap::from([
        (
            PARQUET_GRID_METADATA_KEY.to_owned(),
            serde_json::to_string(model.grid()).expect("grid should serialize"),
        ),
        (
            PARQUET_SCHEMA_METADATA_KEY.to_owned(),
            serde_json::to_string(
                &model
                    .schema()
                    .iter()
                    .map(|(_, column_schema)| column_schema.clone())
                    .collect::<Vec<_>>(),
            )
            .expect("schema should serialize"),
        ),
        (
            PARQUET_MODEL_METADATA_KEY.to_owned(),
            serde_json::to_string(model.metadata()).expect("metadata should serialize"),
        ),
    ])
}

fn fixture_parquet_batch(schema: Arc<Schema>, arrays: Vec<ArrayRef>) -> RecordBatch {
    RecordBatch::try_new(schema, arrays).expect("fixture batch should be valid")
}

fn write_parquet_fixture(path: &Path, batch: RecordBatch) {
    let mut writer = ArrowWriter::try_new(
        File::create(path).expect("fixture file should be created"),
        batch.schema(),
        None,
    )
    .expect("fixture writer should be created");
    writer
        .write(&batch)
        .expect("fixture batch should be written");
    writer.close().expect("fixture writer should be closed");
}
