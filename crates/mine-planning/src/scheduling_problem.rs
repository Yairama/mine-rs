//! Contrato genérico para problemas de scheduling de largo plazo.
//!
//! Este módulo separa el problema de scheduling del solver concreto. El objetivo
//! es representar, de forma serializable y mine-agnostic:
//! - periodos y recursos con cotas explícitas;
//! - unidades programables y sus precedencias temporales;
//! - opciones de destino y stockpiles;
//! - términos de objetivo y consumos de recursos.
//!
//! # References
//! - Lambert, W. B., Brickey, A., Newman, A. M., Eurek, K. (2014).
//!   *Open-Pit Block-Sequencing Formulations: A Tutorial*.
//!   <https://doi.org/10.1287/inte.2013.0731>
//! - Espinoza, D., Goycoolea, M., Moreno, E., Newman, A. M. (2013).
//!   *MineLib: a library of open pit mining problems*.
//!   <https://doi.org/10.1007/s10479-012-1258-3>
//! - Meagher, C., Dimitrakopoulos, R., Avis, D. (2014).
//!   *Optimized Open Pit Mine Design, Pushbacks and the Gap Problem — A Review*.
//!   <https://doi.org/10.1134/S1062739114030132>

use std::collections::{BTreeMap, BTreeSet};

use mine_core::{Metadata, MineError, ModelId, ScenarioId};
use serde::{Deserialize, Serialize};

use crate::long_term_schedule::{
    LongTermSchedulePeriodCapacity, LongTermScheduleStockpile, ScheduleDestinationCapacity,
    ScheduleDestinationId, ScheduleStockpileCapacity, ScheduleStockpileId,
};
use crate::phase_design::{PhaseDesign, PushbackPlan};

fn validate_named_identifier(parameter: &'static str, value: String) -> Result<String, MineError> {
    if value.trim().is_empty() {
        return Err(MineError::invalid_parameter(
            parameter,
            "must not be empty or whitespace only",
        ));
    }
    if value.trim() != value {
        return Err(MineError::invalid_parameter(
            parameter,
            "must not contain leading or trailing whitespace",
        ));
    }
    Ok(value)
}

fn validate_optional_positive(
    parameter: &'static str,
    value: Option<f64>,
) -> Result<(), MineError> {
    if let Some(value) = value
        && (!value.is_finite() || value <= 0.0)
    {
        return Err(MineError::invalid_parameter(
            parameter,
            "must be finite and greater than zero when provided",
        ));
    }
    Ok(())
}

fn validate_optional_finite(parameter: &'static str, value: Option<f64>) -> Result<(), MineError> {
    if let Some(value) = value
        && !value.is_finite()
    {
        return Err(MineError::invalid_parameter(
            parameter,
            "must be finite when provided",
        ));
    }
    Ok(())
}

