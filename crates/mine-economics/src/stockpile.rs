//! Primitives deterministas para balances de stockpile y blending por destino.
//!
//! Este modulo no optimiza decisiones. Solo modela:
//! - inventarios de stockpile por periodo;
//! - deposits y reclaim explicitamente ordenados;
//! - degradacion opcional aplicada al opening balance;
//! - reportes minimos de mezcla por destino.

use std::collections::{BTreeMap, BTreeSet};

use mine_core::MineError;
use serde::{Deserialize, Serialize};

use crate::DestinationId;

const MATERIAL_TOLERANCE: f64 = 1e-9;

/// Material elemental trazable por tonelaje y metal contenido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MaterialParcel {
    tonnes: f64,
    contained_metals: BTreeMap<String, f64>,
}

impl MaterialParcel {
    /// Construye un paquete de material validado.
    pub fn new(tonnes: f64, contained_metals: BTreeMap<String, f64>) -> Result<Self, MineError> {
        if !tonnes.is_finite() || tonnes < 0.0 {
            return Err(MineError::invalid_parameter(
                "tonnes",
                "material tonnes must be finite and non-negative",
            ));
        }

        let mut normalized_metals = BTreeMap::new();
        for (metal, quantity) in contained_metals {
            if metal.trim().is_empty() {
                return Err(MineError::invalid_parameter(
                    "contained_metals",
                    "metal keys must not be empty",
                ));
            }

            if !quantity.is_finite() || quantity < 0.0 {
                return Err(MineError::invalid_parameter(
                    "contained_metals",
                    format!(
                        "contained metal for `{metal}` must be finite and non-negative"
                    ),
                ));
            }

            let normalized = normalize_value(quantity);
            if normalized > 0.0 {
                normalized_metals.insert(metal, normalized);
            }
        }

        if normalize_value(tonnes) == 0.0 && !normalized_metals.is_empty() {
            return Err(MineError::invalid_parameter(
                "contained_metals",
                "contained metal cannot be positive when tonnes are zero",
            ));
        }

        Ok(Self {
            tonnes: normalize_value(tonnes),
            contained_metals: normalized_metals,
        })
    }

    /// Retorna un paquete vacio.
    #[must_use]
    pub fn zero() -> Self {
        Self::default()
    }

    /// Tonelaje total del paquete.
    #[must_use]
    pub const fn tonnes(&self) -> f64 {
        self.tonnes
    }

    /// Metal contenido por clave.
    #[must_use]
    pub fn contained_metals(&self) -> &BTreeMap<String, f64> {
        &self.contained_metals
    }

