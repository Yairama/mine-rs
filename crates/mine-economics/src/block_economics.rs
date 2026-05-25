//! Economía determinista para modelos de bloques.

use mine_blockmodel::{BlockModel, ColumnData};
use mine_core::{ColumnId, ColumnLogicalType, MeasurementUnit, MineError};
use mine_planning::MiningScenario;
use serde::{Deserialize, Serialize};

/// Supuestos económicos explícitos para evaluar bloques.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomicAssumptions {
    price_per_recovered_metal_unit: f64,
    selling_cost_per_recovered_metal_unit: f64,
    mining_cost_per_tonne: f64,
    processing_cost_per_tonne: f64,
    recovery: f64,
    units: EconomicUnits,
}

/// Unidades explícitas usadas por los supuestos económicos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicUnits {
    grade_unit: MeasurementUnit,
    tonnage_unit: MeasurementUnit,
    recovered_metal_unit: MeasurementUnit,
}

impl EconomicUnits {
    /// Construye las unidades explícitas requeridas por economía.
    #[must_use]
    pub fn new(
        grade_unit: MeasurementUnit,
        tonnage_unit: MeasurementUnit,
        recovered_metal_unit: MeasurementUnit,
    ) -> Self {
        Self {
            grade_unit,
            tonnage_unit,
            recovered_metal_unit,
        }
    }

    /// Unidad esperada para la ley.
    #[must_use]
    pub fn grade_unit(&self) -> &MeasurementUnit {
        &self.grade_unit
    }

    /// Unidad esperada para el tonelaje.
    #[must_use]
    pub fn tonnage_unit(&self) -> &MeasurementUnit {
        &self.tonnage_unit
    }

    /// Unidad usada para precio/costo de metal recuperado.
    #[must_use]
    pub fn recovered_metal_unit(&self) -> &MeasurementUnit {
        &self.recovered_metal_unit
    }
}

impl EconomicAssumptions {
    /// Construye supuestos económicos explícitos con validación.
    pub fn new(
        price_per_recovered_metal_unit: f64,
        selling_cost_per_recovered_metal_unit: f64,
        mining_cost_per_tonne: f64,
        processing_cost_per_tonne: f64,
        recovery: f64,
        units: EconomicUnits,
    ) -> Result<Self, MineError> {
        validate_positive_finite(
            "price_per_recovered_metal_unit",
            price_per_recovered_metal_unit,
        )?;
        validate_non_negative_finite(
            "selling_cost_per_recovered_metal_unit",
            selling_cost_per_recovered_metal_unit,
        )?;
        validate_non_negative_finite("mining_cost_per_tonne", mining_cost_per_tonne)?;
        validate_non_negative_finite("processing_cost_per_tonne", processing_cost_per_tonne)?;
        validate_recovery(recovery)?;
        validate_grade_unit(units.grade_unit())?;

        Ok(Self {
            price_per_recovered_metal_unit,
            selling_cost_per_recovered_metal_unit,
            mining_cost_per_tonne,
            processing_cost_per_tonne,
            recovery,
            units,
        })
    }

    /// Precio por unidad de metal recuperado.
    #[must_use]
    pub const fn price_per_recovered_metal_unit(&self) -> f64 {
        self.price_per_recovered_metal_unit
    }

    /// Costo de venta por unidad de metal recuperado.
    #[must_use]
    pub const fn selling_cost_per_recovered_metal_unit(&self) -> f64 {
        self.selling_cost_per_recovered_metal_unit
    }

    /// Costo de minado por tonelada.
    #[must_use]
    pub const fn mining_cost_per_tonne(&self) -> f64 {
        self.mining_cost_per_tonne
    }

    /// Costo de procesamiento por tonelada.
    #[must_use]
    pub const fn processing_cost_per_tonne(&self) -> f64 {
        self.processing_cost_per_tonne
    }

    /// Recuperación metalúrgica asumida.
    #[must_use]
    pub const fn recovery(&self) -> f64 {
        self.recovery
    }

    /// Unidad esperada para la ley.
    #[must_use]
    pub fn grade_unit(&self) -> &MeasurementUnit {
        self.units.grade_unit()
    }

    /// Unidad esperada para el tonelaje.
    #[must_use]
    pub fn tonnage_unit(&self) -> &MeasurementUnit {
        self.units.tonnage_unit()
    }

