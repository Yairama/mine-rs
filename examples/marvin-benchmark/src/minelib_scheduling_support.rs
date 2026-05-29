//! Helpers compartidos para validar scheduling MineLib con el mismo contrato core.
//!
//! Referencia minima:
//! - Espinoza, D., Goycoolea, M., Moreno, E., Newman, A. M. (2013).
//!   *MineLib: a library of open pit mining problems*.
//!   https://doi.org/10.1007/s10479-012-1258-3

use std::collections::{BTreeMap, BTreeSet};

use crate::marvin_support::{MinelibScheduleAssignment, MinelibScheduleProblem};
use mine_sdk::{
    BenchParameters, BlockModel, ColumnData, ColumnId, Metadata, MetadataValue, ModelId,
    NestingAccessRules, PhaseDesign, PitShellSet, PushbackPlan, ScenarioId, ScheduleDestinationId,
    SchedulingPeriod, SchedulingProblem, SchedulingResourceBound, SchedulingResourceId,
    SchedulingResourceRequirement, SchedulingUnit, SchedulingUnitId, assign_benches,
    derive_phase_design_from_nested_shells_from_map, generate_nested_shells_from_weight_map,
    generate_nested_shells_from_weight_scenarios, uniform_revenue_factors,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinelibResourceRole {
    MineTonnage,
    PlantTonnage,
    Generic,
}

#[cfg_attr(not(test), allow(dead_code))]
pub struct NestedShellPhasePlanArtifacts {
    pub shell_set: PitShellSet,
    pub phase_plan: PushbackPlan,
}

const REFERENCE_PERIOD_BENCH_AGGREGATION: &str = "reference-period-bench";
const NESTED_SHELL_BENCH_AGGREGATION: &str = "nested-shell-bench";

/// Metadatos del phase plan preferido para wiring de reportes.
#[cfg_attr(not(test), allow(dead_code))]
pub struct PreferredPhasePlanMetadata {
    pub aggregation_strategy: String,
    pub nested_shell_primary: bool,
    pub unique_shell_count: Option<usize>,
    pub descriptive_note: String,
    pub limitations: Vec<String>,
}

/// Resultado del helper de selección del phase plan preferido.
#[cfg_attr(not(test), allow(dead_code))]
pub struct PreferredPhasePlanArtifacts {
    pub phase_plan: PushbackPlan,
    pub metadata: PreferredPhasePlanMetadata,
}

pub fn build_linear_index_to_row_index(
    model: &BlockModel,
) -> Result<BTreeMap<usize, usize>, mine_sdk::MineError> {
    let mut lookup = BTreeMap::new();
    for row_index in 0..model.block_count() {
        let linear_index = model.linear_index_at(row_index)?;
        lookup.insert(linear_index, row_index);
    }
    Ok(lookup)
}

pub fn float_column<'a>(
    model: &'a BlockModel,
    column_id: &ColumnId,
) -> Result<&'a [f64], mine_sdk::MineError> {
    let Some(column_data) = model.column(column_id) else {
        return Err(mine_sdk::MineError::schema(format!(
            "column `{column_id}` does not exist in block model storage"
        )));
    };
    let ColumnData::Floats(values) = column_data else {
        return Err(mine_sdk::MineError::schema(format!(
            "column `{column_id}` must be a float column"
        )));
    };
    Ok(values)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_linear_index_float_lookup(
    model: &BlockModel,
    column_id: &ColumnId,
) -> Result<BTreeMap<usize, f64>, mine_sdk::MineError> {
    let values = float_column(model, column_id)?;
    let mut lookup = BTreeMap::new();
    for row_index in 0..model.block_count() {
        let linear_index = model.linear_index_at(row_index)?;
        let value = values.get(row_index).copied().ok_or_else(|| {
            mine_sdk::MineError::validation(format!(
                "column `{column_id}` is missing row `{row_index}`"
            ))
        })?;
        lookup.insert(linear_index, value);
    }
    Ok(lookup)
}