    /// Retorna `true` cuando el paquete no contiene material remanente.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        normalize_value(self.tonnes) == 0.0 && self.contained_metals.is_empty()
    }

    /// Calcula las leyes medias del paquete como metal contenido / tonelaje.
    #[must_use]
    pub fn average_grades(&self) -> BTreeMap<String, f64> {
        if normalize_value(self.tonnes) == 0.0 {
            return BTreeMap::new();
        }

        self.contained_metals
            .iter()
            .map(|(metal, quantity)| (metal.clone(), normalize_value(quantity / self.tonnes)))
            .collect()
    }

    fn add(&self, other: &Self) -> Self {
        let mut contained_metals = self.contained_metals.clone();
        for (metal, quantity) in &other.contained_metals {
            *contained_metals.entry(metal.clone()).or_insert(0.0) += quantity;
        }

        Self::new(self.tonnes + other.tonnes, contained_metals)
            .expect("adding validated material parcels must remain valid")
    }

    fn scale(&self, fraction: f64) -> Result<Self, MineError> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(MineError::invalid_parameter(
                "fraction",
                "material fraction must be finite and between 0.0 and 1.0",
            ));
        }

        let contained_metals = self
            .contained_metals
            .iter()
            .map(|(metal, quantity)| (metal.clone(), quantity * fraction))
            .collect();

        Self::new(self.tonnes * fraction, contained_metals)
    }

    fn subtract(&self, other: &Self) -> Result<Self, MineError> {
        if other.tonnes > self.tonnes + MATERIAL_TOLERANCE {
            return Err(MineError::Economics {
                message: format!(
                    "cannot remove {:.6} t from a parcel with only {:.6} t available",
                    other.tonnes, self.tonnes
                ),
            });
        }

        let mut contained_metals = self.contained_metals.clone();
        for (metal, quantity) in &other.contained_metals {
            let available = contained_metals.get(metal).copied().unwrap_or(0.0);
            if quantity > &(available + MATERIAL_TOLERANCE) {
                return Err(MineError::Economics {
                    message: format!(
                        "cannot remove {:.6} units of `{metal}` from a parcel with only {:.6} available",
                        quantity, available
                    ),
                });
            }

            let remaining = normalize_value(available - quantity);
            if remaining == 0.0 {
                contained_metals.remove(metal);
            } else {
                contained_metals.insert(metal.clone(), remaining);
            }
        }

        Self::new(normalize_value(self.tonnes - other.tonnes), contained_metals)
    }

    fn take_tonnes(&self, tonnes: f64) -> Result<Self, MineError> {
        if !tonnes.is_finite() || tonnes < 0.0 {
            return Err(MineError::invalid_parameter(
                "reclaim_tonnes",
                "requested reclaim tonnes must be finite and non-negative",
            ));
        }

        if tonnes > self.tonnes + MATERIAL_TOLERANCE {
            return Err(MineError::Economics {
                message: format!(
                    "cannot reclaim {:.6} t from a balance with only {:.6} t available",
                    tonnes, self.tonnes
                ),
            });
        }

        if normalize_value(tonnes) == 0.0 {
            return Ok(Self::zero());
        }

        if normalize_value(self.tonnes) == 0.0 {
            return Err(MineError::Economics {
                message: "cannot reclaim material from an empty stockpile".to_owned(),
            });
        }

        self.scale(tonnes / self.tonnes)
    }
}

/// Identificador estable de un stockpile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StockpileId(String);

impl StockpileId {
    /// Construye un identificador validado.
    pub fn new(name: impl Into<String>) -> Result<Self, MineError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "stockpile_id",
                "stockpile id must not be empty",
            ));
        }
        Ok(Self(name))
    }

    /// Nombre estable del stockpile.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StockpileId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Perdidas opcionales aplicadas al opening balance antes de los movimientos del periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpileDegradation {
    tonnage_loss_fraction_per_period: f64,
    contained_metal_loss_fraction_per_period: f64,
}

impl StockpileDegradation {
    /// Construye una degradacion validada.
    pub fn new(
        tonnage_loss_fraction_per_period: f64,
        contained_metal_loss_fraction_per_period: f64,
    ) -> Result<Self, MineError> {
        validate_fraction(
            "tonnage_loss_fraction_per_period",
            tonnage_loss_fraction_per_period,
        )?;
        validate_fraction(
            "contained_metal_loss_fraction_per_period",
            contained_metal_loss_fraction_per_period,
        )?;

        Ok(Self {
            tonnage_loss_fraction_per_period,
            contained_metal_loss_fraction_per_period,
        })
    }

    /// Fraccion de perdida de tonelaje por periodo.
    #[must_use]
    pub const fn tonnage_loss_fraction_per_period(&self) -> f64 {
        self.tonnage_loss_fraction_per_period
    }

    /// Fraccion de perdida de metal contenido por periodo.
    #[must_use]
    pub const fn contained_metal_loss_fraction_per_period(&self) -> f64 {
        self.contained_metal_loss_fraction_per_period
    }

    fn apply(&self, material: &MaterialParcel) -> MaterialParcel {
        let contained_metals = material
            .contained_metals()
            .iter()
            .map(|(metal, quantity)| {
                (
                    metal.clone(),
                    quantity * self.contained_metal_loss_fraction_per_period,
                )
            })
            .collect();

        MaterialParcel::new(material.tonnes() * self.tonnage_loss_fraction_per_period, contained_metals)
            .expect("validated degradation must yield a valid parcel")
    }
}

/// Definicion base de un stockpile para evaluar un plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpileDefinition {
    stockpile_id: StockpileId,
    opening_balance: MaterialParcel,
    degradation: Option<StockpileDegradation>,
}