    /// Unidad usada para precio/costo de metal recuperado.
    #[must_use]
    pub fn recovered_metal_unit(&self) -> &MeasurementUnit {
        self.units.recovered_metal_unit()
    }
}

/// Resultado económico determinista por bloque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockEconomics {
    /// Índice lineal del bloque evaluado.
    pub block_index: usize,
    /// Tonelaje del bloque.
    pub tonnage: f64,
    /// Ley original del bloque.
    pub grade: f64,
    /// Metal contenido en unidades del tonelaje base.
    pub contained_metal: f64,
    /// Metal recuperado tras aplicar recuperación.
    pub recovered_metal: f64,
    /// Revenue bruto del bloque.
    pub revenue: f64,
    /// Costo de venta del bloque.
    pub selling_cost: f64,
    /// Costo de minado del bloque.
    pub mining_cost: f64,
    /// Costo de procesamiento del bloque.
    pub processing_cost: f64,
    /// Costo total del bloque.
    pub total_cost: f64,
    /// Margen del bloque.
    pub margin: f64,
}

/// Resultado agregado de economía por bloque para un modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockEconomicsReport {
    /// Columna de ley evaluada.
    pub grade_column: ColumnId,
    /// Columna de tonelaje evaluada.
    pub tonnage_column: ColumnId,
    /// Supuestos económicos aplicados.
    pub assumptions: EconomicAssumptions,
    /// Resultados por bloque.
    pub blocks: Vec<BlockEconomics>,
    /// Revenue total.
    pub total_revenue: f64,
    /// Costo total.
    pub total_cost: f64,
    /// Margen total.
    pub total_margin: f64,
}

/// Input explícito de revenue y costo para un periodo de escenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeriodCashflowInput {
    period_label: String,
    revenue: f64,
    cost: f64,
}

impl PeriodCashflowInput {
    /// Construye un input de cashflow validado por periodo.
    pub fn new(
        period_label: impl Into<String>,
        revenue: f64,
        cost: f64,
    ) -> Result<Self, MineError> {
        let period_label = period_label.into();

        if period_label.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "period_label",
                "cashflow period label must not be empty",
            ));
        }

        if !revenue.is_finite() {
            return Err(MineError::invalid_parameter(
                "revenue",
                "cashflow revenue must be finite",
            ));
        }

        if !cost.is_finite() || cost < 0.0 {
            return Err(MineError::invalid_parameter(
                "cost",
                "cashflow cost must be finite and greater than or equal to zero",
            ));
        }

        Ok(Self {
            period_label,
            revenue,
            cost,
        })
    }

    /// Etiqueta del periodo.
    #[must_use]
    pub fn period_label(&self) -> &str {
        &self.period_label
    }

    /// Revenue del periodo.
    #[must_use]
    pub const fn revenue(&self) -> f64 {
        self.revenue
    }

    /// Costo del periodo.
    #[must_use]
    pub const fn cost(&self) -> f64 {
        self.cost
    }
}

/// Resultado financiero por periodo dentro de un escenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPeriodCashflow {
    /// Etiqueta del periodo.
    pub period_label: String,
    /// Revenue del periodo.
    pub revenue: f64,
    /// Costo del periodo.
    pub cost: f64,
    /// Cashflow no descontado.
    pub cashflow: f64,
    /// Factor de descuento aplicado.
    pub discount_factor: f64,
    /// Cashflow descontado.
    pub discounted_cashflow: f64,
}

/// Reporte financiero agregado para un escenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioCashflowReport {
    /// Identificador del escenario evaluado.
    pub scenario_id: String,
    /// Cashflow detallado por periodo.
    pub periods: Vec<ScenarioPeriodCashflow>,
    /// Revenue total.
    pub total_revenue: f64,
    /// Costo total.
    pub total_cost: f64,
    /// Cashflow total sin descuento.
    pub total_cashflow: f64,
    /// Valor presente neto bajo la tasa configurada.
    pub npv: f64,
    /// Tasa de descuento por periodo usada en el cálculo.
    pub discount_rate_per_period: f64,
}