fn validate_unique_named_ids<T, I>(parameter: &'static str, values: I) -> Result<(), MineError>
where
    T: Ord,
    I: IntoIterator<Item = T>,
{
    let mut seen = BTreeSet::<T>::new();
    for value in values {
        if !seen.insert(value) {
            return Err(MineError::invalid_parameter(
                parameter,
                "must not contain duplicate entries",
            ));
        }
    }
    Ok(())
}

/// Identificador de una unidad programable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SchedulingUnitId(String);

impl SchedulingUnitId {
    /// Construye un identificador validado.
    pub fn new(value: impl Into<String>) -> Result<Self, MineError> {
        Ok(Self(validate_named_identifier("unit_id", value.into())?))
    }

    /// Valor textual del identificador.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SchedulingUnitId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Identificador de un recurso consumido o limitado por el scheduler.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SchedulingResourceId(String);

impl SchedulingResourceId {
    /// Construye un identificador validado.
    pub fn new(value: impl Into<String>) -> Result<Self, MineError> {
        Ok(Self(validate_named_identifier(
            "resource_id",
            value.into(),
        )?))
    }

    /// Valor textual del identificador.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SchedulingResourceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Recurso agregado que limita capacidad específica por destino.
pub fn destination_capacity_resource_id(
    destination_id: &ScheduleDestinationId,
) -> Result<SchedulingResourceId, MineError> {
    SchedulingResourceId::new(format!("destination_capacity::{destination_id}"))
}

/// Recurso agregado que limita reclaim/throughput de un stockpile.
pub fn stockpile_reclaim_capacity_resource_id(
    stockpile_id: &ScheduleStockpileId,
) -> Result<SchedulingResourceId, MineError> {
    SchedulingResourceId::new(format!("stockpile_reclaim_capacity::{stockpile_id}"))
}

/// Cotas inferior/superior de un recurso dentro de un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingResourceBound {
    resource_id: SchedulingResourceId,
    min_total: Option<f64>,
    max_total: Option<f64>,
}

impl SchedulingResourceBound {
    /// Construye una cota validada.
    pub fn new(
        resource_id: SchedulingResourceId,
        min_total: Option<f64>,
        max_total: Option<f64>,
    ) -> Result<Self, MineError> {
        validate_optional_positive("min_total", min_total)?;
        validate_optional_positive("max_total", max_total)?;
        if let (Some(min_total), Some(max_total)) = (min_total, max_total)
            && min_total > max_total
        {
            return Err(MineError::invalid_parameter(
                "resource_bound",
                "min_total must not exceed max_total",
            ));
        }

        Ok(Self {
            resource_id,
            min_total,
            max_total,
        })
    }

    /// Recurso al que aplica la cota.
    #[must_use]
    pub fn resource_id(&self) -> &SchedulingResourceId {
        &self.resource_id
    }

    /// Cota inferior del recurso.
    #[must_use]
    pub const fn min_total(&self) -> Option<f64> {
        self.min_total
    }

    /// Cota superior del recurso.
    #[must_use]
    pub const fn max_total(&self) -> Option<f64> {
        self.max_total
    }
}

/// Periodo programable dentro de un `SchedulingProblem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingPeriod {
    period_label: String,
    resource_bounds: Vec<SchedulingResourceBound>,
    destination_capacities: Vec<ScheduleDestinationCapacity>,
    stockpile_capacities: Vec<ScheduleStockpileCapacity>,
}

impl SchedulingPeriod {
    /// Construye un periodo validado con recursos y capacidades opcionales.
    pub fn new(
        period_label: impl Into<String>,
        resource_bounds: Vec<SchedulingResourceBound>,
        destination_capacities: Vec<ScheduleDestinationCapacity>,
        stockpile_capacities: Vec<ScheduleStockpileCapacity>,
    ) -> Result<Self, MineError> {
        let period_label = validate_named_identifier("period_label", period_label.into())?;
        validate_unique_named_ids(
            "resource_bounds",
            resource_bounds
                .iter()
                .map(SchedulingResourceBound::resource_id)
                .cloned(),
        )?;
        validate_unique_named_ids(
            "destination_capacities",
            destination_capacities
                .iter()
                .map(ScheduleDestinationCapacity::destination_id)
                .cloned(),
        )?;
        validate_unique_named_ids(
            "stockpile_capacities",
            stockpile_capacities
                .iter()
                .map(ScheduleStockpileCapacity::stockpile_id)
                .cloned(),
        )?;

        Ok(Self {
            period_label,
            resource_bounds,
            destination_capacities,
            stockpile_capacities,
        })
    }

    /// Adapta capacidades del contrato de largo plazo existente.
    pub fn from_long_term_capacity(
        capacity: &LongTermSchedulePeriodCapacity,
    ) -> Result<Self, MineError> {
        let mut resource_bounds = Vec::new();
        if capacity.max_mine_tonnage().is_some() {
            resource_bounds.push(SchedulingResourceBound::new(
                SchedulingResourceId::new("mine_tonnage")?,
                None,
                capacity.max_mine_tonnage(),
            )?);
        }
        if capacity.max_plant_tonnage().is_some() {
            resource_bounds.push(SchedulingResourceBound::new(
                SchedulingResourceId::new("plant_tonnage")?,
                None,
                capacity.max_plant_tonnage(),
            )?);
        }

        Self::new(
            capacity.period_label(),
            resource_bounds,
            capacity.destination_capacities().to_vec(),
            capacity.stockpile_capacities().to_vec(),
        )
    }

    /// Etiqueta del periodo.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Cotas de recursos del periodo.
    #[must_use]
    pub fn resource_bounds(&self) -> &[SchedulingResourceBound] {
        &self.resource_bounds
    }

    /// Capacidades por destino.
    #[must_use]
    pub fn destination_capacities(&self) -> &[ScheduleDestinationCapacity] {
        &self.destination_capacities
    }

    /// Capacidades por stockpile.
    #[must_use]
    pub fn stockpile_capacities(&self) -> &[ScheduleStockpileCapacity] {
        &self.stockpile_capacities
    }
}

/// Unidad programable con precedencias y opciones de ruteo explícitas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingUnit {
    unit_id: SchedulingUnitId,
    tonnage: f64,
    block_count: usize,
    predecessor_unit_ids: Vec<SchedulingUnitId>,
    eligible_destination_ids: Vec<ScheduleDestinationId>,
    eligible_stockpile_ids: Vec<ScheduleStockpileId>,
    #[serde(default)]
    stockpile_inventory_delta_tonnage: Option<f64>,
    block_indices: Vec<usize>,
    bench: Option<i64>,
    shell_index: Option<usize>,
    metadata: Metadata,
}