impl StockpileDefinition {
    /// Construye una definicion de stockpile.
    pub fn new(
        stockpile_id: StockpileId,
        opening_balance: MaterialParcel,
        degradation: Option<StockpileDegradation>,
    ) -> Self {
        Self {
            stockpile_id,
            opening_balance,
            degradation,
        }
    }

    /// Identificador del stockpile.
    #[must_use]
    pub fn stockpile_id(&self) -> &StockpileId {
        &self.stockpile_id
    }

    /// Balance de apertura antes del primer periodo.
    #[must_use]
    pub fn opening_balance(&self) -> &MaterialParcel {
        &self.opening_balance
    }

    /// Regla de degradacion aplicada al opening balance de cada periodo.
    #[must_use]
    pub fn degradation(&self) -> Option<&StockpileDegradation> {
        self.degradation.as_ref()
    }
}

/// Movimiento de deposito hacia un stockpile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpileDeposit {
    stockpile_id: StockpileId,
    material: MaterialParcel,
}

impl StockpileDeposit {
    /// Construye un deposito validado.
    pub fn new(stockpile_id: StockpileId, material: MaterialParcel) -> Result<Self, MineError> {
        if material.is_zero() {
            return Err(MineError::invalid_parameter(
                "material",
                "stockpile deposit material must not be empty",
            ));
        }

        Ok(Self {
            stockpile_id,
            material,
        })
    }

    /// Stockpile receptor del material.
    #[must_use]
    pub fn stockpile_id(&self) -> &StockpileId {
        &self.stockpile_id
    }

    /// Material depositado.
    #[must_use]
    pub fn material(&self) -> &MaterialParcel {
        &self.material
    }
}

/// Solicitud de reclaim desde un stockpile hacia un destino final.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpileReclaim {
    stockpile_id: StockpileId,
    destination_id: DestinationId,
    tonnes: f64,
}

impl StockpileReclaim {
    /// Construye una solicitud de reclaim validada.
    pub fn new(
        stockpile_id: StockpileId,
        destination_id: DestinationId,
        tonnes: f64,
    ) -> Result<Self, MineError> {
        if !tonnes.is_finite() || tonnes <= 0.0 {
            return Err(MineError::invalid_parameter(
                "tonnes",
                "stockpile reclaim tonnes must be finite and greater than zero",
            ));
        }

        Ok(Self {
            stockpile_id,
            destination_id,
            tonnes,
        })
    }

    /// Stockpile fuente del reclaim.
    #[must_use]
    pub fn stockpile_id(&self) -> &StockpileId {
        &self.stockpile_id
    }

    /// Destino final del material reclaimado.
    #[must_use]
    pub fn destination_id(&self) -> &DestinationId {
        &self.destination_id
    }

    /// Tonelaje solicitado desde el stockpile.
    #[must_use]
    pub const fn tonnes(&self) -> f64 {
        self.tonnes
    }
}

/// Flujo directo hacia un destino final sin pasar por stockpile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectDestinationFeed {
    destination_id: DestinationId,
    material: MaterialParcel,
}

impl DirectDestinationFeed {
    /// Construye un flujo directo validado.
    pub fn new(destination_id: DestinationId, material: MaterialParcel) -> Result<Self, MineError> {
        if material.is_zero() {
            return Err(MineError::invalid_parameter(
                "material",
                "direct destination feed material must not be empty",
            ));
        }

        Ok(Self {
            destination_id,
            material,
        })
    }

    /// Destino receptor del flujo directo.
    #[must_use]
    pub fn destination_id(&self) -> &DestinationId {
        &self.destination_id
    }

    /// Material enviado directamente al destino.
    #[must_use]
    pub fn material(&self) -> &MaterialParcel {
        &self.material
    }
}

/// Orden explicito de movimientos dentro de cada periodo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StockpileTransactionOrder {
    /// Deposits primero y luego reclaim.
    DepositThenReclaim,
    /// Reclaim primero y luego deposits.
    ReclaimThenDeposit,
}

/// Inputs de un periodo para balances y blending.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpilePeriodInput {
    period_label: String,
    deposits: Vec<StockpileDeposit>,
    reclaims: Vec<StockpileReclaim>,
    direct_destination_feeds: Vec<DirectDestinationFeed>,
}

