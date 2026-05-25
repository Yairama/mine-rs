//! Tests de integracion para contratos de realizaciones condicionales.

use mine_blockmodel::{
    ConditionalRealization, ConditionalRealizationLineage, ConditionalRealizationSet,
    RealizationStorageFormat, RealizationSupport,
};
use mine_core::{ArtifactId, ColumnId, Metadata, MineError, ModelId};

#[test]
fn build_conditional_realization_set_with_explicit_lineage() {
    let sampled_columns = sampled_columns();
    let support = sample_support();
    let realization_a = sample_realization(
        "realization-a",
        0,
        sampled_columns.clone(),
        support.clone(),
        101,
    );
    let realization_b = sample_realization(
        "realization-b",
        1,
        sampled_columns.clone(),
        support.clone(),
        202,
    );

    let ensemble = ConditionalRealizationSet::new(
        ArtifactId::new("ensemble-01").expect("artifact should be valid"),
        ModelId::new("model-01").expect("model should be valid"),
        sampled_columns,
        support,
        vec![realization_a.clone(), realization_b.clone()],
        Metadata::new(),
    )
    .expect("ensemble should be valid");

    assert_eq!(ensemble.realization_count(), 2);
    assert_eq!(ensemble.support().block_count(), 24);
    assert_eq!(ensemble.sampled_columns().len(), 2);
    assert_eq!(
        ensemble
            .realization(&ArtifactId::new("realization-b").expect("artifact should be valid"))
            .expect("realization should exist")
            .random_seed(),
        202
    );
}

#[test]
fn reject_realization_without_conditioning_artifacts() {
    let error = ConditionalRealizationLineage::new(vec![], None)
        .expect_err("missing lineage should fail");

    assert_eq!(
        error,
        MineError::invalid_parameter(
            "conditioning_artifact_ids",
            "conditional realizations require at least one conditioning artifact"
        )
    );
}

#[test]
fn reject_inconsistent_support_inside_ensemble() {
    let sampled_columns = sampled_columns();
    let realization = sample_realization(
        "realization-a",
        0,
        sampled_columns.clone(),
        RealizationSupport::new(
            12,
            false,
            ArtifactId::new("grid-small").expect("artifact should be valid"),
        )
        .expect("support should be valid"),
        101,
    );

    let error = ConditionalRealizationSet::new(
        ArtifactId::new("ensemble-01").expect("artifact should be valid"),
        ModelId::new("model-01").expect("model should be valid"),
        sampled_columns,
        sample_support(),
        vec![realization],
        Metadata::new(),
    )
    .expect_err("support mismatch should fail");

    assert_eq!(
        error,
        MineError::Validation {
            message: "realization `realization-a` does not match the ensemble support".to_owned(),
        }
    );
}

#[test]
fn reject_duplicate_realization_indices() {
    let sampled_columns = sampled_columns();
    let support = sample_support();

    let error = ConditionalRealizationSet::new(
        ArtifactId::new("ensemble-01").expect("artifact should be valid"),
        ModelId::new("model-01").expect("model should be valid"),
        sampled_columns.clone(),
        support.clone(),
        vec![
            sample_realization("realization-a", 0, sampled_columns.clone(), support.clone(), 101),
            sample_realization("realization-b", 0, sampled_columns, support, 202),
        ],
        Metadata::new(),
    )
    .expect_err("duplicate index should fail");

    assert_eq!(
        error,
        MineError::Validation {
            message: "duplicate realization index `0` in conditional realization set".to_owned(),
        }
    );
}

fn sample_support() -> RealizationSupport {
    RealizationSupport::new(
        24,
        false,
        ArtifactId::new("grid-main").expect("artifact should be valid"),
    )
    .expect("support should be valid")
}

fn sampled_columns() -> Vec<ColumnId> {
    vec![
        ColumnId::new("cu").expect("column should be valid"),
        ColumnId::new("au").expect("column should be valid"),
    ]
}

fn sample_realization(
    realization_id: &str,
    realization_index: usize,
    sampled_columns: Vec<ColumnId>,
    support: RealizationSupport,
    seed: u64,
) -> ConditionalRealization {
    ConditionalRealization::new(
        ArtifactId::new(realization_id).expect("artifact should be valid"),
        realization_index,
        ModelId::new("model-01").expect("model should be valid"),
        sampled_columns,
        ArtifactId::new(format!("{realization_id}.parquet")).expect("artifact should be valid"),
        RealizationStorageFormat::Parquet,
        "sgs",
        seed,
        support,
        ConditionalRealizationLineage::new(
            vec![ArtifactId::new("drillholes.parquet").expect("artifact should be valid")],
            Some(ArtifactId::new("sgs-config.json").expect("artifact should be valid")),
        )
        .expect("lineage should be valid"),
        Metadata::new(),
    )
    .expect("realization should be valid")
}
