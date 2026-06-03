use std::collections::{BTreeMap, BTreeSet};

use mine_core::MineError;
use mine_planning::{PhaseDesign, PushbackPlan};

use crate::{
    BlockGrades, DestinationAssumptionSet, DestinationId, DestinationKind,
    DestinationPurePhaseRefinement, EconomicBlockModel,
    schedule_economics::{build_grade_column_slices, build_row_index_map, summarize_block_indices},
    value_block_by_destinations,
};

/// Parcel de stockpile detectado durante el staging económico previo al scheduling.
#[derive(Debug, Clone, PartialEq)]
pub struct StockpileTargetParcel {
    /// Fase padre original antes de separar hijos directos y parcels staged.
    pub parent_phase_id: String,
    /// Identificador staged auditable para este parcel todavía no promovible.
    pub staged_parcel_id: String,
    /// Destino económico fuente detectado en el `EconomicBlockModel`.
    pub source_destination_id: DestinationId,
    /// Tipo del destino fuente.
    pub source_destination_kind: DestinationKind,
    /// Stockpile objetivo asociado al parcel staged.
    pub stockpile_id: DestinationId,
    /// Índices lineales originales incluidos en el parcel.
    pub block_indices: Vec<usize>,
    /// Tonelaje total del parcel.
    pub total_tonnage: f64,
    /// Revenue agregado del parcel.
    pub revenue: f64,
    /// Costo agregado del parcel.
    pub cost: f64,
    /// Metal pagable agregado del parcel.
    pub payable_metal: BTreeMap<String, f64>,
    /// Destino feed final candidato si el parcel pudiera promoverse luego como reclaim explícito.
    pub reclaim_destination_id: Option<DestinationId>,
    /// Tipo del destino feed final candidato.
    pub reclaim_destination_kind: Option<DestinationKind>,
    /// Delta firmado de inventario que usaría el contrato reusable en reclaim explícito.
    pub reclaim_inventory_delta_tonnage: f64,
    /// Limitaciones/diagnósticos explícitos para promover el parcel staged a reclaim/direct-feed.
    pub reclaim_promotion_limitations: Vec<String>,
}

/// Resultado de staging/revisión antes de promover un `PushbackPlan` a scheduling reusable.
#[derive(Debug, Clone, PartialEq)]
pub struct StockpileSchedulingStage {
    /// Subconjunto directo ya refinado a hijos destino-puros y listo para scheduling.
    pub direct_pushback_plan: PushbackPlan,
    /// Lineage auditable de las unidades directas promovibles hoy.
    pub direct_phase_refinements: Vec<DestinationPurePhaseRefinement>,
    /// Parcels detectados hacia stockpile que todavía no son promovibles a `SchedulingProblem`.
    pub stockpile_target_parcels: Vec<StockpileTargetParcel>,
    /// Limitaciones explícitas del staging.
    pub limitations: Vec<String>,
}

