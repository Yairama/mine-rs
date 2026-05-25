//! Contratos serializables para scheduling agregado de largo plazo.

use std::collections::{BTreeMap, BTreeSet};

use mine_core::{Metadata, MineError, ModelId, ScenarioId};
use serde::{Deserialize, Serialize};

use crate::phase_design::PushbackPlan;
use crate::schedule::{Schedule, ScheduleViolationCode};

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

/// Identificador de un destino dentro del contrato de largo plazo.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScheduleDestinationId(String);

impl ScheduleDestinationId {
    /// Construye un identificador de destino validado.
    pub fn new(value: impl Into<String>) -> Result<Self, MineError> {
        Ok(Self(validate_named_identifier(
            "destination_id",
            value.into(),
        )?))
    }

    /// Valor textual del identificador.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ScheduleDestinationId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Identificador de stockpile dentro del contrato de largo plazo.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScheduleStockpileId(String);

impl ScheduleStockpileId {
    /// Construye un identificador de stockpile validado.
    pub fn new(value: impl Into<String>) -> Result<Self, MineError> {
        Ok(Self(validate_named_identifier(
            "stockpile_id",
            value.into(),
        )?))
    }

    /// Valor textual del identificador.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ScheduleStockpileId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Capacidad específica por destino dentro de un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleDestinationCapacity {
    destination_id: ScheduleDestinationId,
    max_tonnage: Option<f64>,
}

impl ScheduleDestinationCapacity {
    /// Construye una capacidad por destino validada.
    pub fn new(
        destination_id: ScheduleDestinationId,
        max_tonnage: Option<f64>,
    ) -> Result<Self, MineError> {
        validate_optional_positive("max_tonnage", max_tonnage)?;
        Ok(Self {
            destination_id,
            max_tonnage,
        })
    }

    /// Destino al que aplica la capacidad.
    #[must_use]
    pub fn destination_id(&self) -> &ScheduleDestinationId {
        &self.destination_id
    }

    /// Máximo tonelaje permitido para el destino en el periodo.
    #[must_use]
    pub const fn max_tonnage(&self) -> Option<f64> {
        self.max_tonnage
    }
}

/// Capacidad específica por stockpile dentro de un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleStockpileCapacity {
    stockpile_id: ScheduleStockpileId,
    max_inventory_tonnage: Option<f64>,
    max_reclaim_tonnage: Option<f64>,
}

impl ScheduleStockpileCapacity {
    /// Construye una capacidad de stockpile validada.
    pub fn new(
        stockpile_id: ScheduleStockpileId,
        max_inventory_tonnage: Option<f64>,
        max_reclaim_tonnage: Option<f64>,
    ) -> Result<Self, MineError> {
        validate_optional_positive("max_inventory_tonnage", max_inventory_tonnage)?;
        validate_optional_positive("max_reclaim_tonnage", max_reclaim_tonnage)?;
        Ok(Self {
            stockpile_id,
            max_inventory_tonnage,
            max_reclaim_tonnage,
        })
    }

    /// Stockpile al que aplica la capacidad.
    #[must_use]
    pub fn stockpile_id(&self) -> &ScheduleStockpileId {
        &self.stockpile_id
    }

    /// Inventario máximo permitido en el periodo.
    #[must_use]
    pub const fn max_inventory_tonnage(&self) -> Option<f64> {
        self.max_inventory_tonnage
    }

    /// Reclaim máximo permitido en el periodo.
    #[must_use]
    pub const fn max_reclaim_tonnage(&self) -> Option<f64> {
        self.max_reclaim_tonnage
    }
}

/// Capacidades agregadas aplicables a un periodo del long-term schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermSchedulePeriodCapacity {
    period_label: String,
    max_mine_tonnage: Option<f64>,
    max_plant_tonnage: Option<f64>,
    destination_capacities: Vec<ScheduleDestinationCapacity>,
    stockpile_capacities: Vec<ScheduleStockpileCapacity>,
}

impl LongTermSchedulePeriodCapacity {
    /// Construye capacidades de periodo con validaciones explícitas.
    pub fn new(
        period_label: impl Into<String>,
        max_mine_tonnage: Option<f64>,
        max_plant_tonnage: Option<f64>,
        destination_capacities: Vec<ScheduleDestinationCapacity>,
        stockpile_capacities: Vec<ScheduleStockpileCapacity>,
    ) -> Result<Self, MineError> {
        let period_label = validate_named_identifier("period_label", period_label.into())?;
        validate_optional_positive("max_mine_tonnage", max_mine_tonnage)?;
        validate_optional_positive("max_plant_tonnage", max_plant_tonnage)?;
        validate_unique_destination_capacities(&destination_capacities)?;
        validate_unique_stockpile_capacities(&stockpile_capacities)?;

        Ok(Self {
            period_label,
            max_mine_tonnage,
            max_plant_tonnage,
            destination_capacities,
            stockpile_capacities,
        })
    }

