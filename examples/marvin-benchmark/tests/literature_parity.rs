//! Valida la tabla viva de paridad (`docs/references/literature-parity.md`)
//! contra los reportes JSON versionados (MR-217).
//!
//! Si un harness regenera reportes con valores distintos, estos tests fallan
//! hasta que la tabla se actualice, evitando divergencia manual entre docs y
//! artefactos.

#[path = "../src/benchmark_path_policy.rs"]
mod benchmark_path_policy;

use std::fs;
use std::path::PathBuf;

use benchmark_path_policy::BenchmarkPathPolicy;
use serde_json::Value;

fn repo_path(relative: &str) -> PathBuf {
    BenchmarkPathPolicy::discover()
        .expect("repo root should resolve")
        .repo_root()
        .join(relative)
}

fn read_json(relative: &str) -> Value {
    let path = repo_path(relative);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("unable to read {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("unable to parse {}: {error}", path.display()))
}

fn read_parity_doc() -> String {
    let path = repo_path("docs/references/literature-parity.md");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("unable to read {}: {error}", path.display()))
}

/// Los valores UPIT de la tabla deben coincidir con `upit-runtime-report.json`
/// y todos los datasets medidos deben reproducir el objetivo oficial.
#[test]
fn parity_table_matches_upit_runtime_report() {
    let report = read_json("datasets/benchmarks/outputs/upit-runtime-report.json");
    let doc = read_parity_doc();

    let datasets = report["datasets"]
        .as_array()
        .expect("report should contain datasets");
    assert!(
        !datasets.is_empty(),
        "upit runtime report should not be empty"
    );

    for dataset in datasets {
        let dataset_id = dataset["dataset_id"].as_str().expect("dataset_id");
        assert_eq!(
            dataset["matches_official_objective"], true,
            "dataset `{dataset_id}` no longer matches the official UPIT objective"
        );
        let pit_value = dataset["pit_value"].as_f64().expect("pit_value");
        let formatted = format!("`{pit_value:.3}`");
        assert!(
            doc.contains(&formatted),
            "literature-parity.md is missing UPIT value {formatted} for dataset `{dataset_id}`"
        );
    }
}

