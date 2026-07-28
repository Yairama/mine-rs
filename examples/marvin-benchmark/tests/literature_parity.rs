//! Valida la tabla viva de paridad (`docs/references/literature-parity.md`)
//! contra reportes JSON generados localmente (MR-217).
//!
//! Si un harness regenera reportes con valores distintos, estos tests fallan
//! hasta que la tabla se actualice, evitando divergencia manual entre docs y
//! artefactos. Se ejecutan explícitamente porque los reportes no forman parte
//! de un checkout limpio.

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
#[ignore = "requires locally generated UPIT benchmark report"]
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
#[ignore = "requires locally generated CPIT TopoSort benchmark report"]
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
#[ignore = "requires locally generated PCPSP TopoSort benchmark report"]
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
#[ignore = "requires locally generated PCPSP TopoSort benchmark report"]
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

/// Los bounds Lagrangianos propios y el candidato self-contained citados en la
/// tabla deben coincidir con `pcpsp-bound-report.json` (MR-213).
#[test]
#[ignore = "requires locally generated PCPSP bound benchmark report"]
fn parity_table_matches_pcpsp_bound_report() {
    let report = read_json("datasets/benchmarks/outputs/pcpsp-bound-report.json");
    let doc = read_parity_doc();

    let formulations = report["formulations"]
        .as_array()
        .expect("report should contain formulations");
    assert!(
        !formulations.is_empty(),
        "pcpsp bound report should not be empty"
    );

    for formulation in formulations {
        let run_id = formulation["run_id"].as_str().expect("run_id");

        // El bound debe declarar cobertura completa de precedencias y quedar
        // por encima del LP oficial (validez metodológica).
        let coverage = formulation["precedence_coverage_completeness"]
            .as_str()
            .expect("coverage field");
        assert!(
            coverage.starts_with("complete"),
            "run `{run_id}` lost full precedence coverage: {coverage}"
        );
        let bound = formulation["best_bound"].as_f64().expect("best_bound");
        let official_lp = formulation["official_lp_objective"]
            .as_f64()
            .expect("official_lp_objective");
        assert!(
            bound >= official_lp - 1.0,
            "run `{run_id}` bound {bound} fell below the official LP {official_lp}; \
             a valid relaxation bound cannot do that"
        );
        let formatted_bound = format!("`{bound:.3}`");
        assert!(
            doc.contains(&formatted_bound),
            "literature-parity.md is missing own bound {formatted_bound} for run `{run_id}`"
        );

        let candidate = &formulation["self_contained_candidate"];
        assert_eq!(
            candidate["audit_objective_consistent"], true,
            "run `{run_id}` self-contained candidate failed objective audit"
        );
        assert_eq!(
            candidate["precedence_feasibility_verified"], true,
            "run `{run_id}` self-contained candidate failed precedence verification"
        );
        let max_excess = candidate["audited_max_resource_excess"]
            .as_f64()
            .expect("audited_max_resource_excess");
        assert!(
            max_excess <= 1.0e-6,
            "run `{run_id}` self-contained candidate exceeds resource limits by {max_excess}"
        );
    }

    // El mejor candidato CPIT de Marvin citado en la tabla es el self-contained.
    let marvin_cpit = formulations
        .iter()
        .find(|formulation| formulation["run_id"] == "marvin-cpit")
        .expect("marvin-cpit run should be present");
    let candidate_objective = marvin_cpit["self_contained_candidate"]["discounted_objective"]
        .as_f64()
        .expect("discounted_objective");
    let formatted_candidate = format!("`{candidate_objective:.3}`");
    assert!(
        doc.contains(&formatted_candidate),
        "literature-parity.md is missing self-contained CPIT candidate {formatted_candidate}"
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