/// Separa un `PushbackPlan` entre unidades directas schedulables y parcels staged a stockpile.
pub fn stage_pushback_plan_for_stockpile_readiness(
    phase_plan: &PushbackPlan,
    economic_model: &EconomicBlockModel,
) -> Result<StockpileSchedulingStage, MineError> {
    let summary_by_linear_index = economic_model
        .block_summaries()
        .iter()
        .map(|summary| (summary.linear_index, summary))
        .collect::<BTreeMap<_, _>>();
    let row_by_linear_index = build_row_index_map(economic_model)?;
    let grade_columns = build_grade_column_slices(economic_model)?;
    let reclaim_feed_destinations = build_reclaim_feed_destinations(economic_model)?;

    let mut pending_direct_children = Vec::<PendingDirectChildPhase>::new();
    let mut direct_parent_to_child_ids = BTreeMap::<String, Vec<String>>::new();
    let mut direct_phase_ids = BTreeSet::<String>::new();
    let mut staged_parcel_ids = BTreeSet::<String>::new();
    let mut stockpile_target_parcels = Vec::<StockpileTargetParcel>::new();

    for phase in &phase_plan.phases {
        let mut destination_blocks = BTreeMap::<String, Vec<usize>>::new();
        for &linear_index in &phase.block_indices {
            let summary = summary_by_linear_index
                .get(&linear_index)
                .copied()
                .ok_or_else(|| MineError::Economics {
                    message: format!(
                        "phase `{}` references block `{linear_index}` that is missing from the economic block model",
                        phase.phase_id
                    ),
                })?;
            let destination = economic_model
                .destinations()
                .get(&summary.best_destination_id)
                .ok_or_else(|| MineError::Economics {
                    message: format!(
                        "destination `{}` is missing from the economic assumptions",
                        summary.best_destination_id.as_str()
                    ),
                })?;
            destination_blocks
                .entry(destination.id().as_str().to_owned())
                .or_default()
                .push(linear_index);
        }

        let is_split = destination_blocks.len() > 1;
        let mut direct_child_ids = Vec::new();

        for (destination_id, block_indices) in destination_blocks {
            let destination = economic_model
                .destinations()
                .get(&DestinationId::new(destination_id.clone())?)
                .ok_or_else(|| MineError::Economics {
                    message: format!(
                        "destination `{destination_id}` is missing from the economic assumptions"
                    ),
                })?;

            if destination.kind() == DestinationKind::Stockpile {
                let staged_parcel_id = format!("{}::stockpile-{destination_id}", phase.phase_id);
                ensure_unique_identifier(
                    &mut staged_parcel_ids,
                    &staged_parcel_id,
                    &phase.phase_id,
                    &destination_id,
                    "stockpile staging",
                )?;
                let phase_summary = summarize_block_indices(
                    &staged_parcel_id,
                    &block_indices,
                    economic_model,
                    &summary_by_linear_index,
                    &row_by_linear_index,
                    &grade_columns,
                )?;
                let reclaim_readiness = summarize_reclaim_readiness(
                    &block_indices,
                    economic_model,
                    &summary_by_linear_index,
                    &row_by_linear_index,
                    &grade_columns,
                    reclaim_feed_destinations.as_ref(),
                )?;
                stockpile_target_parcels.push(StockpileTargetParcel {
                    parent_phase_id: phase.phase_id.clone(),
                    staged_parcel_id,
                    source_destination_id: destination.id().clone(),
                    source_destination_kind: destination.kind(),
                    stockpile_id: destination.id().clone(),
                    block_indices,
                    total_tonnage: phase_summary.total_tonnage,
                    revenue: phase_summary.revenue,
                    cost: phase_summary.cost,
                    payable_metal: phase_summary.payable_metal,
                    reclaim_destination_id: reclaim_readiness.destination_id,
                    reclaim_destination_kind: reclaim_readiness.destination_kind,
                    reclaim_inventory_delta_tonnage: -phase_summary.total_tonnage,
                    reclaim_promotion_limitations: reclaim_readiness.limitations,
                });
                continue;
            }

            let child_phase_id = if is_split {
                format!("{}::dest-{destination_id}", phase.phase_id)
            } else {
                phase.phase_id.clone()
            };
            ensure_unique_identifier(
                &mut direct_phase_ids,
                &child_phase_id,
                &phase.phase_id,
                &destination_id,
                "destination-pure preprocessing",
            )?;

            let phase_summary = summarize_block_indices(
                &child_phase_id,
                &block_indices,
                economic_model,
                &summary_by_linear_index,
                &row_by_linear_index,
                &grade_columns,
            )?;
            direct_child_ids.push(child_phase_id.clone());
            pending_direct_children.push(PendingDirectChildPhase {
                phase: PhaseDesign {
                    phase_id: child_phase_id.clone(),
                    pushback_index: phase.pushback_index,
                    shell_index: phase.shell_index,
                    revenue_factor: if destination.kind() == DestinationKind::Waste {
                        None
                    } else {
                        phase.revenue_factor
                    },
                    bench: phase.bench,
                    block_count: block_indices.len(),
                    total_tonnage: Some(phase_summary.total_tonnage),
                    block_indices: block_indices.clone(),
                    predecessor_phase_ids: vec![],
                },
                predecessor_parent_ids: phase.predecessor_phase_ids.clone(),
                refinement: DestinationPurePhaseRefinement {
                    parent_phase_id: phase.phase_id.clone(),
                    phase_id: child_phase_id,
                    destination_id: destination.id().clone(),
                    block_indices,
                    total_tonnage: phase_summary.total_tonnage,
                    revenue: phase_summary.revenue,
                    cost: phase_summary.cost,
                    payable_metal: phase_summary.payable_metal,
                },
            });
        }

        direct_parent_to_child_ids.insert(phase.phase_id.clone(), direct_child_ids);
    }

    let mut direct_phases = Vec::with_capacity(pending_direct_children.len());
    let mut direct_phase_refinements = Vec::with_capacity(pending_direct_children.len());

    for pending_child in pending_direct_children {
        let predecessor_phase_ids = expand_predecessors(
            &pending_child.predecessor_parent_ids,
            &direct_parent_to_child_ids,
            &pending_child.phase.phase_id,
        )?;
        let mut phase = pending_child.phase;
        phase.predecessor_phase_ids = predecessor_phase_ids;
        direct_phases.push(phase);
        direct_phase_refinements.push(pending_child.refinement);
    }

    let mut limitations = phase_plan.limitations.clone();
    push_unique_limitation(
        &mut limitations,
        "destination-pure preprocessing may split mixed-destination phases into child phases before scheduling".to_owned(),
    );
    if !stockpile_target_parcels.is_empty() {
        push_unique_limitation(
            &mut limitations,
            "stockpile-target parcels are staged separately from direct schedulable units until reclaim/inventory semantics exist in SchedulingProblem".to_owned(),
        );
        push_unique_limitation(
            &mut limitations,
            "the direct pushback subset omits stockpile-target parcels and must not be promoted to SchedulingProblem while staged stockpile material remains".to_owned(),
        );
    }

    let total_tonnage = direct_phase_refinements
        .iter()
        .map(|phase| phase.total_tonnage)
        .sum::<f64>();
    let total_block_count = direct_phases.iter().map(|phase| phase.block_count).sum();

    Ok(StockpileSchedulingStage {
        direct_pushback_plan: PushbackPlan {
            phase_count: direct_phases.len(),
            phases: direct_phases,
            total_block_count,
            total_tonnage: Some(total_tonnage),
            nesting_rules: phase_plan.nesting_rules.clone(),
            limitations: limitations.clone(),
        },
        direct_phase_refinements,
        stockpile_target_parcels,
        limitations,
    })
}