/// Los candidatos CPIT de la tabla deben coincidir con el mejor candidato
/// auditado del reporte TopoSort (variante delayed-waste).
#[test]
fn parity_table_matches_cpit_toposort_report() {
    let report = read_json("datasets/benchmarks/outputs/cpit-toposort-report.json");
    let doc = read_parity_doc();

    let datasets = report["datasets"]
        .as_array()
        .expect("report should contain datasets");
    assert!(
        !datasets.is_empty(),
        "cpit toposort report should not be empty"
    );

    for dataset in datasets {
        let dataset_id = dataset["dataset_id"].as_str().expect("dataset_id");
        let delayed = dataset["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .find(|candidate| candidate["variant"] == "toposort-delayed-waste")
            .unwrap_or_else(|| panic!("dataset `{dataset_id}` lacks delayed-waste candidate"));

        assert_eq!(
            delayed["audit_objective_consistent"], true,
            "dataset `{dataset_id}` delayed candidate failed objective audit"
        );
        assert_eq!(
            delayed["precedence_feasibility_verified"], true,
            "dataset `{dataset_id}` delayed candidate failed precedence verification"
        );
        let max_excess = delayed["audited_max_resource_excess"]
            .as_f64()
            .expect("audited_max_resource_excess");
        assert!(
            max_excess <= 1.0e-6,
            "dataset `{dataset_id}` delayed candidate exceeds resource limits by {max_excess}"
        );

        let objective = delayed["discounted_objective"]
            .as_f64()
            .expect("discounted_objective");
        let formatted = format!("`{objective:.3}`");
        assert!(
            doc.contains(&formatted),
            "literature-parity.md is missing CPIT value {formatted} for dataset `{dataset_id}`"
        );
    }
}

/// Los candidatos PCPSP de la tabla deben coincidir con el mejor candidato
/// auditado del reporte TopoSort multi-destino (variante delayed-waste).
#[test]
fn parity_table_matches_pcpsp_toposort_report() {
    let report = read_json("datasets/benchmarks/outputs/pcpsp-toposort-report.json");
    let doc = read_parity_doc();

    let datasets = report["datasets"]
        .as_array()
        .expect("report should contain datasets");
    assert!(
        !datasets.is_empty(),
        "pcpsp toposort report should not be empty"
    );

    for dataset in datasets {
        let dataset_id = dataset["dataset_id"].as_str().expect("dataset_id");
        let delayed = dataset["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .find(|candidate| candidate["variant"] == "toposort-delayed-waste")
            .unwrap_or_else(|| panic!("dataset `{dataset_id}` lacks delayed-waste candidate"));

        assert_eq!(
            delayed["audit_objective_consistent"], true,
            "dataset `{dataset_id}` delayed candidate failed objective audit"
        );
        assert_eq!(
            delayed["precedence_feasibility_verified"], true,
            "dataset `{dataset_id}` delayed candidate failed precedence verification"
        );
        let max_excess = delayed["audited_max_resource_excess"]
            .as_f64()
            .expect("audited_max_resource_excess");
        assert!(
            max_excess <= 1.0e-6,
            "dataset `{dataset_id}` delayed candidate exceeds resource limits by {max_excess}"
        );

        let objective = delayed["discounted_objective"]
            .as_f64()
            .expect("discounted_objective");
        let formatted = format!("`{objective:.3}`");
        assert!(
            doc.contains(&formatted),
            "literature-parity.md is missing PCPSP value {formatted} for dataset `{dataset_id}`"
        );
    }
}

/// Hitos cuantitativos de MR-212 sobre Marvin: el candidato PCPSP propio debe
/// superar la baseline `cpit-period-routed` (820,726,047.95) y mantener gap
/// de un dígito contra el objetivo oficial (885,968,070).
#[test]
fn marvin_pcpsp_candidate_meets_mr212_milestones() {
    let report = read_json("datasets/benchmarks/outputs/pcpsp-toposort-report.json");
    let marvin = report["datasets"]
        .as_array()
        .expect("datasets")
        .iter()
        .find(|dataset| dataset["dataset_id"] == "marvin")
        .expect("marvin dataset should be present");
    let delayed = marvin["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .find(|candidate| candidate["variant"] == "toposort-delayed-waste")
        .expect("delayed candidate should be present");

    let objective = delayed["discounted_objective"]
        .as_f64()
        .expect("discounted_objective");
    const CPIT_PERIOD_ROUTED_BASELINE: f64 = 820_726_047.95;
    const OFFICIAL_PCPSP: f64 = 885_968_070.0;
    assert!(
        objective > CPIT_PERIOD_ROUTED_BASELINE,
        "MR-212 milestone regression: marvin PCPSP candidate {objective} no longer beats the \
         cpit-period-routed baseline {CPIT_PERIOD_ROUTED_BASELINE}"
    );
    let gap = (OFFICIAL_PCPSP - objective) / OFFICIAL_PCPSP;
    assert!(
        gap < 0.10,
        "MR-212 milestone regression: marvin PCPSP gap {gap:.4} is no longer below 10%"
    );
}

/// La tabla debe declarar las instancias MineLib aún no staged (MR-208).
#[test]
fn parity_table_declares_missing_minelib_instances() {
    let doc = read_parity_doc();
    for instance in ["newman1", "zuck_", "kd", "p4hd", "w23", "sm2"] {
        assert!(
            doc.contains(instance),
            "literature-parity.md should mention pending MineLib instance `{instance}`"
        );
    }
    assert!(
        doc.contains("MR-208"),
        "literature-parity.md should reference MR-208 for pending staging"
    );
}