pub fn build_phase_plan_from_reference_periods(
    model: &BlockModel,
    linear_index_to_row_index: &BTreeMap<usize, usize>,
    reference_assignments: &[MinelibScheduleAssignment],
    tonnage_column: &ColumnId,
    limitation_note: &str,
) -> Result<PushbackPlan, mine_sdk::MineError> {
    let tonnage_values = float_column(model, tonnage_column)?;
    let bench_assignments = assign_benches(model, &BenchParameters::new(1.0, 0.0, 1.0e-9)?)?;
    let bench_by_linear_index = bench_assignments
        .iter()
        .map(|assignment| (assignment.linear_index, assignment.bench))
        .collect::<BTreeMap<_, _>>();
    let mut reference_period_by_linear_index = BTreeMap::<usize, usize>::new();
    for assignment in reference_assignments {
        if assignment.fraction <= 1.0e-9 {
            continue;
        }
        if let Some(previous_period_index) = reference_period_by_linear_index
            .insert(assignment.linear_index, assignment.period_index)
        {
            if previous_period_index != assignment.period_index {
                return Err(mine_sdk::MineError::validation(format!(
                    "reference solution assigns block `{}` to periods `{previous_period_index}` and `{}`",
                    assignment.linear_index, assignment.period_index
                )));
            }
        }
    }
    if reference_period_by_linear_index.is_empty() {
        return Err(mine_sdk::MineError::Planning {
            message: "MineLib scheduling benchmark requires at least one selected block".to_owned(),
        });
    }

    let mut phase_blocks = BTreeMap::<(usize, i64), Vec<usize>>::new();
    let mut phase_tonnage = BTreeMap::<(usize, i64), f64>::new();
    let mut benches_by_period = BTreeMap::<usize, BTreeSet<i64>>::new();

    for (linear_index, period_index) in reference_period_by_linear_index {
        let bench = *bench_by_linear_index.get(&linear_index).ok_or_else(|| {
            mine_sdk::MineError::Planning {
                message: format!("selected block `{linear_index}` is missing a bench assignment"),
            }
        })?;
        let row_index = *linear_index_to_row_index
            .get(&linear_index)
            .ok_or_else(|| {
                mine_sdk::MineError::validation(format!(
                    "linear index `{linear_index}` is not materialized in the block model"
                ))
            })?;
        phase_blocks
            .entry((period_index, bench))
            .or_default()
            .push(linear_index);
        *phase_tonnage.entry((period_index, bench)).or_insert(0.0) += tonnage_values[row_index];
        benches_by_period
            .entry(period_index)
            .or_default()
            .insert(bench);
    }

    let periods = benches_by_period.keys().copied().collect::<Vec<_>>();
    let previous_period_by_period = periods
        .iter()
        .enumerate()
        .map(|(index, period_index)| {
            let previous_period = index
                .checked_sub(1)
                .map(|previous_index| periods[previous_index]);
            (*period_index, previous_period)
        })
        .collect::<BTreeMap<_, _>>();
    let mut phase_id_by_key = BTreeMap::<(usize, i64), String>::new();
    for (period_index, benches) in &benches_by_period {
        for bench in benches.iter().rev() {
            phase_id_by_key.insert(
                (*period_index, *bench),
                format!(
                    "period-{:02}-{}",
                    period_index + 1,
                    bench_phase_label(*bench)
                ),
            );
        }
    }

    let mut phases = Vec::new();
    for (period_index, benches) in benches_by_period {
        let benches = benches.into_iter().rev().collect::<Vec<_>>();
        for (bench_position, bench) in benches.iter().enumerate() {
            let phase_key = (period_index, *bench);
            let mut block_indices = phase_blocks.remove(&phase_key).ok_or_else(|| {
                mine_sdk::MineError::Planning {
                    message: format!(
                        "reference aggregation could not find blocks for period `{period_index}` bench `{bench}`"
                    ),
                }
            })?;
            block_indices.sort_unstable();
            let phase_id = phase_id_by_key.get(&phase_key).cloned().ok_or_else(|| {
                mine_sdk::MineError::Planning {
                    message: format!(
                        "reference aggregation could not derive a phase id for period `{period_index}` bench `{bench}`"
                    ),
                }
            })?;
            let mut predecessor_phase_ids = BTreeSet::new();
            if bench_position > 0 {
                let shallower_bench = benches[bench_position - 1];
                if let Some(predecessor_phase_id) =
                    phase_id_by_key.get(&(period_index, shallower_bench))
                {
                    predecessor_phase_ids.insert(predecessor_phase_id.clone());
                }
            }
            if let Some(Some(previous_period_index)) = previous_period_by_period.get(&period_index)
            {
                if let Some(predecessor_phase_id) =
                    phase_id_by_key.get(&(*previous_period_index, *bench))
                {
                    predecessor_phase_ids.insert(predecessor_phase_id.clone());
                }
            }
            phases.push(PhaseDesign {
                phase_id,
                pushback_index: period_index,
                shell_index: None,
                revenue_factor: None,
                bench: Some(*bench),
                block_count: block_indices.len(),
                total_tonnage: phase_tonnage.get(&phase_key).copied(),
                block_indices,
                predecessor_phase_ids: predecessor_phase_ids.into_iter().collect(),
            });
        }
    }

    Ok(PushbackPlan {
        total_block_count: phases.iter().map(|phase| phase.block_count).sum(),
        total_tonnage: Some(phase_tonnage.values().sum()),
        phase_count: phases.len(),
        phases,
        nesting_rules: mine_sdk::NestingAccessRules::default_open(),
        limitations: vec![limitation_note.to_owned()],
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_phase_plan_from_nested_shells(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    upit_block_values: &BTreeMap<usize, f64>,
    tonnage_column: &ColumnId,
    revenue_factors: &[f64],
    limitation_note: &str,
) -> Result<NestedShellPhasePlanArtifacts, mine_sdk::MineError> {
    let shell_set = generate_nested_shells_from_weight_map(
        upit_block_values,
        precedence_graph,
        revenue_factors,
    )?;
    let bench_assignments = assign_benches(model, &BenchParameters::new(1.0, 0.0, 1.0e-9)?)?;
    let tonnage_by_linear_index = build_linear_index_float_lookup(model, tonnage_column)?;
    let mut phase_plan = derive_phase_design_from_nested_shells_from_map(
        &shell_set,
        &bench_assignments,
        precedence_graph,
        Some(&tonnage_by_linear_index),
        NestingAccessRules::default_open(),
    )?;
    phase_plan.limitations.push(limitation_note.to_owned());
    Ok(NestedShellPhasePlanArtifacts {
        shell_set,
        phase_plan,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_phase_plan_from_shell_weight_scenarios(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    weight_scenarios: &[(f64, BTreeMap<usize, f64>)],
    tonnage_column: &ColumnId,
    nesting_rules: NestingAccessRules,
    limitation_note: &str,
) -> Result<NestedShellPhasePlanArtifacts, mine_sdk::MineError> {
    let shell_set =
        generate_nested_shells_from_weight_scenarios(weight_scenarios, precedence_graph)?;
    let bench_assignments = assign_benches(model, &BenchParameters::new(1.0, 0.0, 1.0e-9)?)?;
    let tonnage_by_linear_index = build_linear_index_float_lookup(model, tonnage_column)?;
    let mut phase_plan = derive_phase_design_from_nested_shells_from_map(
        &shell_set,
        &bench_assignments,
        precedence_graph,
        Some(&tonnage_by_linear_index),
        nesting_rules,
    )?;
    phase_plan.limitations.push(limitation_note.to_owned());
    Ok(NestedShellPhasePlanArtifacts {
        shell_set,
        phase_plan,
    })
}

/// Selecciona la ruta preferida del phase plan para scheduling MineLib.
#[cfg_attr(not(test), allow(dead_code))]
pub fn build_preferred_phase_plan_for_minelib_scheduling(
    dataset_id: &str,
    nested_shell_primary_enabled: bool,
    model: &BlockModel,
    linear_index_to_row_index: &BTreeMap<usize, usize>,
    reference_assignments: &[MinelibScheduleAssignment],
    precedence_graph: Option<&mine_sdk::PrecedenceGraph>,
    tonnage_column: &ColumnId,
    marvin_factor_count: usize,
) -> Result<PreferredPhasePlanArtifacts, mine_sdk::MineError> {
    if dataset_id == "marvin" && nested_shell_primary_enabled {
        let precedence_graph = precedence_graph.ok_or_else(|| {
            mine_sdk::MineError::invalid_parameter(
                "precedence_graph",
                "Marvin nested-shell primary path requires a precedence graph",
            )
        })?;
        let revenue_factors = uniform_revenue_factors(marvin_factor_count)?;
        let descriptive_note = format!(
            "Marvin scheduling prefers a bounded {marvin_factor_count}-factor nested-shell × bench phase plan rebuilt from revenue/cost-aware factor scenarios with strict sequential shell access."
        );
        let shell_artifacts = build_marvin_phase_plan_from_revenue_factor_shells(
            model,
            precedence_graph,
            &revenue_factors,
            NestingAccessRules::strict_sequential(),
            &descriptive_note,
        )?;
        let limitations = shell_artifacts.phase_plan.limitations.clone();

        return Ok(PreferredPhasePlanArtifacts {
            phase_plan: shell_artifacts.phase_plan,
            metadata: PreferredPhasePlanMetadata {
                aggregation_strategy: NESTED_SHELL_BENCH_AGGREGATION.to_owned(),
                nested_shell_primary: true,
                unique_shell_count: Some(shell_artifacts.shell_set.unique_shell_count),
                descriptive_note,
                limitations,
            },
        });
    }

    let descriptive_note = if nested_shell_primary_enabled {
        format!(
            "Nested-shell primary routing is not promoted for {dataset_id}; scheduling keeps reference-period × bench aggregation until a dataset-specific revenue/cost-aware shell route exists."
        )
    } else {
        format!(
            "Reference-period × bench aggregation groups the staged CPIT memberships for {dataset_id} before routing; nested-shell is not enabled for this dataset."
        )
    };
    let phase_plan = build_phase_plan_from_reference_periods(
        model,
        linear_index_to_row_index,
        reference_assignments,
        tonnage_column,
        &descriptive_note,
    )?;
    let limitations = phase_plan.limitations.clone();

    Ok(PreferredPhasePlanArtifacts {
        phase_plan,
        metadata: PreferredPhasePlanMetadata {
            aggregation_strategy: REFERENCE_PERIOD_BENCH_AGGREGATION.to_owned(),
            nested_shell_primary: false,
            unique_shell_count: None,
            descriptive_note,
            limitations,
        },
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_marvin_revenue_factor_weight_scenarios(
    model: &BlockModel,
    revenue_factors: &[f64],
) -> Result<Vec<(f64, BTreeMap<usize, f64>)>, mine_sdk::MineError> {
    const MINE_COST_PER_TON: f64 = 0.9;
    const PROC_COST_PER_TON: f64 = 4.0;
    const AU_RECOVERY: f64 = 0.6;
    const AU_NET_PRICE: f64 = 12.0 - 0.2;
    const CU_RECOVERY: f64 = 0.88;
    const CU_NET_PRICE: f64 = 20.0 - 7.2;

    let tonnage_column = ColumnId::new("field_4")?;
    let au_column = ColumnId::new("field_5")?;
    let cu_column = ColumnId::new("field_6")?;
    let tonnage_values = float_column(model, &tonnage_column)?;
    let au_values = float_column(model, &au_column)?;
    let cu_values = float_column(model, &cu_column)?;

    revenue_factors
        .iter()
        .map(|revenue_factor| {
            let mut block_weights = BTreeMap::new();
            for row_index in 0..model.block_count() {
                let linear_index = model.linear_index_at(row_index)?;
                let tonnage = tonnage_values[row_index];
                let au = au_values[row_index];
                let cu = cu_values[row_index];
                let revenue_per_ton =
                    (au * AU_RECOVERY * AU_NET_PRICE) + (cu * CU_RECOVERY * CU_NET_PRICE);
                let block_value_per_ton = (revenue_per_ton * *revenue_factor - PROC_COST_PER_TON)
                    .max(0.0)
                    - MINE_COST_PER_TON;
                block_weights.insert(linear_index, tonnage * block_value_per_ton);
            }
            Ok((*revenue_factor, block_weights))
        })
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_marvin_phase_plan_from_revenue_factor_shells(
    model: &BlockModel,
    precedence_graph: &mine_sdk::PrecedenceGraph,
    revenue_factors: &[f64],
    nesting_rules: NestingAccessRules,
    limitation_note: &str,
) -> Result<NestedShellPhasePlanArtifacts, mine_sdk::MineError> {
    let tonnage_column = ColumnId::new("field_4")?;
    let weight_scenarios = build_marvin_revenue_factor_weight_scenarios(model, revenue_factors)?;
    build_phase_plan_from_shell_weight_scenarios(
        model,
        precedence_graph,
        &weight_scenarios,
        &tonnage_column,
        nesting_rules,
        limitation_note,
    )
}

fn bench_phase_label(bench: i64) -> String {
    if bench < 0 {
        format!("bench-neg{}", -bench)
    } else {
        format!("bench-{bench}")
    }
}

pub fn build_scheduling_problem_from_minelib_problem(
    phase_plan: &PushbackPlan,
    minelib_problem: &MinelibScheduleProblem,
    dataset_id: &str,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
    limitation_note: &str,
) -> Result<SchedulingProblem, mine_sdk::MineError> {
    let phase_by_linear_index = phase_plan
        .phases
        .iter()
        .flat_map(|phase| {
            phase
                .block_indices
                .iter()
                .copied()
                .map(move |linear_index| (linear_index, phase.phase_id.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut objective_by_phase_destination = BTreeMap::<(String, usize), f64>::new();
    let mut requirements_by_phase_resource_destination =
        BTreeMap::<(String, usize, usize), f64>::new();

    for term in &minelib_problem.objective_terms {
        let Some(phase_id) = phase_by_linear_index.get(&term.linear_index) else {
            continue;
        };
        *objective_by_phase_destination
            .entry((phase_id.clone(), term.destination_index))
            .or_insert(0.0) += term.objective_value;
    }

    for coefficient in &minelib_problem.resource_coefficients {
        if coefficient.coefficient < -1.0e-9 {
            return Err(mine_sdk::MineError::validation(format!(
                "MineLib resource coefficient for block `{}` resource `{}` destination `{}` must be non-negative to build an aggregated SchedulingProblem",
                coefficient.linear_index, coefficient.resource_index, coefficient.destination_index
            )));
        }
        if coefficient.coefficient <= 1.0e-9 {
            continue;
        }
        let Some(phase_id) = phase_by_linear_index.get(&coefficient.linear_index) else {
            continue;
        };
        *requirements_by_phase_resource_destination
            .entry((
                phase_id.clone(),
                coefficient.resource_index,
                coefficient.destination_index,
            ))
            .or_insert(0.0) += coefficient.coefficient;
    }

    let periods = build_periods_from_minelib_problem(minelib_problem, resource_roles)?;
    let destination_ids = (0..minelib_problem.destination_count)
        .map(minelib_destination_id)
        .collect::<Result<Vec<_>, _>>()?;
    let mut max_limit_by_resource = BTreeMap::<usize, f64>::new();
    for limit in &minelib_problem.resource_constraint_limits {
        if !matches!(limit.relation, 'L' | 'E') || limit.limit <= 1.0e-9 {
            continue;
        }
        max_limit_by_resource
            .entry(limit.resource_index)
            .and_modify(|current| *current = (*current).min(limit.limit))
            .or_insert(limit.limit);
    }

    let mut units = Vec::new();
    let mut objective_terms = Vec::new();
    let mut resource_requirements = Vec::new();
    let mut last_chunk_id_by_phase = BTreeMap::<String, SchedulingUnitId>::new();

    for phase in &phase_plan.phases {
        let total_tonnage = phase
            .total_tonnage
            .ok_or_else(|| mine_sdk::MineError::Planning {
                message: format!(
                    "phase `{}` requires total_tonnage to build a MineLib scheduling problem",
                    phase.phase_id
                ),
            })?;
        let candidate_destination_indices = (0..minelib_problem.destination_count)
            .filter(|destination_index| {
                objective_by_phase_destination
                    .contains_key(&(phase.phase_id.clone(), *destination_index))
                    || requirements_by_phase_resource_destination.keys().any(
                        |(phase_id, _, requirement_destination_index)| {
                            phase_id == &phase.phase_id
                                && requirement_destination_index == destination_index
                        },
                    )
            })
            .collect::<Vec<_>>();
        let candidate_destinations = candidate_destination_indices
            .iter()
            .copied()
            .map(minelib_destination_id)
            .collect::<Result<Vec<_>, _>>()?;
        let mut chunk_count = 1usize;

        for ((phase_id, resource_index, _), amount) in &requirements_by_phase_resource_destination {
            if phase_id != &phase.phase_id {
                continue;
            }
            if let Some(max_limit) = max_limit_by_resource.get(resource_index) {
                chunk_count = chunk_count.max((amount / max_limit).ceil() as usize);
            }
        }
        if let Some(max_limit) = max_limit_by_resource
            .iter()
            .filter_map(|(resource_index, max_limit)| {
                matches!(
                    resource_roles
                        .get(resource_index)
                        .copied()
                        .unwrap_or(MinelibResourceRole::Generic),
                    MinelibResourceRole::MineTonnage
                )
                .then_some(max_limit)
            })
            .min_by(|left, right| left.partial_cmp(right).expect("limits should be finite"))
        {
            chunk_count = chunk_count.max((total_tonnage / max_limit).ceil() as usize);
        }
        chunk_count = chunk_count.max(1);

        let tonnage_splits = split_f64(total_tonnage, chunk_count);
        let block_splits = split_usize(phase.block_count, chunk_count);
        let mut previous_chunk_id = None::<SchedulingUnitId>;

        for chunk_index in 0..chunk_count {
            let unit_name = if chunk_count == 1 {
                phase.phase_id.clone()
            } else {
                format!("{}::part-{:02}", phase.phase_id, chunk_index + 1)
            };
            let unit_id = SchedulingUnitId::new(unit_name)?;
            let predecessor_unit_ids = if let Some(previous_chunk_id) = &previous_chunk_id {
                vec![previous_chunk_id.clone()]
            } else {
                phase
                    .predecessor_phase_ids
                    .iter()
                    .map(|phase_id| {
                        last_chunk_id_by_phase.get(phase_id).cloned().ok_or_else(|| {
                            mine_sdk::MineError::Planning {
                                message: format!(
                                    "phase `{}` references predecessor `{phase_id}` before it was chunked",
                                    phase.phase_id
                                ),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            let unit_metadata = Metadata::from_entries(vec![(
                "phase_id".to_owned(),
                MetadataValue::Text(phase.phase_id.clone()),
            )])?;

            units.push(SchedulingUnit::new(
                unit_id.clone(),
                tonnage_splits[chunk_index],
                block_splits[chunk_index],
                predecessor_unit_ids,
                candidate_destinations.clone(),
                Vec::new(),
                Vec::new(),
                phase.bench,
                phase.shell_index,
                unit_metadata,
            )?);

            for destination_index in &candidate_destination_indices {
                let phase_objective = objective_by_phase_destination
                    .get(&(phase.phase_id.clone(), *destination_index))
                    .copied()
                    .unwrap_or(0.0);
                if phase_objective.abs() > 1.0e-9 {
                    objective_terms.push(mine_sdk::SchedulingObjectiveTerm::new(
                        unit_id.clone(),
                        Some(minelib_destination_id(*destination_index)?),
                        phase_objective / chunk_count as f64,
                    )?);
                }
            }

            for ((phase_id, resource_index, destination_index), amount) in
                &requirements_by_phase_resource_destination
            {
                if phase_id != &phase.phase_id || *amount <= 1.0e-9 {
                    continue;
                }
                resource_requirements.push(SchedulingResourceRequirement::new(
                    unit_id.clone(),
                    minelib_resource_id(*resource_index, resource_roles)?,
                    Some(minelib_destination_id(*destination_index)?),
                    amount / chunk_count as f64,
                )?);
            }

            previous_chunk_id = Some(unit_id.clone());
            last_chunk_id_by_phase.insert(phase.phase_id.clone(), unit_id);
        }
    }

    let metadata = Metadata::from_entries(vec![
        (
            "benchmark_family".to_owned(),
            MetadataValue::Text(dataset_id.to_owned()),
        ),
        (
            "source_problem_kind".to_owned(),
            MetadataValue::Text(format!("{:?}", minelib_problem.kind)),
        ),
    ])?;

    SchedulingProblem::new(
        ScenarioId::new(format!("{dataset_id}-candidate"))?,
        ModelId::new(dataset_id)?,
        periods,
        units,
        objective_terms,
        resource_requirements,
        destination_ids,
        Vec::new(),
        minelib_problem.discount_rate,
        metadata,
        vec![limitation_note.to_owned()],
    )
}

pub fn build_candidate_period_memberships(
    linear_index_to_row_index: &BTreeMap<usize, usize>,
    model: &BlockModel,
    phase_plan: &PushbackPlan,
    schedule: &mine_sdk::LongTermSchedule,
    tonnage_column: &ColumnId,
) -> Result<BTreeMap<String, BTreeSet<usize>>, mine_sdk::MineError> {
    let tonnage_values = float_column(model, tonnage_column)?;
    let mut memberships = BTreeMap::<String, BTreeSet<usize>>::new();

    for phase in &phase_plan.phases {
        let mut entries = schedule
            .entries()
            .iter()
            .filter(|entry| entry.phase_id() == Some(phase.phase_id.as_str()))
            .collect::<Vec<_>>();
        if entries.is_empty() {
            continue;
        }
        entries.sort_by_key(|entry| entry.period_label().to_owned());

        let mut entry_index = 0usize;
        let mut remaining_tonnage = entries[entry_index].tonnage();
        let mut blocks = phase.block_indices.clone();
        blocks.sort_unstable();

        for linear_index in blocks {
            while remaining_tonnage <= 1.0e-9 && entry_index + 1 < entries.len() {
                entry_index += 1;
                remaining_tonnage = entries[entry_index].tonnage();
            }

            let row_index = *linear_index_to_row_index
                .get(&linear_index)
                .ok_or_else(|| {
                    mine_sdk::MineError::validation(format!(
                        "linear index `{linear_index}` is not materialized in the block model"
                    ))
                })?;
            let block_tonnage = tonnage_values[row_index];
            memberships
                .entry(entries[entry_index].period_label().to_owned())
                .or_default()
                .insert(linear_index);
            remaining_tonnage -= block_tonnage;
        }
    }

    Ok(memberships)
}

fn build_periods_from_minelib_problem(
    minelib_problem: &MinelibScheduleProblem,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
) -> Result<Vec<SchedulingPeriod>, mine_sdk::MineError> {
    let mut bounds_by_period =
        vec![BTreeMap::<usize, (Option<f64>, Option<f64>)>::new(); minelib_problem.period_count];

    for limit in &minelib_problem.resource_constraint_limits {
        let period_bounds = bounds_by_period
            .get_mut(limit.period_index)
            .ok_or_else(|| {
                mine_sdk::MineError::validation(format!(
                    "MineLib resource limit references period `{}` outside declared range 0..{}",
                    limit.period_index, minelib_problem.period_count
                ))
            })?;
        let bound = period_bounds
            .entry(limit.resource_index)
            .or_insert((None, None));
        match limit.relation {
            'L' => {
                bound.1 = Some(
                    bound
                        .1
                        .map_or(limit.limit, |current| current.min(limit.limit)),
                );
            }
            'G' => {
                bound.0 = Some(
                    bound
                        .0
                        .map_or(limit.limit, |current| current.max(limit.limit)),
                );
            }
            'E' => {
                bound.0 = Some(
                    bound
                        .0
                        .map_or(limit.limit, |current| current.max(limit.limit)),
                );
                bound.1 = Some(
                    bound
                        .1
                        .map_or(limit.limit, |current| current.min(limit.limit)),
                );
            }
            relation => {
                return Err(mine_sdk::MineError::validation(format!(
                    "MineLib resource limit uses unsupported relation `{relation}`"
                )));
            }
        }
    }

    bounds_by_period
        .into_iter()
        .enumerate()
        .map(|(period_index, resource_bounds)| {
            SchedulingPeriod::new(
                format!("P{:02}", period_index + 1),
                resource_bounds
                    .into_iter()
                    .map(|(resource_index, (min_total, max_total))| {
                        SchedulingResourceBound::new(
                            minelib_resource_id(resource_index, resource_roles)?,
                            min_total,
                            max_total,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                Vec::new(),
                Vec::new(),
            )
        })
        .collect()
}

fn minelib_resource_id(
    resource_index: usize,
    resource_roles: &BTreeMap<usize, MinelibResourceRole>,
) -> Result<SchedulingResourceId, mine_sdk::MineError> {
    match resource_roles
        .get(&resource_index)
        .copied()
        .unwrap_or(MinelibResourceRole::Generic)
    {
        MinelibResourceRole::MineTonnage => SchedulingResourceId::new("mine_tonnage"),
        MinelibResourceRole::PlantTonnage => SchedulingResourceId::new("plant_tonnage"),
        MinelibResourceRole::Generic => {
            SchedulingResourceId::new(format!("resource-{resource_index:02}"))
        }
    }
}

fn minelib_destination_id(
    destination_index: usize,
) -> Result<ScheduleDestinationId, mine_sdk::MineError> {
    ScheduleDestinationId::new(format!("dest-{destination_index:02}"))
}

fn split_f64(total: f64, parts: usize) -> Vec<f64> {
    if parts <= 1 {
        return vec![total];
    }

    let base = total / parts as f64;
    let mut result = Vec::with_capacity(parts);
    let mut remaining = total;
    for part_index in 0..parts {
        if part_index + 1 == parts {
            result.push(remaining);
            continue;
        }
        result.push(base);
        remaining -= base;
    }
    result
}

fn split_usize(total: usize, parts: usize) -> Vec<usize> {
    if parts <= 1 {
        return vec![total];
    }

    let mut result = Vec::with_capacity(parts);
    let mut assigned = 0usize;
    for part_index in 0..parts {
        if part_index + 1 == parts {
            result.push(total.saturating_sub(assigned));
            continue;
        }
        let next_assigned =
            (((part_index + 1) as f64 / parts as f64) * total as f64).round() as usize;
        let current = next_assigned.saturating_sub(assigned);
        result.push(current);
        assigned += current;
    }
    result
}
