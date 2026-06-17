//! Prototipo pequeño de selección de schedules bajo incertidumbre.
//!
//! Uso:
//!   cargo run -p stochastic-planning [output_path]
//!
//! Si no se especifica `output_path`, el reporte se escribe en
//! `datasets/benchmarks/synthetic/outputs/stochastic-planning-report.json`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use mine_sdk::{
    blockmodel::{
        BlockModel, ColumnData, EstimationPass, ExperimentalVariogram, ExperimentalVariogramLag,
        SampleCountLimits, SearchAnisotropy, SearchNeighborhood,
        SequentialGaussianSimulationOptions, SimulatedNodeValue, SimulationTarget, SpatialSample,
        VariogramFitSummary, VariogramLagConfig, VariogramModel, VariogramModelKind,
        generate_sequential_gaussian_ensemble,
    },
    core::{
        ArtifactId, BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
        ModelId, ScenarioId,
    },
    economics::{
        DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
        DestinationKind, DestinationPayability, DestinationRecovery, EconomicBlockModel,
        EconomicBlockModelConfig, evaluate_long_term_schedule_economics,
        summarize_long_term_schedule_risk,
    },
    experimental::PushbackPlan,
    planning::{
        LongTermSchedule, LongTermScheduleEntry, LongTermSchedulePeriodCapacity,
        NestingAccessRules, PhaseDesign,
    },
};
use serde::Serialize;

const REALIZATION_SEEDS: [u64; 4] = [7, 17, 29, 41];

#[derive(Debug, Serialize)]
struct RealizationSnapshot {
    realization_id: String,
    random_seed: u64,
    west_grade: f64,
    east_grade: f64,
}

#[derive(Debug, Serialize)]
struct CandidateScorecard {
    candidate_id: String,
    scenario_count: usize,
    npv_mean: f64,
    npv_p10: f64,
    npv_p50: f64,
    npv_p90: f64,
    npv_downside_probability: f64,
    npv_cvar10: f64,
    total_cashflow_mean: f64,
}

#[derive(Debug, Serialize)]
struct StochasticPlanningPrototypeOutput {
    prototype_name: String,
    selected_candidate_id: String,
    realization_count: usize,
    realizations: Vec<RealizationSnapshot>,
    candidate_scorecards: Vec<CandidateScorecard>,
    assumptions: Vec<String>,
    decision_criteria: Vec<String>,
    limitations: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let output_path = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        repo_root
            .join("datasets")
            .join("benchmarks")
            .join("synthetic")
            .join("outputs")
            .join("stochastic-planning-report.json")
    });

    let base_model_id = ModelId::new("synthetic-stochastic-model")?;
    let phase_plan = sample_phase_plan();
    let west_first_schedule =
        candidate_schedule(&base_model_id, "west-first", "phase-west", "phase-east")?;
    let east_first_schedule =
        candidate_schedule(&base_model_id, "east-first", "phase-east", "phase-west")?;
    let sgs_ensemble = generate_sequential_gaussian_ensemble(
        ArtifactId::new("synthetic-sgs-ensemble")?,
        base_model_id.clone(),
        ColumnId::new("cu")?,
        ArtifactId::new("grid.synthetic")?,
        vec![ArtifactId::new("conditioning.synthetic")?],
        &conditioning_samples(),
        &simulation_targets(),
        &[estimation_pass()?],
        &variogram_model()?,
        &REALIZATION_SEEDS,
        &SequentialGaussianSimulationOptions::new(1.1)?,
    )?;

    let mut west_first_reports = Vec::with_capacity(sgs_ensemble.realizations.len());
    let mut east_first_reports = Vec::with_capacity(sgs_ensemble.realizations.len());
    let mut realizations = Vec::with_capacity(sgs_ensemble.realizations.len());
    for realization in &sgs_ensemble.realizations {
        let economic_model = economic_block_model_from_realization(realization.values.as_slice())?;
        west_first_reports.push(evaluate_long_term_schedule_economics(
            &west_first_schedule,
            &phase_plan,
            &economic_model,
            0.1,
        )?);
        east_first_reports.push(evaluate_long_term_schedule_economics(
            &east_first_schedule,
            &phase_plan,
            &economic_model,
            0.1,
        )?);

        realizations.push(RealizationSnapshot {
            realization_id: realization.descriptor.realization_id().to_string(),
            random_seed: realization.descriptor.random_seed(),
            west_grade: grade_for_target(realization.values.as_slice(), "west-block")?,
            east_grade: grade_for_target(realization.values.as_slice(), "east-block")?,
        });
    }

    let west_first_risk = summarize_long_term_schedule_risk(&west_first_reports)?;
    let east_first_risk = summarize_long_term_schedule_risk(&east_first_reports)?;
    let candidate_scorecards = vec![
        CandidateScorecard {
            candidate_id: "west-first".to_owned(),
            scenario_count: west_first_risk.scenario_ids.len(),
            npv_mean: west_first_risk.npv.mean,
            npv_p10: west_first_risk.npv.p10,
            npv_p50: west_first_risk.npv.p50,
            npv_p90: west_first_risk.npv.p90,
            npv_downside_probability: west_first_risk.npv.downside_probability,
            npv_cvar10: west_first_risk.npv.cvar10,
            total_cashflow_mean: west_first_risk.total_cashflow.mean,
        },
        CandidateScorecard {
            candidate_id: "east-first".to_owned(),
            scenario_count: east_first_risk.scenario_ids.len(),
            npv_mean: east_first_risk.npv.mean,
            npv_p10: east_first_risk.npv.p10,
            npv_p50: east_first_risk.npv.p50,
            npv_p90: east_first_risk.npv.p90,
            npv_downside_probability: east_first_risk.npv.downside_probability,
            npv_cvar10: east_first_risk.npv.cvar10,
            total_cashflow_mean: east_first_risk.total_cashflow.mean,
        },
    ];
    let selected_candidate_id = select_candidate(&candidate_scorecards)
        .expect("scorecards should not be empty")
        .candidate_id
        .clone();
    let output = StochasticPlanningPrototypeOutput {
        prototype_name: "sgs-driven stochastic schedule ranking".to_owned(),
        selected_candidate_id,
        realization_count: realizations.len(),
        realizations,
        candidate_scorecards,
        assumptions: vec![
            "El ensemble geológico es pequeño y sintético: 2 bloques objetivo simulados con SGS sobre una línea 1D.".to_owned(),
            "Cada candidato usa el mismo PushbackPlan; solo cambia el orden fase-oeste / fase-este para observar sensibilidad temporal del valor.".to_owned(),
            "La valuación económica usa un único destino `mill` y un discount rate fijo de 10% por periodo.".to_owned(),
        ],
        decision_criteria: vec![
            "Seleccionar el candidato con mayor P50 de NPV.".to_owned(),
            "Si hay empate en P50, preferir menor downside_probability.".to_owned(),
            "Si persiste el empate, preferir mayor mean NPV.".to_owned(),
        ],
        limitations: vec![
            "Esto no es un solver estocástico industrial: compara schedules candidatos explícitos ya construidos.".to_owned(),
            "El ejemplo solo usa SGS; SIS y pits finales bajo incertidumbre siguen siendo trabajo experimental posterior.".to_owned(),
            "La relación entre realizaciones y bloques económicos es directa y sintética; no modela domaining, blending ni restricciones operativas complejas.".to_owned(),
        ],
    };

    let json = serde_json::to_string_pretty(&output)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, &json)?;
    eprintln!(
        "stochastic planning prototype written to {}",
        output_path.display()
    );
    println!("{json}");

    Ok(())
}