impl SchedulingUnit {
    /// Construye una unidad programable validada.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        unit_id: SchedulingUnitId,
        tonnage: f64,
        block_count: usize,
        predecessor_unit_ids: Vec<SchedulingUnitId>,
        eligible_destination_ids: Vec<ScheduleDestinationId>,
        eligible_stockpile_ids: Vec<ScheduleStockpileId>,
        block_indices: Vec<usize>,
        bench: Option<i64>,
        shell_index: Option<usize>,
        metadata: Metadata,
    ) -> Result<Self, MineError> {
        if !tonnage.is_finite() || tonnage <= 0.0 {
            return Err(MineError::invalid_parameter(
                "tonnage",
                "must be finite and greater than zero",
            ));
        }
        if block_count == 0 {
            return Err(MineError::invalid_parameter(
                "block_count",
                "must be greater than zero",
            ));
        }
        if !block_indices.is_empty() && block_indices.len() > block_count {
            return Err(MineError::invalid_parameter(
                "block_indices",
                "must not contain more indices than block_count",
            ));
        }
        validate_unique_named_ids("predecessor_unit_ids", predecessor_unit_ids.iter().cloned())?;
        validate_unique_named_ids(
            "eligible_destination_ids",
            eligible_destination_ids.iter().cloned(),
        )?;
        validate_unique_named_ids(
            "eligible_stockpile_ids",
            eligible_stockpile_ids.iter().cloned(),
        )?;
        let mut seen_block_indices = BTreeSet::new();
        for linear_index in &block_indices {
            if !seen_block_indices.insert(*linear_index) {
                return Err(MineError::invalid_parameter(
                    "block_indices",
                    "must not contain duplicate linear indices",
                ));
            }
        }

        Ok(Self {
            unit_id,
            tonnage,
            block_count,
            predecessor_unit_ids,
            eligible_destination_ids,
            eligible_stockpile_ids,
            stockpile_inventory_delta_tonnage: None,
            block_indices,
            bench,
            shell_index,
            metadata,
        })
    }

    /// Adapta una `PhaseDesign` al contrato genérico.
    pub fn from_phase_design(phase: &PhaseDesign) -> Result<Self, MineError> {
        let predecessor_unit_ids = phase
            .predecessor_phase_ids
            .iter()
            .map(SchedulingUnitId::new)
            .collect::<Result<Vec<_>, _>>()?;

        Self::new(
            SchedulingUnitId::new(phase.phase_id.clone())?,
            phase.total_tonnage.ok_or_else(|| MineError::Planning {
                message: format!(
                    "phase `{}` requires total_tonnage to derive a scheduling unit",
                    phase.phase_id
                ),
            })?,
            phase.block_count,
            predecessor_unit_ids,
            Vec::new(),
            Vec::new(),
            phase.block_indices.clone(),
            phase.bench,
            phase.shell_index,
            Metadata::new(),
        )
    }

    /// Identificador de la unidad.
    #[must_use]
    pub fn unit_id(&self) -> &SchedulingUnitId {
        &self.unit_id
    }

    /// Tonelaje total de la unidad.
    #[must_use]
    pub const fn tonnage(&self) -> f64 {
        self.tonnage
    }

    /// Conteo de bloques de la unidad.
    #[must_use]
    pub const fn block_count(&self) -> usize {
        self.block_count
    }

    /// Unidades predecesoras.
    #[must_use]
    pub fn predecessor_unit_ids(&self) -> &[SchedulingUnitId] {
        &self.predecessor_unit_ids
    }

    /// Destinos explícitamente elegibles.
    #[must_use]
    pub fn eligible_destination_ids(&self) -> &[ScheduleDestinationId] {
        &self.eligible_destination_ids
    }

    /// Stockpiles explícitamente elegibles.
    #[must_use]
    pub fn eligible_stockpile_ids(&self) -> &[ScheduleStockpileId] {
        &self.eligible_stockpile_ids
    }

    /// Delta explícito de inventario al aplicar un movimiento de stockpile.
    ///
    /// Valores positivos representan depósito hacia stockpile. Valores negativos
    /// representan reclaim desde stockpile hacia un destino final.
    #[must_use]
    pub const fn stockpile_inventory_delta_tonnage(&self) -> Option<f64> {
        self.stockpile_inventory_delta_tonnage
    }

    /// Delta efectivo de inventario para el movimiento de stockpile de la unidad.
    #[must_use]
    pub fn effective_stockpile_inventory_delta_tonnage(&self) -> f64 {
        if self.eligible_stockpile_ids.is_empty() {
            0.0
        } else {
            self.stockpile_inventory_delta_tonnage
                .unwrap_or(self.tonnage)
        }
    }

    /// Indica si la unidad representa reclaim desde un stockpile.
    #[must_use]
    pub fn is_stockpile_reclaim(&self) -> bool {
        !self.eligible_stockpile_ids.is_empty()
            && self.effective_stockpile_inventory_delta_tonnage() < 0.0
    }

    /// Tonelaje positivo de reclaim cuando la unidad representa descarga desde stockpile.
    #[must_use]
    pub fn stockpile_reclaim_tonnage(&self) -> Option<f64> {
        self.is_stockpile_reclaim()
            .then_some(-self.effective_stockpile_inventory_delta_tonnage())
    }

    /// Fija un delta explícito de inventario para un movimiento de stockpile.
    pub fn with_stockpile_inventory_delta_tonnage(
        mut self,
        stockpile_inventory_delta_tonnage: Option<f64>,
    ) -> Result<Self, MineError> {
        validate_optional_finite(
            "stockpile_inventory_delta_tonnage",
            stockpile_inventory_delta_tonnage,
        )?;
        self.stockpile_inventory_delta_tonnage = stockpile_inventory_delta_tonnage;
        Ok(self)
    }

    /// Índices lineales de bloques cuando la unidad preserva membresía.
    #[must_use]
    pub fn block_indices(&self) -> &[usize] {
        &self.block_indices
    }

    /// Banco representativo cuando aplica.
    #[must_use]
    pub const fn bench(&self) -> Option<i64> {
        self.bench
    }

    /// Índice de shell fuente cuando aplica.
    #[must_use]
    pub const fn shell_index(&self) -> Option<usize> {
        self.shell_index
    }

    /// Metadata adicional de la unidad.
    #[must_use]
    pub const fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Término de objetivo de una unidad, con destino opcional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingObjectiveTerm {
    unit_id: SchedulingUnitId,
    destination_id: Option<ScheduleDestinationId>,
    value: f64,
}