/// Calcula revenue, costo y margen por bloque usando supuestos explícitos.
pub fn evaluate_block_economics(
    model: &BlockModel,
    grade_column: &ColumnId,
    tonnage_column: &ColumnId,
    assumptions: &EconomicAssumptions,
) -> Result<BlockEconomicsReport, MineError> {
    validate_column_unit(model, grade_column, assumptions.grade_unit(), "grade")?;
    validate_column_unit(model, tonnage_column, assumptions.tonnage_unit(), "tonnage")?;
    let grade_values = float_column(model, grade_column, "grade")?;
    let tonnage_values = float_column(model, tonnage_column, "tonnage")?;
    let grade_factor =
        grade_to_fraction_factor(assumptions.grade_unit()).ok_or_else(|| MineError::Economics {
            message: format!(
                "grade unit `{}` is not supported for block economics",
                assumptions.grade_unit().as_str()
            ),
        })?;
    let mut blocks = Vec::with_capacity(model.block_count());
    let mut total_revenue = 0.0;
    let mut total_cost = 0.0;

    for block_index in 0..model.block_count() {
        let tonnage = tonnage_values[block_index];
        let grade = grade_values[block_index];

        if tonnage < 0.0 {
            return Err(MineError::Economics {
                message: format!(
                    "block `{block_index}` has negative tonnage `{tonnage}` and cannot be evaluated"
                ),
            });
        }

        let contained_metal = tonnage * grade * grade_factor;
        let recovered_metal = contained_metal * assumptions.recovery();
        let revenue = recovered_metal * assumptions.price_per_recovered_metal_unit();
        let selling_cost = recovered_metal * assumptions.selling_cost_per_recovered_metal_unit();
        let mining_cost = tonnage * assumptions.mining_cost_per_tonne();
        let processing_cost = tonnage * assumptions.processing_cost_per_tonne();
        let total_block_cost = selling_cost + mining_cost + processing_cost;
        let margin = revenue - total_block_cost;

        total_revenue += revenue;
        total_cost += total_block_cost;
        blocks.push(BlockEconomics {
            block_index,
            tonnage,
            grade,
            contained_metal,
            recovered_metal,
            revenue,
            selling_cost,
            mining_cost,
            processing_cost,
            total_cost: total_block_cost,
            margin,
        });
    }

    Ok(BlockEconomicsReport {
        grade_column: grade_column.clone(),
        tonnage_column: tonnage_column.clone(),
        assumptions: assumptions.clone(),
        blocks,
        total_revenue,
        total_cost,
        total_margin: total_revenue - total_cost,
    })
}

/// Calcula cashflow por periodo y NPV para un escenario.
///
/// Asunción explícita: el primer periodo del escenario se descuenta con factor `1.0`
/// y cada periodo siguiente aplica `1 / (1 + r)^n` usando el orden declarado en el escenario.
pub fn evaluate_scenario_cashflow(
    scenario: &MiningScenario,
    period_inputs: &[PeriodCashflowInput],
    discount_rate_per_period: f64,
) -> Result<ScenarioCashflowReport, MineError> {
    validate_discount_rate(discount_rate_per_period)?;
    let period_inputs = index_period_inputs(period_inputs)?;
    let mut periods = Vec::with_capacity(scenario.periods().len());
    let mut total_revenue = 0.0;
    let mut total_cost = 0.0;
    let mut npv = 0.0;

    for (period_index, period) in scenario.periods().iter().enumerate() {
        let input = period_inputs
            .get(period.label())
            .ok_or_else(|| MineError::Economics {
                message: format!(
                    "scenario period `{}` is missing from cashflow inputs",
                    period.label()
                ),
            })?;
        let cashflow = input.revenue() - input.cost();
        let discount_factor = (1.0 + discount_rate_per_period).powi(-(period_index as i32));
        let discounted_cashflow = cashflow * discount_factor;

        total_revenue += input.revenue();
        total_cost += input.cost();
        npv += discounted_cashflow;
        periods.push(ScenarioPeriodCashflow {
            period_label: period.label().to_owned(),
            revenue: input.revenue(),
            cost: input.cost(),
            cashflow,
            discount_factor,
            discounted_cashflow,
        });
    }

    for period_label in period_inputs.keys() {
        if !scenario
            .periods()
            .iter()
            .any(|period| period.label() == *period_label)
        {
            return Err(MineError::Economics {
                message: format!(
                    "cashflow input period `{period_label}` does not exist in the scenario"
                ),
            });
        }
    }

    Ok(ScenarioCashflowReport {
        scenario_id: scenario.scenario_id().to_string(),
        periods,
        total_revenue,
        total_cost,
        total_cashflow: total_revenue - total_cost,
        npv,
        discount_rate_per_period,
    })
}