struct PendingDirectChildPhase {
    phase: PhaseDesign,
    predecessor_parent_ids: Vec<String>,
    refinement: DestinationPurePhaseRefinement,
}

fn ensure_unique_identifier(
    seen_ids: &mut BTreeSet<String>,
    generated_id: &str,
    parent_phase_id: &str,
    destination_id: &str,
    context: &str,
) -> Result<(), MineError> {
    if !seen_ids.insert(generated_id.to_owned()) {
        return Err(MineError::Economics {
            message: format!(
                "{context} generated duplicated id `{generated_id}` for parent `{parent_phase_id}` and destination `{destination_id}`"
            ),
        });
    }
    Ok(())
}

fn expand_predecessors(
    predecessor_parent_ids: &[String],
    parent_to_child_ids: &BTreeMap<String, Vec<String>>,
    phase_id: &str,
) -> Result<Vec<String>, MineError> {
    let mut predecessors = Vec::new();
    let mut seen = BTreeSet::<String>::new();

    for predecessor_parent_id in predecessor_parent_ids {
        let child_ids = parent_to_child_ids
            .get(predecessor_parent_id)
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "destination-pure preprocessing cannot expand predecessor `{predecessor_parent_id}` referenced by phase `{phase_id}`"
                ),
            })?;
        for child_id in child_ids {
            if seen.insert(child_id.clone()) {
                predecessors.push(child_id.clone());
            }
        }
    }

    Ok(predecessors)
}

fn push_unique_limitation(limitations: &mut Vec<String>, limitation: String) {
    if !limitations.contains(&limitation) {
        limitations.push(limitation);
    }
}

struct ReclaimReadinessSummary {
    destination_id: Option<DestinationId>,
    destination_kind: Option<DestinationKind>,
    limitations: Vec<String>,
}

