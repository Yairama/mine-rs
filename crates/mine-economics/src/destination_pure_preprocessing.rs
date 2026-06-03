use std::collections::BTreeMap;

use mine_core::MineError;
use mine_planning::PushbackPlan;

use crate::{
    DestinationId, EconomicBlockModel,
    stockpile_scheduling_staging::stage_pushback_plan_for_stockpile_readiness,
};

/// Refined destination-pure child phase derived from an original pushback phase.
#[derive(Debug, Clone, PartialEq)]
pub struct DestinationPurePhaseRefinement {
    /// Original parent phase id before destination-pure splitting.
    pub parent_phase_id: String,
    /// Destination-pure child phase id used by scheduling.
    pub phase_id: String,
    /// Economic destination assigned to every block in the child phase.
    pub destination_id: DestinationId,
    /// Original block linear indices grouped into this child phase.
    pub block_indices: Vec<usize>,
    /// Total tonnage represented by this child phase.
    pub total_tonnage: f64,
    /// Total revenue aggregated from the economic block model.
    pub revenue: f64,
    /// Total cost aggregated from the economic block model.
    pub cost: f64,
    /// Payable metal totals aggregated from the economic block model.
    pub payable_metal: BTreeMap<String, f64>,
}

/// Pushback plan rewritten into destination-pure child phases plus the refinement trace.
#[derive(Debug, Clone, PartialEq)]
pub struct DestinationPurePushbackPlan {
    /// Refined pushback plan ready to feed deterministic scheduling.
    pub pushback_plan: PushbackPlan,
    /// Mapping from refined child phases back to their parent/economic aggregates.
    pub phase_refinements: Vec<DestinationPurePhaseRefinement>,
}