impl StockpilePeriodInput {
    /// Construye el input de un periodo.
    pub fn new(
        period_label: impl Into<String>,
        deposits: Vec<StockpileDeposit>,
        reclaims: Vec<StockpileReclaim>,
        direct_destination_feeds: Vec<DirectDestinationFeed>,
    ) -> Result<Self, MineError> {
        let period_label = period_label.into();
        if period_label.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "period_label",
                "stockpile period label must not be empty",
            ));
        }

        Ok(Self {
            period_label,
            deposits,
            reclaims,
            direct_destination_feeds,
        })
    }

    /// Etiqueta estable del periodo.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Depositos del periodo.
    #[must_use]
    pub fn deposits(&self) -> &[StockpileDeposit] {
        &self.deposits
    }

    /// Reclaims del periodo.
    #[must_use]
    pub fn reclaims(&self) -> &[StockpileReclaim] {
        &self.reclaims
    }

    /// Flujos directos hacia destinos finales.
    #[must_use]
    pub fn direct_destination_feeds(&self) -> &[DirectDestinationFeed] {
        &self.direct_destination_feeds
    }
}

/// Input completo para evaluar un plan de stockpiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpilePlanInput {
    stockpiles: Vec<StockpileDefinition>,
    periods: Vec<StockpilePeriodInput>,
    transaction_order: StockpileTransactionOrder,
}

impl StockpilePlanInput {
    /// Construye un plan validando ids y referencias.
    pub fn new(
        stockpiles: Vec<StockpileDefinition>,
        periods: Vec<StockpilePeriodInput>,
        transaction_order: StockpileTransactionOrder,
    ) -> Result<Self, MineError> {
        let mut known_stockpiles = BTreeSet::new();
        for stockpile in &stockpiles {
            if !known_stockpiles.insert(stockpile.stockpile_id().clone()) {
                return Err(MineError::validation(format!(
                    "duplicate stockpile id `{}` in stockpile plan",
                    stockpile.stockpile_id()
                )));
            }
        }

        let mut known_periods = BTreeSet::new();
        for period in &periods {
            if !known_periods.insert(period.period_label().to_owned()) {
                return Err(MineError::validation(format!(
                    "duplicate stockpile period label `{}`",
                    period.period_label()
                )));
            }

            for deposit in period.deposits() {
                if !known_stockpiles.contains(deposit.stockpile_id()) {
                    return Err(MineError::validation(format!(
                        "period `{}` references unknown stockpile `{}` in deposit",
                        period.period_label(),
                        deposit.stockpile_id()
                    )));
                }
            }

            for reclaim in period.reclaims() {
                if !known_stockpiles.contains(reclaim.stockpile_id()) {
                    return Err(MineError::validation(format!(
                        "period `{}` references unknown stockpile `{}` in reclaim",
                        period.period_label(),
                        reclaim.stockpile_id()
                    )));
                }
            }
        }

        Ok(Self {
            stockpiles,
            periods,
            transaction_order,
        })
    }

    /// Definiciones de stockpile del plan.
    #[must_use]
    pub fn stockpiles(&self) -> &[StockpileDefinition] {
        &self.stockpiles
    }

    /// Periodos evaluados por el plan.
    #[must_use]
    pub fn periods(&self) -> &[StockpilePeriodInput] {
        &self.periods
    }

    /// Orden transaccional aplicado dentro de cada periodo.
    #[must_use]
    pub const fn transaction_order(&self) -> StockpileTransactionOrder {
        self.transaction_order
    }
}

/// Balance detallado de un stockpile en un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpileBalanceReport {
    /// Identificador del stockpile evaluado.
    pub stockpile_id: StockpileId,
    /// Balance de apertura antes de degradacion y movimientos.
    pub opening_balance: MaterialParcel,
    /// Material perdido por degradacion en el periodo.
    pub degraded_material: MaterialParcel,
    /// Material total depositado durante el periodo.
    pub deposited_material: MaterialParcel,
    /// Material total reclaimado durante el periodo.
    pub reclaimed_material: MaterialParcel,
    /// Balance de cierre tras aplicar el orden configurado.
    pub closing_balance: MaterialParcel,
}

