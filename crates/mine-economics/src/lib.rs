//! Economía determinista para modelos de bloques.

mod block_economics;
mod block_valuation;
mod destination_pure_preprocessing;
mod destinations;
mod economic_block_model;
mod nsr;
mod schedule_economics;
mod scheduling_problem_adapter;
mod stockpile;
mod stockpile_scheduling_staging;

pub use block_economics::{
    BlockEconomics, BlockEconomicsReport, EconomicAssumptions, EconomicUnits, PeriodCashflowInput,
    ScenarioCashflowReport, ScenarioPeriodCashflow, evaluate_block_economics,
    evaluate_scenario_cashflow,
};
pub use block_valuation::{
    BlockDestinationValue, BlockGrades, MultiDestinationBlockValuation, value_block_by_destinations,
};
pub use destination_pure_preprocessing::{
    DestinationPurePhaseRefinement, DestinationPurePushbackPlan,
    refine_pushback_plan_to_destination_pure,
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
    evaluate_long_term_schedule_economics,
    evaluate_long_term_schedule_economics_with_reclaim_policy,
    evaluate_long_term_schedule_sensitivity_pack, summarize_long_term_schedule_risk,
};
pub use scheduling_problem_adapter::{
    StagedStockpileReclaimDownstreamProfile, StagedStockpileReclaimPolicy,
    StagedStockpileReclaimRule, build_scheduling_problem_from_economic_block_model,
    build_scheduling_problem_from_economic_block_model_with_reclaim_policy,
};
pub use stockpile::{
    DestinationBlendReport, DirectDestinationFeed, MaterialParcel, StockpileBalanceReport,
    StockpileDefinition, StockpileDegradation, StockpileDeposit, StockpileId,
    StockpileInventorySnapshot, StockpilePeriodInput, StockpilePeriodReport, StockpilePlanInput,
    StockpilePlanReport, StockpileReclaim, StockpileTransactionOrder, evaluate_stockpile_plan,
};
pub use stockpile_scheduling_staging::{
    StockpileSchedulingStage, StockpileTargetParcel, stage_pushback_plan_for_stockpile_readiness,
};