fn candidate_schedule(
    model_id: &ModelId,
    scenario_name: &str,
    first_phase: &str,
    second_phase: &str,
) -> Result<LongTermSchedule, Box<dyn std::error::Error>> {
    Ok(LongTermSchedule::new(
        ScenarioId::new(format!("scenario-{scenario_name}"))?,
        model_id.clone(),
        vec![
            LongTermScheduleEntry::new(
                "P1",
                Some(first_phase.to_owned()),
                Some(0),
                None,
                10.0,
                1,
                None,
                None,
                vec![],
            )?,
            LongTermScheduleEntry::new(
                "P2",
                Some(second_phase.to_owned()),
                Some(0),
                None,
                10.0,
                1,
                None,
                None,
                vec![],
            )?,
        ],
        vec![
            LongTermSchedulePeriodCapacity::new("P1", Some(10.0), None, vec![], vec![])?,
            LongTermSchedulePeriodCapacity::new("P2", Some(10.0), None, vec![], vec![])?,
        ],
        vec![],
        vec![],
        Metadata::new(),
    )?)
}

fn conditioning_samples() -> Vec<SpatialSample> {
    vec![
        sample("s1", 0.0, 0.8),
        sample("s2", 1.0, 1.1),
        sample("s3", 2.0, 1.6),
    ]
}