/// Reporte minimo de mezcla entregada a un destino en un periodo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationBlendReport {
    /// Destino final evaluado.
    pub destination_id: DestinationId,
    /// Flujo directo recibido sin pasar por stockpiles.
    pub direct_feed: MaterialParcel,
    /// Flujo recibido desde reclaim de stockpiles.
    pub reclaimed_feed: MaterialParcel,
    /// Suma total enviada al destino.
    pub blended_feed: MaterialParcel,
    /// Ley media por metal del flujo combinado.
    pub blended_grades: BTreeMap<String, f64>,
    /// Stockpiles que aportaron material reclaimado al destino.
    pub contributing_stockpiles: Vec<StockpileId>,
}

/// Reporte completo de un periodo del plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpilePeriodReport {
    /// Etiqueta estable del periodo evaluado.
    pub period_label: String,
    /// Balances por stockpile dentro del periodo.
    pub stockpile_balances: Vec<StockpileBalanceReport>,
    /// Mezclas resultantes por destino final.
    pub destination_blends: Vec<DestinationBlendReport>,
}

/// Snapshot final de inventario para un stockpile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpileInventorySnapshot {
    /// Identificador del stockpile.
    pub stockpile_id: StockpileId,
    /// Balance remanente al cierre del plan.
    pub balance: MaterialParcel,
}

/// Reporte agregado del plan completo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockpilePlanReport {
    /// Orden transaccional aplicado en todos los periodos.
    pub transaction_order: StockpileTransactionOrder,
    /// Reportes detallados de cada periodo.
    pub periods: Vec<StockpilePeriodReport>,
    /// Inventarios finales por stockpile.
    pub final_balances: Vec<StockpileInventorySnapshot>,
}