impl SchedulingObjectiveTerm {
    /// Construye un término de objetivo validado.
    pub fn new(
        unit_id: SchedulingUnitId,
        destination_id: Option<ScheduleDestinationId>,
        value: f64,
    ) -> Result<Self, MineError> {
        if !value.is_finite() {
            return Err(MineError::invalid_parameter("value", "must be finite"));
        }
        Ok(Self {
            unit_id,
            destination_id,
            value,
        })
    }

    /// Unidad a la que aplica el término.
    #[must_use]
    pub fn unit_id(&self) -> &SchedulingUnitId {
        &self.unit_id
    }

    /// Destino al que aplica el término, cuando corresponde.
    #[must_use]
    pub fn destination_id(&self) -> Option<&ScheduleDestinationId> {
        self.destination_id.as_ref()
    }

    /// Valor del término.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }
}

/// Requerimiento de recurso de una unidad, con destino opcional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingResourceRequirement {
    unit_id: SchedulingUnitId,
    resource_id: SchedulingResourceId,
    destination_id: Option<ScheduleDestinationId>,
    amount: f64,
}

impl SchedulingResourceRequirement {
    /// Construye un requerimiento validado.
    pub fn new(
        unit_id: SchedulingUnitId,
        resource_id: SchedulingResourceId,
        destination_id: Option<ScheduleDestinationId>,
        amount: f64,
    ) -> Result<Self, MineError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(MineError::invalid_parameter(
                "amount",
                "must be finite and greater than zero",
            ));
        }
        Ok(Self {
            unit_id,
            resource_id,
            destination_id,
            amount,
        })
    }

    /// Unidad a la que aplica el requerimiento.
    #[must_use]
    pub fn unit_id(&self) -> &SchedulingUnitId {
        &self.unit_id
    }

    /// Recurso consumido.
    #[must_use]
    pub fn resource_id(&self) -> &SchedulingResourceId {
        &self.resource_id
    }

    /// Destino al que aplica el requerimiento, cuando corresponde.
    #[must_use]
    pub fn destination_id(&self) -> Option<&ScheduleDestinationId> {
        self.destination_id.as_ref()
    }

    /// Magnitud del requerimiento.
    #[must_use]
    pub const fn amount(&self) -> f64 {
        self.amount
    }
}