    /// Etiqueta del periodo.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Capacidad máxima de mina en el periodo.
    #[must_use]
    pub const fn max_mine_tonnage(&self) -> Option<f64> {
        self.max_mine_tonnage
    }

    /// Capacidad máxima de planta en el periodo.
    #[must_use]
    pub const fn max_plant_tonnage(&self) -> Option<f64> {
        self.max_plant_tonnage
    }

    /// Capacidades específicas por destino.
    #[must_use]
    pub fn destination_capacities(&self) -> &[ScheduleDestinationCapacity] {
        &self.destination_capacities
    }

    /// Capacidades específicas por stockpile.
    #[must_use]
    pub fn stockpile_capacities(&self) -> &[ScheduleStockpileCapacity] {
        &self.stockpile_capacities
    }
}

/// Estado base de un stockpile declarado dentro del long-term schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermScheduleStockpile {
    stockpile_id: ScheduleStockpileId,
    opening_tonnage: f64,
    metadata: Metadata,
}

impl LongTermScheduleStockpile {
    /// Construye el estado base de un stockpile.
    pub fn new(
        stockpile_id: ScheduleStockpileId,
        opening_tonnage: f64,
        metadata: Metadata,
    ) -> Result<Self, MineError> {
        if !opening_tonnage.is_finite() || opening_tonnage < 0.0 {
            return Err(MineError::invalid_parameter(
                "opening_tonnage",
                "opening stockpile tonnage must be finite and non-negative",
            ));
        }
        Ok(Self {
            stockpile_id,
            opening_tonnage,
            metadata,
        })
    }

    /// Identificador del stockpile.
    #[must_use]
    pub fn stockpile_id(&self) -> &ScheduleStockpileId {
        &self.stockpile_id
    }

    /// Tonelaje de apertura.
    #[must_use]
    pub const fn opening_tonnage(&self) -> f64 {
        self.opening_tonnage
    }