/// Evalua balances y blending de un plan de stockpiles.
///
/// Secuencia por periodo:
/// 1. se parte del opening balance del stockpile;
/// 2. se aplica degradacion opcional al opening balance;
/// 3. se ejecutan deposits y reclaims segun `transaction_order`;
/// 4. se construyen reportes minimos de mezcla por destino.
pub fn evaluate_stockpile_plan(plan: &StockpilePlanInput) -> Result<StockpilePlanReport, MineError> {
    let mut inventories: BTreeMap<StockpileId, MaterialParcel> = plan
        .stockpiles()
        .iter()
        .map(|stockpile| {
            (
                stockpile.stockpile_id().clone(),
                stockpile.opening_balance().clone(),
            )
        })
        .collect();

    let mut period_reports = Vec::with_capacity(plan.periods().len());

    for period in plan.periods() {
        let opening_balances = inventories.clone();
        let mut working_balances = inventories.clone();
        let mut degraded_by_stockpile = BTreeMap::new();
        let mut deposited_by_stockpile = BTreeMap::new();
        let mut reclaimed_by_stockpile = BTreeMap::new();
        let mut reclaimed_by_destination = BTreeMap::new();
        let mut contributing_stockpiles = BTreeMap::<DestinationId, BTreeSet<StockpileId>>::new();

        for stockpile in plan.stockpiles() {
            let degraded = stockpile
                .degradation()
                .map_or_else(MaterialParcel::zero, |rule| rule.apply(
                    working_balances
                        .get(stockpile.stockpile_id())
                        .expect("stockpile inventory must exist"),
                ));

            let current_balance = working_balances
                .get(stockpile.stockpile_id())
                .expect("stockpile inventory must exist")
                .clone();
            let degraded_balance = current_balance.subtract(&degraded)?;
            working_balances.insert(stockpile.stockpile_id().clone(), degraded_balance);
            degraded_by_stockpile.insert(stockpile.stockpile_id().clone(), degraded);
        }

        let deposits_by_stockpile = aggregate_deposits(period.deposits());
        let reclaims_by_stockpile = aggregate_reclaims(period.reclaims());

        match plan.transaction_order() {
            StockpileTransactionOrder::DepositThenReclaim => {
                apply_deposits(
                    &mut working_balances,
                    &deposits_by_stockpile,
                    &mut deposited_by_stockpile,
                );
                apply_reclaims(
                    &mut working_balances,
                    &reclaims_by_stockpile,
                    &mut reclaimed_by_stockpile,
                    &mut reclaimed_by_destination,
                    &mut contributing_stockpiles,
                )?;
            }
            StockpileTransactionOrder::ReclaimThenDeposit => {
                apply_reclaims(
                    &mut working_balances,
                    &reclaims_by_stockpile,
                    &mut reclaimed_by_stockpile,
                    &mut reclaimed_by_destination,
                    &mut contributing_stockpiles,
                )?;
                apply_deposits(
                    &mut working_balances,
                    &deposits_by_stockpile,
                    &mut deposited_by_stockpile,
                );
            }
        }

        let direct_by_destination = aggregate_direct_feeds(period.direct_destination_feeds());
        let destination_blends = build_destination_blends(
            direct_by_destination,
            reclaimed_by_destination,
            contributing_stockpiles,
        );

        let stockpile_balances = plan
            .stockpiles()
            .iter()
            .map(|stockpile| {
                let stockpile_id = stockpile.stockpile_id().clone();
                StockpileBalanceReport {
                    stockpile_id: stockpile_id.clone(),
                    opening_balance: opening_balances
                        .get(&stockpile_id)
                        .cloned()
                        .unwrap_or_else(MaterialParcel::zero),
                    degraded_material: degraded_by_stockpile
                        .get(&stockpile_id)
                        .cloned()
                        .unwrap_or_else(MaterialParcel::zero),
                    deposited_material: deposited_by_stockpile
                        .get(&stockpile_id)
                        .cloned()
                        .unwrap_or_else(MaterialParcel::zero),
                    reclaimed_material: reclaimed_by_stockpile
                        .get(&stockpile_id)
                        .cloned()
                        .unwrap_or_else(MaterialParcel::zero),
                    closing_balance: working_balances
                        .get(&stockpile_id)
                        .cloned()
                        .unwrap_or_else(MaterialParcel::zero),
                }
            })
            .collect();

        inventories = working_balances;
        period_reports.push(StockpilePeriodReport {
            period_label: period.period_label().to_owned(),
            stockpile_balances,
            destination_blends,
        });
    }

    let final_balances = plan
        .stockpiles()
        .iter()
        .map(|stockpile| StockpileInventorySnapshot {
            stockpile_id: stockpile.stockpile_id().clone(),
            balance: inventories
                .get(stockpile.stockpile_id())
                .cloned()
                .unwrap_or_else(MaterialParcel::zero),
        })
        .collect();

    Ok(StockpilePlanReport {
        transaction_order: plan.transaction_order(),
        periods: period_reports,
        final_balances,
    })
}

fn aggregate_deposits(deposits: &[StockpileDeposit]) -> BTreeMap<StockpileId, MaterialParcel> {
    let mut aggregated = BTreeMap::new();
    for deposit in deposits {
        add_material(
            &mut aggregated,
            deposit.stockpile_id().clone(),
            deposit.material(),
        );
    }
    aggregated
}

fn aggregate_reclaims(
    reclaims: &[StockpileReclaim],
) -> BTreeMap<StockpileId, Vec<StockpileReclaim>> {
    let mut aggregated = BTreeMap::<StockpileId, Vec<StockpileReclaim>>::new();
    for reclaim in reclaims {
        aggregated
            .entry(reclaim.stockpile_id().clone())
            .or_default()
            .push(reclaim.clone());
    }
    aggregated
}

fn aggregate_direct_feeds(
    direct_feeds: &[DirectDestinationFeed],
) -> BTreeMap<DestinationId, MaterialParcel> {
    let mut aggregated = BTreeMap::new();
    for feed in direct_feeds {
        add_material(
            &mut aggregated,
            feed.destination_id().clone(),
            feed.material(),
        );
    }
    aggregated
}