/// Problema de scheduling reusable y serializable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingProblem {
    scenario_id: ScenarioId,
    model_id: ModelId,
    periods: Vec<SchedulingPeriod>,
    units: Vec<SchedulingUnit>,
    objective_terms: Vec<SchedulingObjectiveTerm>,
    resource_requirements: Vec<SchedulingResourceRequirement>,
    destination_ids: Vec<ScheduleDestinationId>,
    stockpiles: Vec<LongTermScheduleStockpile>,
    discount_rate: f64,
    metadata: Metadata,
    limitations: Vec<String>,
}

impl SchedulingProblem {
    /// Construye un problema de scheduling validado.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scenario_id: ScenarioId,
        model_id: ModelId,
        periods: Vec<SchedulingPeriod>,
        units: Vec<SchedulingUnit>,
        objective_terms: Vec<SchedulingObjectiveTerm>,
        resource_requirements: Vec<SchedulingResourceRequirement>,
        destination_ids: Vec<ScheduleDestinationId>,
        stockpiles: Vec<LongTermScheduleStockpile>,
        discount_rate: f64,
        metadata: Metadata,
        limitations: Vec<String>,
    ) -> Result<Self, MineError> {
        if periods.is_empty() {
            return Err(MineError::invalid_parameter(
                "periods",
                "must contain at least one period",
            ));
        }
        if units.is_empty() {
            return Err(MineError::invalid_parameter(
                "units",
                "must contain at least one unit",
            ));
        }
        if !discount_rate.is_finite() || discount_rate < 0.0 {
            return Err(MineError::invalid_parameter(
                "discount_rate",
                "must be finite and non-negative",
            ));
        }

        validate_unique_named_ids(
            "periods",
            periods
                .iter()
                .map(SchedulingPeriod::period_label)
                .map(ToOwned::to_owned),
        )?;
        validate_unique_named_ids("units", units.iter().map(SchedulingUnit::unit_id).cloned())?;
        validate_unique_named_ids("destination_ids", destination_ids.iter().cloned())?;
        validate_unique_named_ids(
            "stockpiles",
            stockpiles
                .iter()
                .map(LongTermScheduleStockpile::stockpile_id)
                .cloned(),
        )?;

        let declared_resource_ids = periods
            .iter()
            .flat_map(|period| {
                period
                    .resource_bounds()
                    .iter()
                    .map(SchedulingResourceBound::resource_id)
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let declared_destination_ids = destination_ids.iter().cloned().collect::<BTreeSet<_>>();
        let declared_stockpile_ids = stockpiles
            .iter()
            .map(LongTermScheduleStockpile::stockpile_id)
            .cloned()
            .collect::<BTreeSet<_>>();
        let declared_stockpiles_by_id = stockpiles
            .iter()
            .map(|stockpile| (stockpile.stockpile_id().clone(), stockpile))
            .collect::<BTreeMap<_, _>>();
        let declared_unit_ids = units
            .iter()
            .map(SchedulingUnit::unit_id)
            .cloned()
            .collect::<BTreeSet<_>>();

        for period in &periods {
            for destination_capacity in period.destination_capacities() {
                if !declared_destination_ids.contains(destination_capacity.destination_id()) {
                    return Err(MineError::Validation {
                        message: format!(
                            "period `{}` references undeclared destination `{}`",
                            period.period_label(),
                            destination_capacity.destination_id()
                        ),
                    });
                }
            }
            for stockpile_capacity in period.stockpile_capacities() {
                if !declared_stockpile_ids.contains(stockpile_capacity.stockpile_id()) {
                    return Err(MineError::Validation {
                        message: format!(
                            "period `{}` references undeclared stockpile `{}`",
                            period.period_label(),
                            stockpile_capacity.stockpile_id()
                        ),
                    });
                }
            }
        }

        for unit in &units {
            for predecessor_unit_id in unit.predecessor_unit_ids() {
                if predecessor_unit_id == unit.unit_id() {
                    return Err(MineError::Validation {
                        message: format!(
                            "unit `{}` must not reference itself as predecessor",
                            unit.unit_id()
                        ),
                    });
                }
                if !declared_unit_ids.contains(predecessor_unit_id) {
                    return Err(MineError::Validation {
                        message: format!(
                            "unit `{}` references unknown predecessor `{}`",
                            unit.unit_id(),
                            predecessor_unit_id
                        ),
                    });
                }
            }
            for destination_id in unit.eligible_destination_ids() {
                if !declared_destination_ids.contains(destination_id) {
                    return Err(MineError::Validation {
                        message: format!(
                            "unit `{}` references undeclared destination `{destination_id}`",
                            unit.unit_id()
                        ),
                    });
                }
            }
            for stockpile_id in unit.eligible_stockpile_ids() {
                if !declared_stockpile_ids.contains(stockpile_id) {
                    return Err(MineError::Validation {
                        message: format!(
                            "unit `{}` references undeclared stockpile `{stockpile_id}`",
                            unit.unit_id()
                        ),
                    });
                }
            }
        }
        validate_stockpile_deposit_contract(&periods, &units, &declared_stockpiles_by_id)?;

        for objective_term in &objective_terms {
            validate_term_scope(
                objective_term.unit_id(),
                objective_term.destination_id(),
                &declared_unit_ids,
                &declared_destination_ids,
                &units,
                "objective term",
            )?;
        }

        for requirement in &resource_requirements {
            if !declared_resource_ids.contains(requirement.resource_id()) {
                return Err(MineError::Validation {
                    message: format!(
                        "resource requirement for unit `{}` references undeclared resource `{}`",
                        requirement.unit_id(),
                        requirement.resource_id()
                    ),
                });
            }
            validate_term_scope(
                requirement.unit_id(),
                requirement.destination_id(),
                &declared_unit_ids,
                &declared_destination_ids,
                &units,
                "resource requirement",
            )?;
        }

        Ok(Self {
            scenario_id,
            model_id,
            periods,
            units,
            objective_terms,
            resource_requirements,
            destination_ids,
            stockpiles,
            discount_rate,
            metadata,
            limitations,
        })
    }

    /// Construye un contrato base desde `PushbackPlan` y capacidades existentes.
    ///
    /// Esta adaptación deja el problema listo para ser enriquecido con términos de
    /// objetivo y consumos de recursos sin introducir ninguna regla dataset-específica.
    pub fn from_pushback_plan(
        scenario_id: ScenarioId,
        model_id: ModelId,
        phase_plan: &PushbackPlan,
        capacities: Vec<LongTermSchedulePeriodCapacity>,
        stockpiles: Vec<LongTermScheduleStockpile>,
        discount_rate: f64,
        metadata: Metadata,
    ) -> Result<Self, MineError> {
        let periods = capacities
            .iter()
            .map(SchedulingPeriod::from_long_term_capacity)
            .collect::<Result<Vec<_>, _>>()?;
        let units = phase_plan
            .phases
            .iter()
            .map(SchedulingUnit::from_phase_design)
            .collect::<Result<Vec<_>, _>>()?;
        let destination_ids = capacities
            .iter()
            .flat_map(|capacity| capacity.destination_capacities().iter())
            .map(ScheduleDestinationCapacity::destination_id)
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        Self::new(
            scenario_id,
            model_id,
            periods,
            units,
            Vec::new(),
            Vec::new(),
            destination_ids,
            stockpiles,
            discount_rate,
            metadata,
            vec![
                "This contract is derived from PushbackPlan and period capacities; objective terms and resource coefficients remain to be added explicitly.".to_owned(),
            ],
        )
    }

    /// Identificador del escenario.
    #[must_use]
    pub const fn scenario_id(&self) -> &ScenarioId {
        &self.scenario_id
    }

    /// Identificador del modelo.
    #[must_use]
    pub const fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    /// Periodos ordenados del problema.
    #[must_use]
    pub fn periods(&self) -> &[SchedulingPeriod] {
        &self.periods
    }

    /// Unidades programables del problema.
    #[must_use]
    pub fn units(&self) -> &[SchedulingUnit] {
        &self.units
    }

    /// Términos de objetivo explícitos.
    #[must_use]
    pub fn objective_terms(&self) -> &[SchedulingObjectiveTerm] {
        &self.objective_terms
    }

    /// Consumos de recursos explícitos.
    #[must_use]
    pub fn resource_requirements(&self) -> &[SchedulingResourceRequirement] {
        &self.resource_requirements
    }

    /// Destinos declarados.
    #[must_use]
    pub fn destination_ids(&self) -> &[ScheduleDestinationId] {
        &self.destination_ids
    }

    /// Stockpiles declarados.
    #[must_use]
    pub fn stockpiles(&self) -> &[LongTermScheduleStockpile] {
        &self.stockpiles
    }

    /// Tasa de descuento explícita del problema.
    #[must_use]
    pub const fn discount_rate(&self) -> f64 {
        self.discount_rate
    }

    /// Metadata adicional del contrato.
    #[must_use]
    pub const fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Limitaciones conocidas del contrato.
    #[must_use]
    pub fn limitations(&self) -> &[String] {
        &self.limitations
    }
}

fn validate_term_scope(
    unit_id: &SchedulingUnitId,
    destination_id: Option<&ScheduleDestinationId>,
    declared_unit_ids: &BTreeSet<SchedulingUnitId>,
    declared_destination_ids: &BTreeSet<ScheduleDestinationId>,
    units: &[SchedulingUnit],
    label: &str,
) -> Result<(), MineError> {
    if !declared_unit_ids.contains(unit_id) {
        return Err(MineError::Validation {
            message: format!("{label} references undeclared unit `{unit_id}`"),
        });
    }
    let Some(destination_id) = destination_id else {
        return Ok(());
    };
    if !declared_destination_ids.contains(destination_id) {
        return Err(MineError::Validation {
            message: format!("{label} references undeclared destination `{destination_id}`"),
        });
    }

    let unit = units
        .iter()
        .find(|unit| unit.unit_id() == unit_id)
        .expect("validated unit should exist");
    if !unit.eligible_destination_ids().is_empty()
        && !unit.eligible_destination_ids().contains(destination_id)
    {
        return Err(MineError::Validation {
            message: format!(
                "{label} for unit `{unit_id}` references destination `{destination_id}` outside its eligible destination set"
            ),
        });
    }
    Ok(())
}

fn validate_stockpile_deposit_contract(
    periods: &[SchedulingPeriod],
    units: &[SchedulingUnit],
    stockpiles_by_id: &BTreeMap<ScheduleStockpileId, &LongTermScheduleStockpile>,
) -> Result<(), MineError> {
    for unit in units {
        validate_optional_finite(
            "stockpile_inventory_delta_tonnage",
            unit.stockpile_inventory_delta_tonnage(),
        )?;
        if unit.stockpile_inventory_delta_tonnage().is_some()
            && unit.eligible_stockpile_ids().is_empty()
        {
            return Err(MineError::Validation {
                message: format!(
                    "unit `{}` declares stockpile_inventory_delta_tonnage without any eligible stockpile routing",
                    unit.unit_id()
                ),
            });
        }
        if unit.effective_stockpile_inventory_delta_tonnage() < 0.0
            && unit.eligible_destination_ids().is_empty()
        {
            return Err(MineError::Validation {
                message: format!(
                    "unit `{}` declares reclaim inventory delta without any eligible destination routing",
                    unit.unit_id()
                ),
            });
        }
    }

    let referenced_stockpile_ids = units
        .iter()
        .flat_map(SchedulingUnit::eligible_stockpile_ids)
        .cloned()
        .collect::<BTreeSet<_>>();
    for stockpile_id in referenced_stockpile_ids {
        let stockpile = stockpiles_by_id
            .get(&stockpile_id)
            .copied()
            .expect("validated stockpile should exist");
        for period in periods {
            let capacity = period
                .stockpile_capacities()
                .iter()
                .find(|capacity| capacity.stockpile_id() == &stockpile_id)
                .ok_or_else(|| MineError::Validation {
                    message: format!(
                        "stockpile routing for `{stockpile_id}` requires an explicit stockpile capacity in period `{}`",
                        period.period_label()
                    ),
                })?;
            let max_inventory_tonnage =
                capacity.max_inventory_tonnage().ok_or_else(|| MineError::Validation {
                    message: format!(
                        "stockpile routing for `{stockpile_id}` requires max_inventory_tonnage in period `{}`",
                        period.period_label()
                    ),
                })?;
            if stockpile.opening_tonnage() > max_inventory_tonnage + 1.0e-9 {
                return Err(MineError::Validation {
                    message: format!(
                        "stockpile routing for `{stockpile_id}` starts at {} t, exceeding inventory capacity {max_inventory_tonnage} t in period `{}`",
                        stockpile.opening_tonnage(),
                        period.period_label()
                    ),
                });
            }
        }
    }
    Ok(())
}
