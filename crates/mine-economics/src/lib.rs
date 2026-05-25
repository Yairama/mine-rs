//! Economía determinista para modelos de bloques.

mod block_economics;
mod block_valuation;
mod destinations;
mod economic_block_model;
mod nsr;
mod schedule_economics;
mod stockpile;

pub use block_economics::{
    BlockEconomics, BlockEconomicsReport, EconomicAssumptions, EconomicUnits, PeriodCashflowInput,
    ScenarioCashflowReport, ScenarioPeriodCashflow, evaluate_block_economics,
    evaluate_scenario_cashflow,
};
pub use block_valuation::{
    BlockDestinationValue, BlockGrades, MultiDestinationBlockValuation, value_block_by_destinations,
};
pub use destinations::{
    DestinationAssumptionSet, DestinationAssumptions, DestinationCapacity, DestinationId,
    DestinationKind, DestinationPayability, DestinationRecovery,
};
pub use economic_block_model::{
    BlockEconomicSummary, EconomicBlockModel, EconomicBlockModelConfig,
};
pub use nsr::{EvParameters, EvResult, NsrMetalInput, NsrResult, compute_ev, compute_nsr};
pub use schedule_economics::{
    LongTermScheduleEconomicsReport, LongTermSchedulePeriodEconomics,
    LongTermScheduleSensitivityCase, RiskMetricSummary, ScenarioComparison,
    ScenarioComparisonReport, ScenarioPeriodComparison, ScenarioRiskReport,
    evaluate_long_term_schedule_economics, evaluate_long_term_schedule_sensitivity_pack,
    summarize_long_term_schedule_risk,
};
pub use stockpile::{
    DestinationBlendReport, DirectDestinationFeed, MaterialParcel, StockpileBalanceReport,
    StockpileDefinition, StockpileDegradation, StockpileDeposit, StockpileId,
    StockpileInventorySnapshot, StockpilePeriodInput, StockpilePeriodReport, StockpilePlanInput,
    StockpilePlanReport, StockpileReclaim, StockpileTransactionOrder, evaluate_stockpile_plan,
};