fn apply_deposits(
    working_balances: &mut BTreeMap<StockpileId, MaterialParcel>,
    deposits_by_stockpile: &BTreeMap<StockpileId, MaterialParcel>,
    deposited_by_stockpile: &mut BTreeMap<StockpileId, MaterialParcel>,
) {
    for (stockpile_id, deposit) in deposits_by_stockpile {
        let current = working_balances
            .get(stockpile_id)
            .cloned()
            .unwrap_or_else(MaterialParcel::zero);
        working_balances.insert(stockpile_id.clone(), current.add(deposit));
        add_material(deposited_by_stockpile, stockpile_id.clone(), deposit);
    }
}

fn apply_reclaims(
    working_balances: &mut BTreeMap<StockpileId, MaterialParcel>,
    reclaims_by_stockpile: &BTreeMap<StockpileId, Vec<StockpileReclaim>>,
    reclaimed_by_stockpile: &mut BTreeMap<StockpileId, MaterialParcel>,
    reclaimed_by_destination: &mut BTreeMap<DestinationId, MaterialParcel>,
    contributing_stockpiles: &mut BTreeMap<DestinationId, BTreeSet<StockpileId>>,
) -> Result<(), MineError> {
    for (stockpile_id, requests) in reclaims_by_stockpile {
        let current_balance = working_balances
            .get(stockpile_id)
            .cloned()
            .unwrap_or_else(MaterialParcel::zero);
        let requested_tonnes: f64 = requests.iter().map(StockpileReclaim::tonnes).sum();
        let reclaimed_total = current_balance.take_tonnes(requested_tonnes)?;
        let closing_balance = current_balance.subtract(&reclaimed_total)?;
        working_balances.insert(stockpile_id.clone(), closing_balance);
        add_material(reclaimed_by_stockpile, stockpile_id.clone(), &reclaimed_total);

        for request in requests {
            let fraction = if normalize_value(requested_tonnes) == 0.0 {
                0.0
            } else {
                request.tonnes() / requested_tonnes
            };
            let parcel = reclaimed_total.scale(fraction)?;
            add_material(
                reclaimed_by_destination,
                request.destination_id().clone(),
                &parcel,
            );
            contributing_stockpiles
                .entry(request.destination_id().clone())
                .or_default()
                .insert(stockpile_id.clone());
        }
    }

    Ok(())
}

fn build_destination_blends(
    direct_by_destination: BTreeMap<DestinationId, MaterialParcel>,
    reclaimed_by_destination: BTreeMap<DestinationId, MaterialParcel>,
    contributing_stockpiles: BTreeMap<DestinationId, BTreeSet<StockpileId>>,
) -> Vec<DestinationBlendReport> {
    let destination_ids: BTreeSet<_> = direct_by_destination
        .keys()
        .chain(reclaimed_by_destination.keys())
        .cloned()
        .collect();

    destination_ids
        .into_iter()
        .map(|destination_id| {
            let direct_feed = direct_by_destination
                .get(&destination_id)
                .cloned()
                .unwrap_or_else(MaterialParcel::zero);
            let reclaimed_feed = reclaimed_by_destination
                .get(&destination_id)
                .cloned()
                .unwrap_or_else(MaterialParcel::zero);
            let blended_feed = direct_feed.add(&reclaimed_feed);

            DestinationBlendReport {
                destination_id: destination_id.clone(),
                direct_feed,
                reclaimed_feed,
                blended_feed: blended_feed.clone(),
                blended_grades: blended_feed.average_grades(),
                contributing_stockpiles: contributing_stockpiles
                    .get(&destination_id)
                    .map(|stockpiles| stockpiles.iter().cloned().collect())
                    .unwrap_or_default(),
            }
        })
        .collect()
}

fn add_material<K>(
    target: &mut BTreeMap<K, MaterialParcel>,
    key: K,
    parcel: &MaterialParcel,
) where
    K: Ord,
{
    target
        .entry(key)
        .and_modify(|existing| *existing = existing.add(parcel))
        .or_insert_with(|| parcel.clone());
}

fn validate_fraction(parameter: &'static str, value: f64) -> Result<(), MineError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(MineError::invalid_parameter(
            parameter,
            "fraction must be finite and between 0.0 and 1.0",
        ));
    }
    Ok(())
}

fn normalize_value(value: f64) -> f64 {
    if value.abs() <= MATERIAL_TOLERANCE {
        0.0
    } else {
        value
    }
}