fn build_reclaim_feed_destinations(
    economic_model: &EconomicBlockModel,
) -> Result<Option<DestinationAssumptionSet>, MineError> {
    let destinations = economic_model
        .destinations()
        .destinations()
        .iter()
        .filter(|destination| {
            !matches!(
                destination.kind(),
                DestinationKind::Stockpile | DestinationKind::Waste
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if destinations.is_empty() {
        Ok(None)
    } else {
        Ok(Some(DestinationAssumptionSet::new(destinations)?))
    }
}

fn summarize_reclaim_readiness(
    block_indices: &[usize],
    economic_model: &EconomicBlockModel,
    summary_by_linear_index: &BTreeMap<usize, &crate::BlockEconomicSummary>,
    row_by_linear_index: &BTreeMap<usize, usize>,
    grade_columns: &BTreeMap<String, &[f64]>,
    reclaim_feed_destinations: Option<&DestinationAssumptionSet>,
) -> Result<ReclaimReadinessSummary, MineError> {
    let Some(reclaim_feed_destinations) = reclaim_feed_destinations else {
        return Ok(ReclaimReadinessSummary {
            destination_id: None,
            destination_kind: None,
            limitations: vec![
                "no eligible non-stockpile feed destination exists once stockpile and waste routes are excluded from the economic assumptions, so staged stockpile parcels cannot be mapped to reclaim/direct-feed semantics".to_owned(),
            ],
        });
    };

    let mut feed_destinations = BTreeSet::<DestinationId>::new();
    for &linear_index in block_indices {
        let summary = summary_by_linear_index
            .get(&linear_index)
            .copied()
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "staged stockpile parcel references block `{linear_index}` that is missing from the economic block model"
                ),
            })?;
        let row_index = row_by_linear_index
            .get(&linear_index)
            .copied()
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "block `{linear_index}` cannot be mapped back to a materialized row"
                ),
            })?;

        let grades = grade_columns
            .iter()
            .map(|(metal_key, values)| {
                values
                    .get(row_index)
                    .copied()
                    .map(|value| (metal_key.clone(), value))
                    .ok_or_else(|| MineError::Economics {
                        message: format!(
                            "grade column `{metal_key}` is missing row `{row_index}` required for block `{linear_index}`"
                        ),
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let valuation = value_block_by_destinations(
            &BlockGrades::new(summary.tonnage.max(0.0), grades)?,
            reclaim_feed_destinations,
        )?;
        feed_destinations.insert(valuation.best_destination_id);
    }

    if feed_destinations.is_empty() {
        return Ok(ReclaimReadinessSummary {
            destination_id: None,
            destination_kind: None,
            limitations: vec![
                "staged stockpile parcel has no eligible non-stockpile feed destination candidate once stockpile and waste routes are excluded".to_owned(),
            ],
        });
    }
    if feed_destinations.len() > 1 {
        return Ok(ReclaimReadinessSummary {
            destination_id: None,
            destination_kind: None,
            limitations: vec![format!(
                "staged stockpile parcel spans multiple candidate non-stockpile feed destinations ({}) once stockpile and waste routes are excluded",
                feed_destinations
                    .into_iter()
                    .map(|destination_id| destination_id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )],
        });
    }

    let destination_id = feed_destinations
        .into_iter()
        .next()
        .expect("single destination set should expose its only id");
    let destination_kind = economic_model
        .destinations()
        .get(&destination_id)
        .ok_or_else(|| MineError::Economics {
            message: format!(
                "destination `{}` is missing from the economic assumptions",
                destination_id.as_str()
            ),
        })?
        .kind();

    Ok(ReclaimReadinessSummary {
        destination_id: Some(destination_id.clone()),
        destination_kind: Some(destination_kind),
        limitations: vec![format!(
            "reclaim/direct-feed routing is tonnage-representable toward destination `{}` with signed inventory delta; mining costs remain carryover while downstream reclaim coefficients can be derived from destination downstream costs",
            destination_id.as_str()
        )],
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_blockmodel::{BlockModel, ColumnData};
    use mine_core::{
        BlockDimensions, ColumnId, ColumnLogicalType, ColumnMiningRole, ColumnSchema,
        ColumnSchemaSet, Coordinate3D, GridDefinition, GridShape, MeasurementUnit, Metadata,
    };
    use mine_planning::{NestingAccessRules, PhaseDesign, PushbackPlan};

    use crate::{
        DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
        DestinationKind, DestinationPayability, DestinationRecovery, EconomicBlockModel,
        EconomicBlockModelConfig,
    };

    use super::stage_pushback_plan_for_stockpile_readiness;

    fn small_grid(block_count: usize) -> GridDefinition {
        let origin = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");
        let dims = BlockDimensions::new(10.0, 10.0, 10.0).expect("dims should be valid");
        let shape = GridShape::new(block_count, 1, 1).expect("shape should be valid");
        GridDefinition::new(origin, dims, shape, None).expect("grid should be valid")
    }

    fn small_model(cu_grades: Vec<f64>, tonnages: Vec<f64>) -> BlockModel {
        let grid = small_grid(cu_grades.len());
        let cu = ColumnId::new("cu").expect("column id should be valid");
        let ton = ColumnId::new("ton").expect("column id should be valid");
        let unit_pct = MeasurementUnit::new("%").expect("% is valid");
        let unit_t = MeasurementUnit::new("t").expect("t is valid");

        let schema = ColumnSchemaSet::from_columns(vec![
            ColumnSchema::new(
                cu.clone(),
                ColumnLogicalType::Float,
                Some(unit_pct),
                false,
                ColumnMiningRole::Grade,
            ),
            ColumnSchema::new(
                ton.clone(),
                ColumnLogicalType::Float,
                Some(unit_t),
                false,
                ColumnMiningRole::Tonnage,
            ),
        ])
        .expect("schema should be valid");

        let mut cols = BTreeMap::new();
        cols.insert(cu, ColumnData::Floats(cu_grades));
        cols.insert(ton, ColumnData::Floats(tonnages));

        BlockModel::new(grid, schema, Metadata::new(), cols).expect("model should be valid")
    }

    fn destination_set_with_stockpile() -> DestinationAssumptionSet {
        let cu = ColumnId::new("cu").expect("column id should be valid");

        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            600.0,
            8.0,
            vec![DestinationRecovery::new(cu.clone(), 0.88).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 0.97).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 9_000.0)]),
        )
        .expect("mill should be valid");

        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            2.5,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");

        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            3.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 0.5).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 4_000.0)]),
        )
        .expect("stockpile should be valid");

        DestinationAssumptionSet::new(vec![mill, waste, stockpile]).expect("set should be valid")
    }

    fn destination_set_with_reclaim_feed_candidates() -> DestinationAssumptionSet {
        let cu = ColumnId::new("cu").expect("column id should be valid");

        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            600.0,
            8.0,
            vec![DestinationRecovery::new(cu.clone(), 0.88).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 0.97).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 9_000.0)]),
        )
        .expect("mill should be valid");

        let leach = DestinationAssumptions::new(
            DestinationId::new("leach").expect("id should be valid"),
            DestinationKind::Leach,
            350.0,
            6.0,
            vec![DestinationRecovery::new(cu.clone(), 0.55).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 9_000.0)]),
        )
        .expect("leach should be valid");

        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            100.0,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");

        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            3.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 0.5).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 4_000.0)]),
        )
        .expect("stockpile should be valid");

        DestinationAssumptionSet::new(vec![mill, leach, waste, stockpile])
            .expect("set should be valid")
    }

    fn destination_set_without_reclaim_feed_candidates() -> DestinationAssumptionSet {
        let cu = ColumnId::new("cu").expect("column id should be valid");

        let waste = DestinationAssumptions::new(
            DestinationId::new("waste").expect("id should be valid"),
            DestinationKind::Waste,
            100.0,
            0.0,
            vec![],
            vec![],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::new(),
        )
        .expect("waste should be valid");

        let stockpile = DestinationAssumptions::new(
            DestinationId::new("sp").expect("id should be valid"),
            DestinationKind::Stockpile,
            3.0,
            0.5,
            vec![DestinationRecovery::new(cu.clone(), 0.5).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 1.0).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 4_000.0)]),
        )
        .expect("stockpile should be valid");

        DestinationAssumptionSet::new(vec![waste, stockpile]).expect("set should be valid")
    }

    fn phase_plan(phases: Vec<PhaseDesign>, total_tonnage: f64) -> PushbackPlan {
        PushbackPlan {
            phase_count: phases.len(),
            total_block_count: phases.iter().map(|phase| phase.block_indices.len()).sum(),
            total_tonnage: Some(total_tonnage),
            phases,
            nesting_rules: NestingAccessRules::strict_sequential(),
            limitations: vec![],
        }
    }

    fn phase(
        phase_id: &str,
        block_indices: Vec<usize>,
        tonnage: f64,
        predecessors: Vec<String>,
    ) -> PhaseDesign {
        PhaseDesign {
            phase_id: phase_id.to_owned(),
            pushback_index: 0,
            shell_index: Some(0),
            revenue_factor: Some(1.0),
            bench: Some(100),
            block_indices: block_indices.clone(),
            block_count: block_indices.len(),
            total_tonnage: Some(tonnage),
            predecessor_phase_ids: predecessors,
        }
    }

    #[test]
    fn staging_separates_direct_units_from_stockpile_parcels() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![1.10, 0.07, 0.0], vec![1_000.0, 1_000.0, 1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: destination_set_with_stockpile(),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![
                phase("phase-a", vec![0, 1], 2_000.0, vec![]),
                phase("phase-b", vec![2], 1_000.0, vec!["phase-a".to_owned()]),
            ],
            3_000.0,
        );

        let staged = stage_pushback_plan_for_stockpile_readiness(&phase_plan, &economic_model)
            .expect("staging should succeed");

        assert_eq!(staged.direct_pushback_plan.phase_count, 2);
        assert_eq!(
            staged
                .direct_pushback_plan
                .phases
                .iter()
                .map(|phase| phase.phase_id.as_str())
                .collect::<Vec<_>>(),
            vec!["phase-a::dest-mill", "phase-b"]
        );
        assert_eq!(
            staged.direct_pushback_plan.phases[1].predecessor_phase_ids,
            vec!["phase-a::dest-mill"]
        );
        assert_eq!(staged.stockpile_target_parcels.len(), 1);
        let parcel = &staged.stockpile_target_parcels[0];
        assert_eq!(parcel.parent_phase_id, "phase-a");
        assert_eq!(parcel.staged_parcel_id, "phase-a::stockpile-sp");
        assert_eq!(parcel.stockpile_id.as_str(), "sp");
        assert_eq!(parcel.block_indices, vec![1]);
        assert_eq!(parcel.total_tonnage, 1_000.0);
        assert_eq!(
            parcel
                .reclaim_destination_id
                .as_ref()
                .expect("reclaim destination should exist")
                .as_str(),
            "mill"
        );
        assert_eq!(parcel.reclaim_inventory_delta_tonnage, -1_000.0);
        assert!(
            parcel
                .reclaim_promotion_limitations
                .iter()
                .any(|item| item.contains("mining costs remain carryover"))
        );
        assert!(
            staged
                .limitations
                .iter()
                .any(|item| item.contains("stockpile-target parcels"))
        );
    }

    #[test]
    fn staging_surfaces_single_reclaim_feed_candidate_when_unique() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: destination_set_with_reclaim_feed_candidates(),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(vec![phase("phase-sp", vec![0], 1_000.0, vec![])], 1_000.0);

        let staged = stage_pushback_plan_for_stockpile_readiness(&phase_plan, &economic_model)
            .expect("staging should succeed");

        let parcel = &staged.stockpile_target_parcels[0];
        assert_eq!(
            parcel
                .reclaim_destination_id
                .as_ref()
                .expect("reclaim destination should exist")
                .as_str(),
            "mill"
        );
        assert_eq!(parcel.reclaim_destination_kind, Some(DestinationKind::Mill));
        assert_eq!(parcel.reclaim_inventory_delta_tonnage, -1_000.0);
        assert!(
            parcel
                .reclaim_promotion_limitations
                .iter()
                .any(|item| item.contains("mining costs remain carryover"))
        );
    }

    #[test]
    fn staging_reports_no_eligible_non_stockpile_feed_destination_when_none_exists() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: destination_set_without_reclaim_feed_candidates(),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(vec![phase("phase-sp", vec![0], 1_000.0, vec![])], 1_000.0);

        let staged = stage_pushback_plan_for_stockpile_readiness(&phase_plan, &economic_model)
            .expect("staging should succeed");

        let parcel = &staged.stockpile_target_parcels[0];
        assert!(parcel.reclaim_destination_id.is_none());
        assert!(parcel.reclaim_destination_kind.is_none());
        assert!(
            parcel
                .reclaim_promotion_limitations
                .iter()
                .any(|item| item.contains("no eligible non-stockpile feed destination"))
        );
    }

    #[test]
    fn staging_blocks_reclaim_candidate_when_feed_destinations_are_mixed() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.10, 0.08], vec![1_000.0, 1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: destination_set_with_reclaim_feed_candidates(),
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(
            vec![phase("phase-sp", vec![0, 1], 2_000.0, vec![])],
            2_000.0,
        );

        let staged = stage_pushback_plan_for_stockpile_readiness(&phase_plan, &economic_model)
            .expect("staging should succeed");

        let parcel = &staged.stockpile_target_parcels[0];
        assert!(parcel.reclaim_destination_id.is_none());
        assert!(
            parcel
                .reclaim_promotion_limitations
                .iter()
                .any(|item| item.contains("multiple candidate non-stockpile feed destinations"))
        );
        assert!(
            parcel
                .reclaim_promotion_limitations
                .iter()
                .any(|item| item.contains("leach") && item.contains("mill"))
        );
    }
}