/// Split mixed-destination phases into destination-pure child phases before scheduling.
pub fn refine_pushback_plan_to_destination_pure(
    phase_plan: &PushbackPlan,
    economic_model: &EconomicBlockModel,
) -> Result<DestinationPurePushbackPlan, MineError> {
    let staged = stage_pushback_plan_for_stockpile_readiness(phase_plan, economic_model)?;
    if !staged.stockpile_target_parcels.is_empty() {
        let parcel = &staged.stockpile_target_parcels[0];
        return Err(MineError::Economics {
            message: format!(
                "destination-pure preprocessing cannot promote stockpile-target parcel `{}` from parent `{}` to stockpile `{}`; reclaim/inventory semantics remain outside this contract",
                parcel.staged_parcel_id,
                parcel.parent_phase_id,
                parcel.stockpile_id.as_str()
            ),
        });
    }

    Ok(DestinationPurePushbackPlan {
        pushback_plan: staged.direct_pushback_plan,
        phase_refinements: staged.direct_phase_refinements,
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

    use super::refine_pushback_plan_to_destination_pure;

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

    fn destination_set_with_stockpile(include_stockpile: bool) -> DestinationAssumptionSet {
        let cu = ColumnId::new("cu").expect("column id should be valid");

        let mill = DestinationAssumptions::new(
            DestinationId::new("mill").expect("id should be valid"),
            DestinationKind::Mill,
            2.0,
            8.0,
            vec![DestinationRecovery::new(cu.clone(), 0.88).expect("recovery should be valid")],
            vec![DestinationPayability::new(cu.clone(), 0.97).expect("payability should be valid")],
            DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                .expect("capacity should be valid"),
            BTreeMap::from([("cu".to_owned(), 9000.0)]),
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

        let mut destinations = vec![mill, waste];
        if include_stockpile {
            let stockpile = DestinationAssumptions::new(
                DestinationId::new("sp").expect("id should be valid"),
                DestinationKind::Stockpile,
                1.0,
                0.5,
                vec![DestinationRecovery::new(cu.clone(), 0.50).expect("recovery should be valid")],
                vec![
                    DestinationPayability::new(cu.clone(), 1.0)
                        .expect("payability should be valid"),
                ],
                DestinationCapacity::new(None, MeasurementUnit::new("t").expect("t is valid"))
                    .expect("capacity should be valid"),
                BTreeMap::from([("cu".to_owned(), 4000.0)]),
            )
            .expect("stockpile should be valid");
            destinations.push(stockpile);
        }

        DestinationAssumptionSet::new(destinations).expect("set should be valid")
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
    fn refinement_splits_mixed_phase_and_expands_predecessors() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.0, 0.9, 1.1], vec![1_000.0, 1_000.0, 1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: destination_set_with_stockpile(false),
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

        let refined = refine_pushback_plan_to_destination_pure(&phase_plan, &economic_model)
            .expect("refinement should succeed");

        assert_eq!(refined.pushback_plan.phase_count, 3);
        assert_eq!(
            refined
                .pushback_plan
                .phases
                .iter()
                .map(|phase| phase.phase_id.as_str())
                .collect::<Vec<_>>(),
            vec!["phase-a::dest-mill", "phase-a::dest-waste", "phase-b"]
        );
        assert_eq!(
            refined.pushback_plan.phases[2].predecessor_phase_ids,
            vec!["phase-a::dest-mill", "phase-a::dest-waste"]
        );
        assert_eq!(
            refined
                .phase_refinements
                .iter()
                .map(|phase| phase.total_tonnage)
                .sum::<f64>(),
            3_000.0
        );
        assert_eq!(
            refined
                .phase_refinements
                .iter()
                .find(|phase| phase.phase_id == "phase-a::dest-waste")
                .expect("waste child should exist")
                .payable_metal
                .get("cu")
                .copied()
                .unwrap_or_default(),
            0.0
        );
        assert!(
            refined.pushback_plan.phases[1].revenue_factor.is_none(),
            "waste child should not preserve revenue factor"
        );
    }

    #[test]
    fn refinement_rejects_stockpile_destination_blocks() {
        let economic_model = EconomicBlockModel::build(
            small_model(vec![0.15], vec![1_000.0]),
            EconomicBlockModelConfig {
                tonnage_column: ColumnId::new("ton").expect("valid"),
                grade_columns: vec![ColumnId::new("cu").expect("valid")],
                destinations: {
                    let cu = ColumnId::new("cu").expect("column id should be valid");
                    let stockpile = DestinationAssumptions::new(
                        DestinationId::new("sp").expect("id should be valid"),
                        DestinationKind::Stockpile,
                        0.1,
                        0.5,
                        vec![
                            DestinationRecovery::new(cu.clone(), 1.0)
                                .expect("recovery should be valid"),
                        ],
                        vec![
                            DestinationPayability::new(cu.clone(), 1.0)
                                .expect("payability should be valid"),
                        ],
                        DestinationCapacity::new(
                            None,
                            MeasurementUnit::new("t").expect("t is valid"),
                        )
                        .expect("capacity should be valid"),
                        BTreeMap::from([("cu".to_owned(), 10_000.0)]),
                    )
                    .expect("stockpile should be valid");
                    let waste = DestinationAssumptions::new(
                        DestinationId::new("waste").expect("id should be valid"),
                        DestinationKind::Waste,
                        2.5,
                        0.0,
                        vec![],
                        vec![],
                        DestinationCapacity::new(
                            None,
                            MeasurementUnit::new("t").expect("t is valid"),
                        )
                        .expect("capacity should be valid"),
                        BTreeMap::new(),
                    )
                    .expect("waste should be valid");
                    DestinationAssumptionSet::new(vec![stockpile, waste])
                        .expect("destination set should be valid")
                },
            },
        )
        .expect("economic model should build");
        let phase_plan = phase_plan(vec![phase("phase-a", vec![0], 1_000.0, vec![])], 1_000.0);

        let error = refine_pushback_plan_to_destination_pure(&phase_plan, &economic_model)
            .expect_err("stockpile route should fail");

        assert!(format!("{error}").contains("stockpile"));
    }
}