    /// Metadata del stockpile.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Evalúa flujos por destino/stockpile y balances explícitos para un `LongTermSchedule`.
pub fn evaluate_long_term_schedule_material_flows(
    schedule: &LongTermSchedule,
) -> Result<LongTermScheduleMaterialFlowReport, MineError> {
    let mut violations = schedule.violations().to_vec();
    let mut period_labels = Vec::<String>::new();
    let mut seen_periods = BTreeSet::<String>::new();
    for capacity in schedule.capacities() {
        if seen_periods.insert(capacity.period_label().to_owned()) {
            period_labels.push(capacity.period_label().to_owned());
        }
    }
    for entry in schedule.entries() {
        if seen_periods.insert(entry.period_label().to_owned()) {
            period_labels.push(entry.period_label().to_owned());
        }
    }

    let mut inventory_by_stockpile = schedule
        .stockpiles()
        .iter()
        .map(|stockpile| {
            (
                stockpile.stockpile_id().as_str().to_owned(),
                stockpile.opening_tonnage(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let declared_stockpiles = schedule
        .stockpiles()
        .iter()
        .map(|stockpile| stockpile.stockpile_id().as_str().to_owned())
        .collect::<BTreeSet<_>>();

    let mut period_flows = Vec::with_capacity(period_labels.len());
    let mut stockpile_balances = Vec::new();

    for period_label in period_labels {
        let period_entries = schedule
            .entries()
            .iter()
            .filter(|entry| entry.period_label() == period_label)
            .collect::<Vec<_>>();
        let capacity = schedule
            .capacities()
            .iter()
            .find(|capacity| capacity.period_label() == period_label);

        let mut mined_tonnage = 0.0;
        let mut destination_tonnage = BTreeMap::<String, f64>::new();
        let mut stockpile_deposits = BTreeMap::<String, f64>::new();
        let mut stockpile_reclaims = BTreeMap::<String, f64>::new();

        for entry in &period_entries {
            mined_tonnage += entry.tonnage();
            if let Some(destination_id) = entry.destination_id() {
                *destination_tonnage
                    .entry(destination_id.as_str().to_owned())
                    .or_insert(0.0) += entry.tonnage();
            }
            if let Some(stockpile_id) = entry.stockpile_id() {
                *stockpile_deposits
                    .entry(stockpile_id.as_str().to_owned())
                    .or_insert(0.0) += entry.tonnage();
            }
            if let Some(stockpile_id) = entry.reclaim_stockpile_id() {
                *stockpile_reclaims
                    .entry(stockpile_id.as_str().to_owned())
                    .or_insert(0.0) += entry.tonnage();
            }
        }

        if let Some(capacity) = capacity {
            if let Some(max_mine_tonnage) = capacity.max_mine_tonnage()
                && mined_tonnage > max_mine_tonnage + 1.0e-9
            {
                violations.push(LongTermScheduleViolation {
                    code: LongTermScheduleViolationCode::ExceedsMineCapacity,
                    period_label: period_label.clone(),
                    phase_id: None,
                    bench: None,
                    limit_value: Some(max_mine_tonnage),
                    observed_value: Some(mined_tonnage),
                    message: format!(
                        "period `{period_label}` moves {mined_tonnage} t, exceeding mine capacity {max_mine_tonnage} t"
                    ),
                });
            }

            let total_destination_tonnage = destination_tonnage.values().sum::<f64>();
            if let Some(max_plant_tonnage) = capacity.max_plant_tonnage()
                && total_destination_tonnage > max_plant_tonnage + 1.0e-9
            {
                violations.push(LongTermScheduleViolation {
                    code: LongTermScheduleViolationCode::ExceedsPlantCapacity,
                    period_label: period_label.clone(),
                    phase_id: None,
                    bench: None,
                    limit_value: Some(max_plant_tonnage),
                    observed_value: Some(total_destination_tonnage),
                    message: format!(
                        "period `{period_label}` routes {total_destination_tonnage} t to destinations, exceeding plant capacity {max_plant_tonnage} t"
                    ),
                });
            }

            for destination_capacity in capacity.destination_capacities() {
                if let Some(max_tonnage) = destination_capacity.max_tonnage() {
                    let observed_tonnage = destination_tonnage
                        .get(destination_capacity.destination_id().as_str())
                        .copied()
                        .unwrap_or(0.0);
                    if observed_tonnage > max_tonnage + 1.0e-9 {
                        violations.push(LongTermScheduleViolation {
                            code: LongTermScheduleViolationCode::ExceedsDestinationCapacity,
                            period_label: period_label.clone(),
                            phase_id: None,
                            bench: None,
                            limit_value: Some(max_tonnage),
                            observed_value: Some(observed_tonnage),
                            message: format!(
                                "period `{period_label}` routes {observed_tonnage} t to destination `{}`, exceeding capacity {max_tonnage} t",
                                destination_capacity.destination_id()
                            ),
                        });
                    }
                }
            }
        }

        let mut period_stockpiles = declared_stockpiles.clone();
        period_stockpiles.extend(stockpile_deposits.keys().cloned());
        period_stockpiles.extend(stockpile_reclaims.keys().cloned());
        for stockpile_id in period_stockpiles {
            let opening_tonnage = inventory_by_stockpile
                .get(&stockpile_id)
                .copied()
                .unwrap_or(0.0);
            let deposited_tonnage = stockpile_deposits
                .get(&stockpile_id)
                .copied()
                .unwrap_or(0.0);
            let reclaimed_tonnage = stockpile_reclaims
                .get(&stockpile_id)
                .copied()
                .unwrap_or(0.0);
            let closing_tonnage = opening_tonnage + deposited_tonnage - reclaimed_tonnage;

            if !declared_stockpiles.contains(&stockpile_id) {
                violations.push(LongTermScheduleViolation {
                    code: LongTermScheduleViolationCode::InvalidStockpileBalance,
                    period_label: period_label.clone(),
                    phase_id: None,
                    bench: None,
                    limit_value: None,
                    observed_value: Some(closing_tonnage),
                    message: format!(
                        "period `{period_label}` references undeclared stockpile `{stockpile_id}`"
                    ),
                });
            }

            if closing_tonnage < -1.0e-9 {
                violations.push(LongTermScheduleViolation {
                    code: LongTermScheduleViolationCode::InvalidStockpileBalance,
                    period_label: period_label.clone(),
                    phase_id: None,
                    bench: None,
                    limit_value: Some(0.0),
                    observed_value: Some(closing_tonnage),
                    message: format!(
                        "period `{period_label}` closes stockpile `{stockpile_id}` at {closing_tonnage} t"
                    ),
                });
            }

            if let Some(capacity) = capacity.and_then(|capacity| {
                capacity
                    .stockpile_capacities()
                    .iter()
                    .find(|candidate| candidate.stockpile_id().as_str() == stockpile_id)
            }) {
                if let Some(max_inventory_tonnage) = capacity.max_inventory_tonnage()
                    && closing_tonnage > max_inventory_tonnage + 1.0e-9
                {
                    violations.push(LongTermScheduleViolation {
                        code: LongTermScheduleViolationCode::ExceedsStockpileInventory,
                        period_label: period_label.clone(),
                        phase_id: None,
                        bench: None,
                        limit_value: Some(max_inventory_tonnage),
                        observed_value: Some(closing_tonnage),
                        message: format!(
                            "period `{period_label}` closes stockpile `{stockpile_id}` at {closing_tonnage} t, exceeding inventory capacity {max_inventory_tonnage} t"
                        ),
                    });
                }
                if let Some(max_reclaim_tonnage) = capacity.max_reclaim_tonnage()
                    && reclaimed_tonnage > max_reclaim_tonnage + 1.0e-9
                {
                    violations.push(LongTermScheduleViolation {
                        code: LongTermScheduleViolationCode::ExceedsStockpileReclaim,
                        period_label: period_label.clone(),
                        phase_id: None,
                        bench: None,
                        limit_value: Some(max_reclaim_tonnage),
                        observed_value: Some(reclaimed_tonnage),
                        message: format!(
                            "period `{period_label}` reclaims {reclaimed_tonnage} t from stockpile `{stockpile_id}`, exceeding reclaim capacity {max_reclaim_tonnage} t"
                        ),
                    });
                }
            }

            inventory_by_stockpile.insert(stockpile_id.clone(), closing_tonnage);
            stockpile_balances.push(LongTermScheduleStockpileBalance {
                period_label: period_label.clone(),
                stockpile_id,
                opening_tonnage,
                deposited_tonnage,
                reclaimed_tonnage,
                closing_tonnage,
            });
        }

        period_flows.push(LongTermSchedulePeriodFlow {
            period_label,
            mined_tonnage,
            destination_tonnage,
            stockpile_deposits,
            stockpile_reclaims,
        });
    }

    Ok(LongTermScheduleMaterialFlowReport {
        period_flows,
        stockpile_balances,
        violations,
    })
}

/// Entrada elemental del long-term schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermScheduleEntry {
    period_label: String,
    phase_id: Option<String>,
    shell_index: Option<usize>,
    bench: Option<i64>,
    tonnage: f64,
    block_count: usize,
    destination_id: Option<ScheduleDestinationId>,
    stockpile_id: Option<ScheduleStockpileId>,
    reclaim_stockpile_id: Option<ScheduleStockpileId>,
    predecessor_phase_ids: Vec<String>,
}

impl LongTermScheduleEntry {
    /// Construye una entrada explícita del long-term schedule.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        period_label: impl Into<String>,
        phase_id: Option<String>,
        shell_index: Option<usize>,
        bench: Option<i64>,
        tonnage: f64,
        block_count: usize,
        destination_id: Option<ScheduleDestinationId>,
        stockpile_id: Option<ScheduleStockpileId>,
        predecessor_phase_ids: Vec<String>,
    ) -> Result<Self, MineError> {
        Self::new_with_reclaim(
            period_label,
            phase_id,
            shell_index,
            bench,
            tonnage,
            block_count,
            destination_id,
            stockpile_id,
            None,
            predecessor_phase_ids,
        )
    }

    /// Construye una entrada explícita del long-term schedule con reclaim opcional.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_reclaim(
        period_label: impl Into<String>,
        phase_id: Option<String>,
        shell_index: Option<usize>,
        bench: Option<i64>,
        tonnage: f64,
        block_count: usize,
        destination_id: Option<ScheduleDestinationId>,
        stockpile_id: Option<ScheduleStockpileId>,
        reclaim_stockpile_id: Option<ScheduleStockpileId>,
        predecessor_phase_ids: Vec<String>,
    ) -> Result<Self, MineError> {
        let period_label = validate_named_identifier("period_label", period_label.into())?;
        if let Some(phase_id) = &phase_id {
            validate_named_identifier("phase_id", phase_id.clone())?;
        }
        if !tonnage.is_finite() || tonnage <= 0.0 {
            return Err(MineError::invalid_parameter(
                "tonnage",
                "long-term schedule tonnage must be finite and greater than zero",
            ));
        }
        if destination_id.is_some() && stockpile_id.is_some() {
            return Err(MineError::validation(
                "long-term schedule entry cannot route simultaneously to a destination and a stockpile",
            ));
        }
        if stockpile_id.is_some() && reclaim_stockpile_id.is_some() {
            return Err(MineError::validation(
                "long-term schedule entry cannot deposit to and reclaim from a stockpile simultaneously",
            ));
        }
        if reclaim_stockpile_id.is_some() && destination_id.is_none() {
            return Err(MineError::validation(
                "stockpile reclaim entries require a destination_id",
            ));
        }
        if reclaim_stockpile_id.is_some()
            && (phase_id.is_some() || shell_index.is_some() || bench.is_some())
        {
            return Err(MineError::validation(
                "stockpile reclaim entries must not declare phase_id, shell_index or bench",
            ));
        }

        let predecessor_phase_ids = predecessor_phase_ids
            .into_iter()
            .map(|phase_id| validate_named_identifier("predecessor_phase_id", phase_id))
            .collect::<Result<BTreeSet<_>, _>>()?
            .into_iter()
            .collect();

        Ok(Self {
            period_label,
            phase_id,
            shell_index,
            bench,
            tonnage,
            block_count,
            destination_id,
            stockpile_id,
            reclaim_stockpile_id,
            predecessor_phase_ids,
        })
    }

    /// Periodo de la entrada.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Fase asociada, cuando existe.
    #[must_use]
    pub fn phase_id(&self) -> Option<&str> {
        self.phase_id.as_deref()
    }

    /// Shell fuente asociado, cuando existe.
    #[must_use]
    pub const fn shell_index(&self) -> Option<usize> {
        self.shell_index
    }

    /// Bench asociado, cuando existe.
    #[must_use]
    pub const fn bench(&self) -> Option<i64> {
        self.bench
    }

    /// Tonelaje asignado.
    #[must_use]
    pub const fn tonnage(&self) -> f64 {
        self.tonnage
    }

    /// Cantidad de bloques representados.
    #[must_use]
    pub const fn block_count(&self) -> usize {
        self.block_count
    }

    /// Destino explícito, cuando la entrada está ruteada a planta o botadero.
    #[must_use]
    pub fn destination_id(&self) -> Option<&ScheduleDestinationId> {
        self.destination_id.as_ref()
    }

    /// Stockpile explícito, cuando la entrada se envía a acopio.
    #[must_use]
    pub fn stockpile_id(&self) -> Option<&ScheduleStockpileId> {
        self.stockpile_id.as_ref()
    }

    /// Stockpile fuente de un reclaim explícito, cuando la entrada representa recuperación a destino.
    #[must_use]
    pub fn reclaim_stockpile_id(&self) -> Option<&ScheduleStockpileId> {
        self.reclaim_stockpile_id.as_ref()
    }

    /// Fases predecesoras declaradas para esta entrada.
    #[must_use]
    pub fn predecessor_phase_ids(&self) -> &[String] {
        &self.predecessor_phase_ids
    }
}

/// Código estable de violación para el long-term schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LongTermScheduleViolationCode {
    /// Se excede la capacidad de mina declarada.
    ExceedsMineCapacity,
    /// Se excede la capacidad de planta declarada.
    ExceedsPlantCapacity,
    /// Se excede la capacidad declarada para un destino específico.
    ExceedsDestinationCapacity,
    /// Se viola el avance vertical permitido.
    ExceedsVerticalAdvance,
    /// Se viola una precedencia declarada entre fases.
    BreaksPhasePrecedence,
    /// Se excede el inventario máximo declarado para un stockpile.
    ExceedsStockpileInventory,
    /// Se excede el reclaim máximo declarado para un stockpile.
    ExceedsStockpileReclaim,
    /// Se genera un balance de stockpile inválido.
    InvalidStockpileBalance,
}

/// Violación estructurada dentro del long-term schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermScheduleViolation {
    /// Código estable de la violación.
    pub code: LongTermScheduleViolationCode,
    /// Periodo afectado.
    pub period_label: String,
    /// Fase afectada, cuando existe.
    pub phase_id: Option<String>,
    /// Bench afectado, cuando existe.
    pub bench: Option<i64>,
    /// Límite configurado.
    pub limit_value: Option<f64>,
    /// Valor observado.
    pub observed_value: Option<f64>,
    /// Mensaje legible para humanos.
    pub message: String,
}