fn validate_positive_finite(parameter: &'static str, value: f64) -> Result<(), MineError> {
    if !value.is_finite() || value <= 0.0 {
        Err(MineError::invalid_parameter(
            parameter,
            "must be finite and greater than zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_non_negative_finite(parameter: &'static str, value: f64) -> Result<(), MineError> {
    if !value.is_finite() || value < 0.0 {
        Err(MineError::invalid_parameter(
            parameter,
            "must be finite and greater than or equal to zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_recovery(recovery: f64) -> Result<(), MineError> {
    if !recovery.is_finite() || !(0.0..=1.0).contains(&recovery) {
        Err(MineError::invalid_parameter(
            "recovery",
            "must be finite and within 0..=1",
        ))
    } else {
        Ok(())
    }
}

fn validate_grade_unit(grade_unit: &MeasurementUnit) -> Result<(), MineError> {
    if grade_to_fraction_factor(grade_unit).is_none() {
        return Err(MineError::invalid_parameter(
            "grade_unit",
            "must be convertible to a fraction (%... or ppm)",
        ));
    }

    Ok(())
}

fn validate_discount_rate(discount_rate_per_period: f64) -> Result<(), MineError> {
    if !discount_rate_per_period.is_finite() || discount_rate_per_period <= -1.0 {
        Err(MineError::invalid_parameter(
            "discount_rate_per_period",
            "must be finite and greater than -1.0",
        ))
    } else {
        Ok(())
    }
}

fn index_period_inputs(
    period_inputs: &[PeriodCashflowInput],
) -> Result<std::collections::BTreeMap<&str, &PeriodCashflowInput>, MineError> {
    let mut indexed = std::collections::BTreeMap::new();

    for input in period_inputs {
        if indexed.insert(input.period_label(), input).is_some() {
            return Err(MineError::Economics {
                message: format!(
                    "cashflow input period `{}` is duplicated",
                    input.period_label()
                ),
            });
        }
    }

    Ok(indexed)
}

fn validate_column_unit(
    model: &BlockModel,
    column_id: &ColumnId,
    expected_unit: &MeasurementUnit,
    purpose: &str,
) -> Result<(), MineError> {
    let column_schema = model.schema().get(column_id).ok_or_else(|| {
        MineError::schema(format!(
            "{purpose} column `{column_id}` does not exist in block model schema"
        ))
    })?;
    let actual_unit = column_schema.unit().ok_or_else(|| MineError::Economics {
        message: format!(
            "{purpose} column `{column_id}` must declare unit `{}` for economics",
            expected_unit.as_str()
        ),
    })?;

    if actual_unit != expected_unit {
        return Err(MineError::Economics {
            message: format!(
                "{purpose} column `{column_id}` uses unit `{}` but economics expect `{}`",
                actual_unit.as_str(),
                expected_unit.as_str()
            ),
        });
    }

    Ok(())
}

fn float_column<'a>(
    model: &'a BlockModel,
    column_id: &ColumnId,
    purpose: &str,
) -> Result<&'a [f64], MineError> {
    let Some(column_schema) = model.schema().get(column_id) else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` does not exist in block model schema"
        )));
    };

    if column_schema.logical_type() != ColumnLogicalType::Float {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` must be a float column"
        )));
    }

    let Some(column_data) = model.column(column_id) else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` does not exist in block model storage"
        )));
    };
    let ColumnData::Floats(values) = column_data else {
        return Err(MineError::schema(format!(
            "{purpose} column `{column_id}` must be a float column"
        )));
    };

    Ok(values)
}

fn grade_to_fraction_factor(unit: &MeasurementUnit) -> Option<f64> {
    let unit = unit.as_str().to_ascii_lowercase();

    if unit.starts_with('%') {
        return Some(0.01);
    }

    if unit == "ppm" {
        return Some(1e-6);
    }

    None
}