fn simulation_targets() -> Vec<SimulationTarget> {
    vec![
        SimulationTarget::new(
            "west-block",
            Coordinate3D::new(0.5, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
        )
        .expect("target should be valid"),
        SimulationTarget::new(
            "east-block",
            Coordinate3D::new(1.5, 0.0, 0.0).expect("coordinate should be valid"),
            Some("ore".to_owned()),
        )
        .expect("target should be valid"),
    ]
}

fn sample(sample_id: &str, x: f64, value: f64) -> SpatialSample {
    SpatialSample::new(
        sample_id,
        Coordinate3D::new(x, 0.0, 0.0).expect("coordinate should be valid"),
        Some("ore".to_owned()),
        BTreeMap::from([(
            ColumnId::new("cu").expect("column id should be valid"),
            value,
        )]),
    )
    .expect("sample should be valid")
}

fn estimation_pass() -> Result<EstimationPass, Box<dyn std::error::Error>> {
    Ok(EstimationPass::new(
        "primary",
        SearchNeighborhood::new(
            SearchAnisotropy::new(3.0, 3.0, 3.0, 0.0, 0.0, 0.0)?,
            Some(vec!["ore".to_owned()]),
        )?,
        SampleCountLimits::new(2, 3)?,
    )?)
}

fn variogram_model() -> Result<VariogramModel, Box<dyn std::error::Error>> {
    let variogram = ExperimentalVariogram {
        column_id: ColumnId::new("cu")?,
        domain: Some("ore".to_owned()),
        direction: None,
        lag_config: VariogramLagConfig::new(1.0, 2, 0.1)?,
        sample_count: 3,
        lags: vec![
            ExperimentalVariogramLag {
                lag_index: 1,
                lag_center: 1.0,
                pair_count: 2,
                average_distance: Some(1.0),
                semivariance: Some(0.367_187_5),
            },
            ExperimentalVariogramLag {
                lag_index: 2,
                lag_center: 2.0,
                pair_count: 1,
                average_distance: Some(2.0),
                semivariance: Some(0.687_5),
            },
        ],
    };

    Ok(VariogramModel::from_variogram(
        &variogram,
        VariogramModelKind::Spherical,
        0.0,
        1.0,
        Some(4.0),
        VariogramFitSummary {
            observed_lag_count: 2,
            total_pair_count: 3,
            weighted_sse: 0.0,
            rmse: 0.0,
            mean_absolute_error: 0.0,
        },
    )?)
}

fn economic_block_model_from_realization(
    values: &[SimulatedNodeValue],
) -> Result<EconomicBlockModel, Box<dyn std::error::Error>> {
    let grades = vec![
        grade_for_target(values, "west-block")?,
        grade_for_target(values, "east-block")?,
    ];
    let grid = GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0)?,
        BlockDimensions::new(10.0, 10.0, 10.0)?,
        GridShape::new(2, 1, 1)?,
        None,
    )?;
    let schema = ColumnSchemaSet::from_columns(vec![
        ColumnSchema::new(
            ColumnId::new("cu")?,
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("%Cu")?),
            false,
            ColumnMiningRole::Grade,
        ),
        ColumnSchema::new(
            ColumnId::new("tonnes")?,
            ColumnLogicalType::Float,
            Some(MeasurementUnit::new("t")?),
            false,
            ColumnMiningRole::Tonnage,
        ),
    ])?;
    let columns = BTreeMap::from([
        (ColumnId::new("cu")?, ColumnData::Floats(grades)),
        (
            ColumnId::new("tonnes")?,
            ColumnData::Floats(vec![10.0, 10.0]),
        ),
    ]);
    let block_model = BlockModel::new(grid, schema, Metadata::new(), columns)?;

    Ok(EconomicBlockModel::build(
        block_model,
        EconomicBlockModelConfig {
            tonnage_column: ColumnId::new("tonnes")?,
            grade_columns: vec![ColumnId::new("cu")?],
            destinations: DestinationAssumptionSet::new(vec![mill_destination()?])?,
        },
    )?)
}

fn mill_destination() -> Result<DestinationAssumptions, Box<dyn std::error::Error>> {
    Ok(DestinationAssumptions::new(
        DestinationId::new("mill")?,
        DestinationKind::Mill,
        2.0,
        8.0,
        vec![DestinationRecovery::new(ColumnId::new("cu")?, 0.9)?],
        vec![DestinationPayability::new(ColumnId::new("cu")?, 0.8)?],
        DestinationCapacity::new(None, MeasurementUnit::new("t")?)?,
        BTreeMap::from([("cu".to_owned(), 100.0)]),
    )?)
}

fn sample_phase_plan() -> PushbackPlan {
    PushbackPlan {
        phases: vec![
            PhaseDesign {
                phase_id: "phase-west".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(1.0),
                bench: Some(100),
                block_indices: vec![0],
                block_count: 1,
                total_tonnage: Some(10.0),
                predecessor_phase_ids: vec![],
            },
            PhaseDesign {
                phase_id: "phase-east".to_owned(),
                pushback_index: 0,
                shell_index: Some(0),
                revenue_factor: Some(1.0),
                bench: Some(100),
                block_indices: vec![1],
                block_count: 1,
                total_tonnage: Some(10.0),
                predecessor_phase_ids: vec![],
            },
        ],
        phase_count: 2,
        total_block_count: 2,
        total_tonnage: Some(20.0),
        nesting_rules: NestingAccessRules::strict_sequential(),
        limitations: vec![],
    }
}

fn grade_for_target(
    values: &[SimulatedNodeValue],
    target_id: &str,
) -> Result<f64, Box<dyn std::error::Error>> {
    values
        .iter()
        .find(|value| value.target_id == target_id)
        .map(|value| value.value)
        .ok_or_else(|| format!("target `{target_id}` is missing from simulated values").into())
}

fn select_candidate(scorecards: &[CandidateScorecard]) -> Option<&CandidateScorecard> {
    scorecards.iter().max_by(|left, right| {
        left.npv_p50
            .partial_cmp(&right.npv_p50)
            .expect("p50 should be finite")
            .then_with(|| {
                right
                    .npv_downside_probability
                    .partial_cmp(&left.npv_downside_probability)
                    .expect("downside should be finite")
            })
            .then_with(|| {
                left.npv_mean
                    .partial_cmp(&right.npv_mean)
                    .expect("mean should be finite")
            })
    })
}