/// Flujos agregados por periodo dentro de un `LongTermSchedule`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermSchedulePeriodFlow {
    /// Etiqueta del periodo.
    pub period_label: String,
    /// Tonelaje total movido en el periodo.
    pub mined_tonnage: f64,
    /// Tonelaje enviado directamente a cada destino.
    pub destination_tonnage: BTreeMap<String, f64>,
    /// Tonelaje depositado a cada stockpile.
    pub stockpile_deposits: BTreeMap<String, f64>,
    /// Tonelaje recuperado desde cada stockpile.
    pub stockpile_reclaims: BTreeMap<String, f64>,
}

/// Snapshot de balance de un stockpile al cierre de un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermScheduleStockpileBalance {
    /// Etiqueta del periodo.
    pub period_label: String,
    /// Stockpile evaluado.
    pub stockpile_id: String,
    /// Inventario al inicio del periodo.
    pub opening_tonnage: f64,
    /// Tonelaje depositado durante el periodo.
    pub deposited_tonnage: f64,
    /// Tonelaje recuperado durante el periodo.
    pub reclaimed_tonnage: f64,
    /// Inventario al cierre del periodo.
    pub closing_tonnage: f64,
}

/// Reporte serializable de flujos y balances para un `LongTermSchedule`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermScheduleMaterialFlowReport {
    /// Flujos agregados por periodo.
    pub period_flows: Vec<LongTermSchedulePeriodFlow>,
    /// Balances explícitos de stockpile por periodo.
    pub stockpile_balances: Vec<LongTermScheduleStockpileBalance>,
    /// Violaciones derivadas del routing/capacidades/balances.
    pub violations: Vec<LongTermScheduleViolation>,
}

