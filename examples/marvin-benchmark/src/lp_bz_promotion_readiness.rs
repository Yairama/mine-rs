use serde::Serialize;

use crate::lp_bz_runtime_budget::{
    LpBzLocalOptimizerRuntimeBudgetContract, validate_lp_bz_local_optimizer_runtime_budget_contract,
};

const READY_FRONTIER_SIGNAL_ID: &str = "ready-frontier-main-candidate";
const NATIVE_LP_SOLVE_SIGNAL_ID: &str = "native-lp-solve-skipped";
const LOCAL_OPTIMIZATION_SIGNAL_ID: &str = "local-optimization-skipped";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzPromotionReadinessSignal {
    pub signal_id: String,
    pub active: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LpBzPromotionReadinessSummary {
    pub promotion_state: String,
    pub comparison_classification: String,
    pub promoted_unit_family_label: String,
    pub promoted_build_label: String,
    pub local_optimizer_runtime_budget_contract: LpBzLocalOptimizerRuntimeBudgetContract,
    pub signals: Vec<LpBzPromotionReadinessSignal>,
    pub blocking_reasons: Vec<String>,
    pub summary: String,
}

pub fn build_lp_bz_promotion_readiness_summary(
    comparison_classification: &str,
    promoted_unit_family_label: &str,
    promoted_build_label: &str,
    ready_frontier_remains_main_candidate: bool,
    native_lp_solve_skipped: bool,
    local_optimizer_runtime_budget_contract: &LpBzLocalOptimizerRuntimeBudgetContract,
) -> LpBzPromotionReadinessSummary {
    let local_optimization_skipped =
        local_optimizer_runtime_budget_contract.execution_state == "skipped";
    let signals = vec![
        build_signal(
            READY_FRONTIER_SIGNAL_ID,
            ready_frontier_remains_main_candidate,
            "ready_frontier remains the main candidate, so the promoted LP/BZ family is still benchmark-side evidence rather than the primary scheduling route",
        ),
        build_signal(
            NATIVE_LP_SOLVE_SIGNAL_ID,
            native_lp_solve_skipped,
            "the native LP solve is skipped, so the effective LP/BZ bound is still reported without a refreshed native kernel solve",
        ),
        build_signal(
            LOCAL_OPTIMIZATION_SIGNAL_ID,
            local_optimization_skipped,
            "local optimization is skipped, so the promoted LP/BZ candidate still stops at deterministic round/repair evidence",
        ),
    ];
    let blocking_reasons = signals
        .iter()
        .filter(|signal| signal.active)
        .map(|signal| signal.summary.clone())
        .collect::<Vec<_>>();
    let promotion_state = if blocking_reasons.is_empty() {
        "active-candidate"
    } else {
        "sidecar-only"
    }
    .to_owned();
    let runtime_contract_summary = format!(
        " Runtime contract: {}",
        local_optimizer_runtime_budget_contract.summary
    );
    let summary = if blocking_reasons.is_empty() {
        format!(
            "`{promoted_unit_family_label}` (`{promoted_build_label}`) is the active benchmark-side candidate route and remains `{comparison_classification}` until broader literature-grade comparability gaps close.{runtime_contract_summary}"
        )
    } else {
        format!(
            "`{promoted_unit_family_label}` (`{promoted_build_label}`) stays `{promotion_state}` / `{comparison_classification}` because {}.{runtime_contract_summary}",
            blocking_reasons.join("; "),
        )
    };

    LpBzPromotionReadinessSummary {
        promotion_state,
        comparison_classification: comparison_classification.to_owned(),
        promoted_unit_family_label: promoted_unit_family_label.to_owned(),
        promoted_build_label: promoted_build_label.to_owned(),
        local_optimizer_runtime_budget_contract: local_optimizer_runtime_budget_contract.clone(),
        signals,
        blocking_reasons,
        summary,
    }
}

pub fn validate_lp_bz_promotion_readiness_summary(
    summary: &LpBzPromotionReadinessSummary,
) -> Result<(), String> {
    validate_lp_bz_local_optimizer_runtime_budget_contract(
        &summary.local_optimizer_runtime_budget_contract,
    )?;
    let expected_signal_ids = [
        READY_FRONTIER_SIGNAL_ID,
        NATIVE_LP_SOLVE_SIGNAL_ID,
        LOCAL_OPTIMIZATION_SIGNAL_ID,
    ];
    if summary.signals.len() != expected_signal_ids.len() {
        return Err(format!(
            "LP/BZ promotion readiness summary must expose {} audit signals, received {}.",
            expected_signal_ids.len(),
            summary.signals.len()
        ));
    }
    for (signal, expected_signal_id) in summary.signals.iter().zip(expected_signal_ids) {
        if signal.signal_id != expected_signal_id {
            return Err(format!(
                "LP/BZ promotion readiness signal order drifted: expected `{expected_signal_id}`, received `{}`.",
                signal.signal_id
            ));
        }
        if signal.summary.trim().is_empty() {
            return Err(format!(
                "LP/BZ promotion readiness signal `{}` must explain its audit condition.",
                signal.signal_id
            ));
        }
    }
    let active_reasons = summary
        .signals
        .iter()
        .filter(|signal| signal.active)
        .map(|signal| signal.summary.clone())
        .collect::<Vec<_>>();
    if summary.blocking_reasons != active_reasons {
        return Err(
            "LP/BZ promotion readiness blocking reasons must match the active audit signals."
                .to_owned(),
        );
    }
    if summary.promotion_state == "sidecar-only" && summary.blocking_reasons.is_empty() {
        return Err(
            "LP/BZ promotion readiness cannot stay `sidecar-only` without explicit blocking reasons."
                .to_owned(),
        );
    }
    if summary.summary.trim().is_empty() {
        return Err("LP/BZ promotion readiness must include a human-readable summary.".to_owned());
    }
    if !summary
        .summary
        .contains(&summary.promoted_unit_family_label)
        || !summary.summary.contains(&summary.promoted_build_label)
        || !summary.summary.contains(&summary.comparison_classification)
    {
        return Err(
            "LP/BZ promotion readiness summary must mention the promoted family, build label and classification."
                .to_owned(),
        );
    }
    if !summary
        .summary
        .contains(&summary.local_optimizer_runtime_budget_contract.summary)
    {
        return Err(
            "LP/BZ promotion readiness summary must embed the explicit local optimizer runtime budget contract summary."
                .to_owned(),
        );
    }
    Ok(())
}

fn build_signal(signal_id: &str, active: bool, summary: &str) -> LpBzPromotionReadinessSignal {
    LpBzPromotionReadinessSignal {
        signal_id: signal_id.to_owned(),
        active,
        summary: summary.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_lp_bz_promotion_readiness_summary, validate_lp_bz_promotion_readiness_summary,
    };
    use crate::lp_bz_runtime_budget::build_lp_bz_local_optimizer_runtime_budget_contract;

    #[test]
    fn promotion_readiness_keeps_active_signal_summaries_in_order() {
        let runtime_budget_contract = build_lp_bz_local_optimizer_runtime_budget_contract(
            "deterministic-local-v8",
            0,
            0,
            "skipped-focused-refresh-runtime",
        );
        let readiness = build_lp_bz_promotion_readiness_summary(
            "exploratory-local",
            "pushback-bench-localized-cut-phase",
            "promoted-lp-bz-family",
            true,
            true,
            &runtime_budget_contract,
        );

        validate_lp_bz_promotion_readiness_summary(&readiness)
            .expect("promotion readiness should validate");
        assert_eq!(readiness.promotion_state, "sidecar-only");
        assert_eq!(readiness.blocking_reasons.len(), 3);
        assert_eq!(
            readiness
                .local_optimizer_runtime_budget_contract
                .execution_state,
            "skipped"
        );
        assert!(
            readiness
                .summary
                .contains("ready_frontier remains the main candidate")
        );
        assert!(readiness.summary.contains("native LP solve is skipped"));
        assert!(readiness.summary.contains("local optimization is skipped"));
        assert!(readiness.summary.contains("Runtime contract:"));
    }
}
