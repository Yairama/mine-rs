use serde::Serialize;

use crate::lp_bz_rounder::local_optimizer_runtime_was_skipped;

pub const LOCAL_OPTIMIZER_RUNTIME_COMPLETED_STATE: &str = "completed-within-budget";
pub const LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE: &str = "budget-hit";
pub const LOCAL_OPTIMIZER_RUNTIME_SKIPPED_STATE: &str = "skipped";
pub const LOCAL_OPTIMIZER_MAX_ITERATIONS_TERMINATION_REASON: &str = "max-iterations-reached";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzLocalOptimizerRuntimeBudgetContract {
    pub contract_scope: String,
    pub strategy_label: String,
    pub budget_metric: String,
    pub max_iteration_count: usize,
    pub executed_iteration_count: usize,
    pub termination_reason: String,
    pub execution_state: String,
    pub budget_hit: bool,
    pub summary: String,
}

pub fn build_lp_bz_local_optimizer_runtime_budget_contract(
    strategy_label: &str,
    max_iteration_count: usize,
    executed_iteration_count: usize,
    termination_reason: &str,
) -> LpBzLocalOptimizerRuntimeBudgetContract {
    let execution_state = if local_optimizer_runtime_was_skipped(termination_reason) {
        LOCAL_OPTIMIZER_RUNTIME_SKIPPED_STATE
    } else if local_optimizer_runtime_budget_was_hit(termination_reason) {
        LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE
    } else {
        LOCAL_OPTIMIZER_RUNTIME_COMPLETED_STATE
    }
    .to_owned();
    let budget_hit = execution_state == LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE;
    let summary = match execution_state.as_str() {
        LOCAL_OPTIMIZER_RUNTIME_SKIPPED_STATE => format!(
            "Local optimizer `{strategy_label}` was skipped with `{termination_reason}`, so the promoted LP/BZ path reports an explicit non-executed runtime contract instead of a silent placeholder."
        ),
        LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE => format!(
            "Local optimizer `{strategy_label}` hit the explicit iteration budget at {executed_iteration_count}/{max_iteration_count} iterations (`{termination_reason}`)."
        ),
        _ => format!(
            "Local optimizer `{strategy_label}` completed within the explicit iteration budget after {executed_iteration_count}/{max_iteration_count} iterations (`{termination_reason}`)."
        ),
    };

    LpBzLocalOptimizerRuntimeBudgetContract {
        contract_scope: "promoted-lp-bz-local-optimizer".to_owned(),
        strategy_label: strategy_label.to_owned(),
        budget_metric: "iteration-count".to_owned(),
        max_iteration_count,
        executed_iteration_count,
        termination_reason: termination_reason.to_owned(),
        execution_state,
        budget_hit,
        summary,
    }
}

pub fn local_optimizer_runtime_budget_was_hit(termination_reason: &str) -> bool {
    termination_reason == LOCAL_OPTIMIZER_MAX_ITERATIONS_TERMINATION_REASON
}

pub fn validate_lp_bz_local_optimizer_runtime_budget_contract(
    contract: &LpBzLocalOptimizerRuntimeBudgetContract,
) -> Result<(), String> {
    if contract.contract_scope != "promoted-lp-bz-local-optimizer" {
        return Err(
            "LP/BZ local optimizer runtime budget contract must stay scoped to the promoted local optimizer."
                .to_owned(),
        );
    }
    if contract.budget_metric != "iteration-count" {
        return Err(
            "LP/BZ local optimizer runtime budget contract must declare `iteration-count` as its budget metric."
                .to_owned(),
        );
    }
    if contract.strategy_label.trim().is_empty() {
        return Err(
            "LP/BZ local optimizer runtime budget contract must include the optimizer strategy label."
                .to_owned(),
        );
    }
    if contract.summary.trim().is_empty() {
        return Err(
            "LP/BZ local optimizer runtime budget contract must include a human-readable summary."
                .to_owned(),
        );
    }
    if !contract.summary.contains(&contract.strategy_label)
        || !contract.summary.contains(&contract.termination_reason)
    {
        return Err(
            "LP/BZ local optimizer runtime budget contract summary must mention the strategy label and termination reason."
                .to_owned(),
        );
    }

    let expected_execution_state =
        if local_optimizer_runtime_was_skipped(&contract.termination_reason) {
            LOCAL_OPTIMIZER_RUNTIME_SKIPPED_STATE
        } else if local_optimizer_runtime_budget_was_hit(&contract.termination_reason) {
            LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE
        } else {
            LOCAL_OPTIMIZER_RUNTIME_COMPLETED_STATE
        };
    if contract.execution_state != expected_execution_state {
        return Err(format!(
            "LP/BZ local optimizer runtime budget contract execution_state drifted: expected `{expected_execution_state}`, received `{}`.",
            contract.execution_state
        ));
    }
    if contract.budget_hit != (contract.execution_state == LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE)
    {
        return Err(
            "LP/BZ local optimizer runtime budget contract must keep `budget_hit` aligned with `execution_state`."
                .to_owned(),
        );
    }
    if contract.execution_state == LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE
        && contract.executed_iteration_count != contract.max_iteration_count
    {
        return Err(
            "LP/BZ local optimizer runtime budget hit contracts must report executed_iteration_count equal to max_iteration_count."
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE, LOCAL_OPTIMIZER_RUNTIME_COMPLETED_STATE,
        LOCAL_OPTIMIZER_RUNTIME_SKIPPED_STATE, build_lp_bz_local_optimizer_runtime_budget_contract,
        validate_lp_bz_local_optimizer_runtime_budget_contract,
    };

    #[test]
    fn runtime_budget_contract_distinguishes_budget_hit_from_skipped() {
        let budget_hit = build_lp_bz_local_optimizer_runtime_budget_contract(
            "deterministic-local-v8",
            12,
            12,
            "max-iterations-reached",
        );
        let skipped = build_lp_bz_local_optimizer_runtime_budget_contract(
            "deterministic-local-v8",
            0,
            0,
            "skipped-focused-refresh-runtime",
        );

        validate_lp_bz_local_optimizer_runtime_budget_contract(&budget_hit)
            .expect("budget-hit contract should validate");
        validate_lp_bz_local_optimizer_runtime_budget_contract(&skipped)
            .expect("skipped contract should validate");
        assert_eq!(
            budget_hit.execution_state,
            LOCAL_OPTIMIZER_RUNTIME_BUDGET_HIT_STATE
        );
        assert!(budget_hit.budget_hit);
        assert_eq!(
            skipped.execution_state,
            LOCAL_OPTIMIZER_RUNTIME_SKIPPED_STATE
        );
        assert!(!skipped.budget_hit);
    }

    #[test]
    fn runtime_budget_contract_marks_completed_execution_without_skip() {
        let completed = build_lp_bz_local_optimizer_runtime_budget_contract(
            "deterministic-local-v8",
            12,
            2,
            "no-improving-local-move",
        );

        validate_lp_bz_local_optimizer_runtime_budget_contract(&completed)
            .expect("completed contract should validate");
        assert_eq!(
            completed.execution_state,
            LOCAL_OPTIMIZER_RUNTIME_COMPLETED_STATE
        );
        assert!(!completed.budget_hit);
    }
}