/// Contrato serializable del scheduling agregado de largo plazo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongTermSchedule {
    scenario_id: ScenarioId,
    model_id: ModelId,
    entries: Vec<LongTermScheduleEntry>,
    capacities: Vec<LongTermSchedulePeriodCapacity>,
    stockpiles: Vec<LongTermScheduleStockpile>,
    violations: Vec<LongTermScheduleViolation>,
    metadata: Metadata,
}

impl LongTermSchedule {
    /// Construye un long-term schedule validando duplicados y contratos internos.
    pub fn new(
        scenario_id: ScenarioId,
        model_id: ModelId,
        entries: Vec<LongTermScheduleEntry>,
        capacities: Vec<LongTermSchedulePeriodCapacity>,
        stockpiles: Vec<LongTermScheduleStockpile>,
        violations: Vec<LongTermScheduleViolation>,
        metadata: Metadata,
    ) -> Result<Self, MineError> {
        if entries.is_empty() {
            return Err(MineError::invalid_parameter(
                "entries",
                "long-term schedule must contain at least one entry",
            ));
        }
        validate_unique_period_capacities(&capacities)?;
        validate_unique_stockpiles(&stockpiles)?;

        Ok(Self {
            scenario_id,
            model_id,
            entries,
            capacities,
            stockpiles,
            violations,
            metadata,
        })
    }

    /// Construye el contrato de largo plazo desde el `Schedule` agregado actual.
    pub fn from_schedule(
        scenario_id: ScenarioId,
        model_id: ModelId,
        schedule: &Schedule,
        metadata: Metadata,
    ) -> Result<Self, MineError> {
        let entries = schedule
            .entries()
            .iter()
            .map(|entry| {
                LongTermScheduleEntry::new(
                    entry.period_label(),
                    entry.phase().map(ToOwned::to_owned),
                    None,
                    Some(entry.bench()),
                    entry.tonnage(),
                    entry.block_count(),
                    None,
                    None,
                    Vec::new(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let capacities = schedule
            .period_summaries()
            .iter()
            .map(|summary| {
                LongTermSchedulePeriodCapacity::new(
                    &summary.period_label,
                    schedule.constraints().max_period_tonnage(),
                    None,
                    Vec::new(),
                    Vec::new(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let violations = schedule
            .violations()
            .iter()
            .map(|violation| LongTermScheduleViolation {
                code: match violation.code {
                    ScheduleViolationCode::ExceedsPeriodTonnage => {
                        LongTermScheduleViolationCode::ExceedsMineCapacity
                    }
                    ScheduleViolationCode::ExceedsVerticalAdvance => {
                        LongTermScheduleViolationCode::ExceedsVerticalAdvance
                    }
                },
                period_label: violation.period_label.clone(),
                phase_id: violation.phase.clone(),
                bench: violation.bench,
                limit_value: Some(violation.limit_value),
                observed_value: Some(violation.observed_value),
                message: violation.message.clone(),
            })
            .collect();

        Self::new(
            scenario_id,
            model_id,
            entries,
            capacities,
            Vec::new(),
            violations,
            metadata,
        )
    }

    /// Escenario asociado al schedule.
    #[must_use]
    pub const fn scenario_id(&self) -> &ScenarioId {
        &self.scenario_id
    }

    /// Modelo asociado al schedule.
    #[must_use]
    pub const fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    /// Entradas discretas del schedule.
    #[must_use]
    pub fn entries(&self) -> &[LongTermScheduleEntry] {
        &self.entries
    }

    /// Capacidades declaradas por periodo.
    #[must_use]
    pub fn capacities(&self) -> &[LongTermSchedulePeriodCapacity] {
        &self.capacities
    }

    /// Stockpiles declarados en el contrato.
    #[must_use]
    pub fn stockpiles(&self) -> &[LongTermScheduleStockpile] {
        &self.stockpiles
    }

    /// Violaciones estructuradas del schedule.
    #[must_use]
    pub fn violations(&self) -> &[LongTermScheduleViolation] {
        &self.violations
    }

    /// Metadata adicional del contrato.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Construye un long-term schedule determinista a partir de un `PushbackPlan`.
///
/// La asignación usa una política greedy y reproducible:
/// - cada fase comienza después del último periodo de todas sus predecesoras;
/// - el tonelaje de una fase puede repartirse entre periodos consecutivos según `max_mine_tonnage`;
/// - el conteo de bloques se distribuye proporcionalmente al tonelaje asignado.
pub fn build_aggregated_long_term_schedule(
    scenario_id: ScenarioId,
    model_id: ModelId,
    phase_plan: &PushbackPlan,
    capacities: Vec<LongTermSchedulePeriodCapacity>,
    max_vertical_advance: Option<i64>,
    metadata: Metadata,
) -> Result<LongTermSchedule, MineError> {
    if phase_plan.phases.is_empty() {
        return Err(MineError::invalid_parameter(
            "phase_plan",
            "aggregated long-term schedule requires at least one phase",
        ));
    }
    validate_unique_period_capacities(&capacities)?;
    if capacities.is_empty() {
        return Err(MineError::invalid_parameter(
            "capacities",
            "aggregated long-term schedule requires at least one period capacity",
        ));
    }

    let mut remaining_mine_capacity = capacities
        .iter()
        .map(LongTermSchedulePeriodCapacity::max_mine_tonnage)
        .collect::<Vec<_>>();
    let mut phase_last_period = std::collections::BTreeMap::<String, usize>::new();
    let mut entries = Vec::<LongTermScheduleEntry>::new();

    for phase in &phase_plan.phases {
        let total_tonnage = phase.total_tonnage.ok_or_else(|| MineError::Planning {
            message: format!(
                "phase `{}` requires total_tonnage to build an aggregated long-term schedule",
                phase.phase_id
            ),
        })?;

        let earliest_period_index = phase
            .predecessor_phase_ids
            .iter()
            .map(|phase_id| {
                phase_last_period.get(phase_id).copied().ok_or_else(|| MineError::Planning {
                    message: format!(
                        "phase `{}` references predecessor `{phase_id}` that has not been scheduled",
                        phase.phase_id
                    ),
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .map_or(0, |period_index| period_index + 1);

        let allocations = allocate_phase_tonnage(
            &phase.phase_id,
            total_tonnage,
            earliest_period_index,
            &mut remaining_mine_capacity,
        )?;
        let block_allocations =
            distribute_block_count(phase.block_count, total_tonnage, &allocations);

        let mut last_period_index = earliest_period_index;
        for ((period_index, allocated_tonnage), allocated_blocks) in
            allocations.into_iter().zip(block_allocations.into_iter())
        {
            last_period_index = period_index;
            entries.push(LongTermScheduleEntry::new(
                capacities[period_index].period_label(),
                Some(phase.phase_id.clone()),
                phase.shell_index,
                phase.bench,
                allocated_tonnage,
                allocated_blocks,
                None,
                None,
                phase.predecessor_phase_ids.clone(),
            )?);
        }

        phase_last_period.insert(phase.phase_id.clone(), last_period_index);
    }

    let mut violations = Vec::new();
    if let Some(max_vertical_advance) = max_vertical_advance {
        violations.extend(build_long_term_vertical_advance_violations(
            &entries,
            &capacities,
            max_vertical_advance,
        )?);
    }

    LongTermSchedule::new(
        scenario_id,
        model_id,
        entries,
        capacities,
        Vec::new(),
        violations,
        metadata,
    )
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

fn allocate_phase_tonnage(
    phase_id: &str,
    total_tonnage: f64,
    earliest_period_index: usize,
    remaining_mine_capacity: &mut [Option<f64>],
) -> Result<Vec<(usize, f64)>, MineError> {
    let mut remaining_tonnage = total_tonnage;
    let mut allocations = Vec::new();

    for (period_index, capacity) in remaining_mine_capacity
        .iter_mut()
        .enumerate()
        .skip(earliest_period_index)
    {
        if remaining_tonnage <= 1e-9 {
            break;
        }

        let allocatable = match capacity {
            Some(available) if *available > 0.0 => remaining_tonnage.min(*available),
            Some(_) => 0.0,
            None => remaining_tonnage,
        };
        if allocatable <= 1e-9 {
            continue;
        }

        allocations.push((period_index, allocatable));
        remaining_tonnage -= allocatable;
        if let Some(available) = capacity {
            *available -= allocatable;
        }
    }

    if remaining_tonnage > 1e-9 {
        return Err(MineError::Planning {
            message: format!(
                "phase `{phase_id}` exceeds the declared mine capacity across the remaining periods"
            ),
        });
    }

    Ok(allocations)
}

fn distribute_block_count(
    total_blocks: usize,
    total_tonnage: f64,
    allocations: &[(usize, f64)],
) -> Vec<usize> {
    let mut distributed_blocks = Vec::with_capacity(allocations.len());
    let mut assigned_blocks = 0usize;
    let mut assigned_tonnage = 0.0_f64;

    for (index, (_, tonnage)) in allocations.iter().enumerate() {
        if index + 1 == allocations.len() {
            distributed_blocks.push(total_blocks.saturating_sub(assigned_blocks));
            continue;
        }

        assigned_tonnage += tonnage;
        let cumulative_fraction = assigned_tonnage / total_tonnage;
        let cumulative_blocks = (total_blocks as f64 * cumulative_fraction).round() as usize;
        let block_slice = cumulative_blocks.saturating_sub(assigned_blocks);
        distributed_blocks.push(block_slice);
        assigned_blocks += block_slice;
    }

    distributed_blocks
}

pub(crate) fn build_long_term_vertical_advance_violations(
    entries: &[LongTermScheduleEntry],
    capacities: &[LongTermSchedulePeriodCapacity],
    max_vertical_advance: i64,
) -> Result<Vec<LongTermScheduleViolation>, MineError> {
    if max_vertical_advance <= 0 {
        return Err(MineError::invalid_parameter(
            "max_vertical_advance",
            "schedule max vertical advance must be greater than zero",
        ));
    }

    let mut previous: Option<(&str, i64)> = None;
    let mut violations = Vec::new();

    for capacity in capacities {
        let Some(current_entry) = entries
            .iter()
            .filter(|entry| entry.period_label() == capacity.period_label())
            .filter(|entry| entry.bench().is_some())
            .max_by_key(|entry| entry.bench().unwrap_or(i64::MIN))
        else {
            continue;
        };

        let current_bench = current_entry
            .bench()
            .expect("bench-filtered aggregated entry should have a bench");

        if let Some((previous_period, previous_bench)) = previous {
            let advance = (current_bench - previous_bench).abs();
            if advance > max_vertical_advance {
                violations.push(LongTermScheduleViolation {
                    code: LongTermScheduleViolationCode::ExceedsVerticalAdvance,
                    period_label: capacity.period_label().to_owned(),
                    phase_id: current_entry.phase_id().map(ToOwned::to_owned),
                    bench: Some(current_bench),
                    limit_value: Some(max_vertical_advance as f64),
                    observed_value: Some(advance as f64),
                    message: format!(
                        "vertical advance from period `{previous_period}` to `{}` reached {advance} benches, exceeding the configured limit of {max_vertical_advance}",
                        capacity.period_label()
                    ),
                });
            }
        }

        previous = Some((capacity.period_label(), current_bench));
    }

    Ok(violations)
}

fn validate_unique_destination_capacities(
    capacities: &[ScheduleDestinationCapacity],
) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();
    for capacity in capacities {
        if !seen.insert(capacity.destination_id().clone()) {
            return Err(MineError::validation(format!(
                "duplicate destination capacity for `{}` in a single period",
                capacity.destination_id()
            )));
        }
    }
    Ok(())
}

fn validate_unique_stockpile_capacities(
    capacities: &[ScheduleStockpileCapacity],
) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();
    for capacity in capacities {
        if !seen.insert(capacity.stockpile_id().clone()) {
            return Err(MineError::validation(format!(
                "duplicate stockpile capacity for `{}` in a single period",
                capacity.stockpile_id()
            )));
        }
    }
    Ok(())
}

fn validate_unique_period_capacities(
    capacities: &[LongTermSchedulePeriodCapacity],
) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();
    for capacity in capacities {
        if !seen.insert(capacity.period_label().to_owned()) {
            return Err(MineError::validation(format!(
                "duplicate long-term schedule capacity for period `{}`",
                capacity.period_label()
            )));
        }
    }
    Ok(())
}

fn validate_unique_stockpiles(stockpiles: &[LongTermScheduleStockpile]) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();
    for stockpile in stockpiles {
        if !seen.insert(stockpile.stockpile_id().clone()) {
            return Err(MineError::validation(format!(
                "duplicate long-term schedule stockpile `{}`",
                stockpile.stockpile_id()
            )));
        }
    }
    Ok(())
}
