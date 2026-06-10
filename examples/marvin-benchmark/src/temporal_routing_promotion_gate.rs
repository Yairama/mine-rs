use serde::Serialize;

pub const TEMPORAL_ROUTING_PROMOTION_GATE_VERSION: &str = "mr206-v1";
pub const MAX_USED_PERIOD_COUNT_DELTA: usize = 2;
pub const MAX_MEAN_ABSOLUTE_PERIOD_DELTA: f64 = 1.0;
pub const MAX_EARLIER_THAN_REFERENCE_COUNT: usize = 500;
pub const MIN_PERIOD_DESTINATION_SIMILARITY: f64 = 0.25;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemporalRoutingPromotionGateThresholds {
    pub max_used_period_count_delta: usize,
    pub max_mean_absolute_period_delta: f64,
    pub max_earlier_than_reference_count: usize,
    pub min_period_destination_similarity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemporalRoutingPromotionGateMetric {
    pub metric_id: String,
    pub candidate_value: f64,
    pub reference_value: Option<f64>,
    pub compared_value: f64,
    pub threshold_value: f64,
    pub threshold_relation: String,
    pub passed: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemporalRoutingPromotionGateSummary {
    pub gate_version: String,
    pub promotion_decision: String,
    pub npv_improves_over_reference: bool,
    pub npv_delta_vs_reference: f64,
    pub temporal_routing_gate_passed: bool,
    pub thresholds: TemporalRoutingPromotionGateThresholds,
    pub metrics: Vec<TemporalRoutingPromotionGateMetric>,
    pub blocking_metric_ids: Vec<String>,
    pub summary: String,
}

pub fn build_temporal_routing_promotion_gate_summary(
    candidate_discounted_objective: f64,
    reference_discounted_objective: f64,
    candidate_used_period_count: usize,
    reference_used_period_count: usize,
    mean_absolute_period_delta: f64,
    earlier_than_reference_count: usize,
    period_destination_similarity: f64,
) -> TemporalRoutingPromotionGateSummary {
    let thresholds = TemporalRoutingPromotionGateThresholds {
        max_used_period_count_delta: MAX_USED_PERIOD_COUNT_DELTA,
        max_mean_absolute_period_delta: MAX_MEAN_ABSOLUTE_PERIOD_DELTA,
        max_earlier_than_reference_count: MAX_EARLIER_THAN_REFERENCE_COUNT,
        min_period_destination_similarity: MIN_PERIOD_DESTINATION_SIMILARITY,
    };
    let used_period_count_delta =
        candidate_used_period_count.abs_diff(reference_used_period_count) as f64;
    let metrics = vec![
        build_max_metric(
            "used_period_count",
            candidate_used_period_count as f64,
            Some(reference_used_period_count as f64),
            used_period_count_delta,
            thresholds.max_used_period_count_delta as f64,
            format!(
                "candidate uses {candidate_used_period_count} active periods vs {reference_used_period_count} in the reference (|Δ| = {used_period_count_delta:.0})."
            ),
        ),
        build_max_metric(
            "mean_absolute_period_delta",
            mean_absolute_period_delta,
            Some(0.0),
            mean_absolute_period_delta,
            thresholds.max_mean_absolute_period_delta,
            format!(
                "candidate shifts shared blocks by {mean_absolute_period_delta:.3} periods on average in absolute value."
            ),
        ),
        build_max_metric(
            "earlier_than_reference_count",
            earlier_than_reference_count as f64,
            Some(0.0),
            earlier_than_reference_count as f64,
            thresholds.max_earlier_than_reference_count as f64,
            format!(
                "candidate schedules {earlier_than_reference_count} shared blocks earlier than the public reference."
            ),
        ),
        build_min_metric(
            "period_destination_similarity",
            period_destination_similarity,
            None,
            period_destination_similarity,
            thresholds.min_period_destination_similarity,
            format!(
                "candidate keeps period/destination Jaccard similarity at {period_destination_similarity:.3}."
            ),
        ),
    ];
    let blocking_metric_ids = metrics
        .iter()
        .filter(|metric| !metric.passed)
        .map(|metric| metric.metric_id.clone())
        .collect::<Vec<_>>();
    let temporal_routing_gate_passed = blocking_metric_ids.is_empty();
    let npv_delta_vs_reference = candidate_discounted_objective - reference_discounted_objective;
    let npv_improves_over_reference = npv_delta_vs_reference > 1.0e-6;
    let promotion_decision = match (npv_improves_over_reference, temporal_routing_gate_passed) {
        (true, true) => "eligible-for-promotion",
        (true, false) => "blocked-by-temporal-routing",
        (false, true) => "blocked-by-npv",
        (false, false) => "blocked-by-npv-and-temporal-routing",
    }
    .to_owned();
    let summary = match (npv_improves_over_reference, temporal_routing_gate_passed) {
        (true, true) => format!(
            "Candidate improves discounted objective by {npv_delta_vs_reference:.3} and clears all MR-206 temporal/routing thresholds, so promotion may proceed."
        ),
        (true, false) => format!(
            "Candidate improves discounted objective by {npv_delta_vs_reference:.3}, but promotion remains blocked because temporal/routing gate metrics failed: {}.",
            blocking_metric_ids.join(", ")
        ),
        (false, true) => format!(
            "Candidate clears the MR-206 temporal/routing gate, but promotion still requires an NPV improvement (ΔNPV = {npv_delta_vs_reference:.3})."
        ),
        (false, false) => format!(
            "Candidate is not promotable because it does not improve NPV (ΔNPV = {npv_delta_vs_reference:.3}) and it fails temporal/routing gate metrics: {}.",
            blocking_metric_ids.join(", ")
        ),
    };

    TemporalRoutingPromotionGateSummary {
        gate_version: TEMPORAL_ROUTING_PROMOTION_GATE_VERSION.to_owned(),
        promotion_decision,
        npv_improves_over_reference,
        npv_delta_vs_reference,
        temporal_routing_gate_passed,
        thresholds,
        metrics,
        blocking_metric_ids,
        summary,
    }
}

pub fn validate_temporal_routing_promotion_gate_summary(
    summary: &TemporalRoutingPromotionGateSummary,
) -> Result<(), String> {
    if summary.gate_version != TEMPORAL_ROUTING_PROMOTION_GATE_VERSION {
        return Err(format!(
            "Temporal/routing promotion gate version drifted: expected `{TEMPORAL_ROUTING_PROMOTION_GATE_VERSION}`, received `{}`.",
            summary.gate_version
        ));
    }
    let expected_metric_ids = [
        "used_period_count",
        "mean_absolute_period_delta",
        "earlier_than_reference_count",
        "period_destination_similarity",
    ];
    if summary.metrics.len() != expected_metric_ids.len() {
        return Err(format!(
            "Temporal/routing promotion gate must expose {} metrics, received {}.",
            expected_metric_ids.len(),
            summary.metrics.len()
        ));
    }
    for (metric, expected_metric_id) in summary.metrics.iter().zip(expected_metric_ids) {
        if metric.metric_id != expected_metric_id {
            return Err(format!(
                "Temporal/routing promotion gate metric order drifted: expected `{expected_metric_id}`, received `{}`.",
                metric.metric_id
            ));
        }
        if metric.summary.trim().is_empty() {
            return Err(format!(
                "Temporal/routing promotion gate metric `{}` must explain the comparison.",
                metric.metric_id
            ));
        }
    }
    let expected_blocking_metric_ids = summary
        .metrics
        .iter()
        .filter(|metric| !metric.passed)
        .map(|metric| metric.metric_id.clone())
        .collect::<Vec<_>>();
    if summary.blocking_metric_ids != expected_blocking_metric_ids {
        return Err(
            "Temporal/routing promotion gate blocking_metric_ids must match the failing metrics."
                .to_owned(),
        );
    }
    if summary.temporal_routing_gate_passed != summary.blocking_metric_ids.is_empty() {
        return Err(
            "Temporal/routing promotion gate pass/fail flag must match the metric results."
                .to_owned(),
        );
    }
    if summary.summary.trim().is_empty() {
        return Err(
            "Temporal/routing promotion gate must include a human-readable summary.".to_owned(),
        );
    }
    let expected_decision = match (
        summary.npv_improves_over_reference,
        summary.temporal_routing_gate_passed,
    ) {
        (true, true) => "eligible-for-promotion",
        (true, false) => "blocked-by-temporal-routing",
        (false, true) => "blocked-by-npv",
        (false, false) => "blocked-by-npv-and-temporal-routing",
    };
    if summary.promotion_decision != expected_decision {
        return Err(format!(
            "Temporal/routing promotion gate decision must be `{expected_decision}`, received `{}`.",
            summary.promotion_decision
        ));
    }
    Ok(())
}

fn build_max_metric(
    metric_id: &str,
    candidate_value: f64,
    reference_value: Option<f64>,
    compared_value: f64,
    threshold_value: f64,
    base_summary: String,
) -> TemporalRoutingPromotionGateMetric {
    let passed = compared_value <= threshold_value + 1.0e-9;
    TemporalRoutingPromotionGateMetric {
        metric_id: metric_id.to_owned(),
        candidate_value,
        reference_value,
        compared_value,
        threshold_value,
        threshold_relation: "max".to_owned(),
        passed,
        summary: format!(
            "{base_summary} Threshold: value <= {threshold_value:.3}; status = {}.",
            if passed { "pass" } else { "fail" }
        ),
    }
}

fn build_min_metric(
    metric_id: &str,
    candidate_value: f64,
    reference_value: Option<f64>,
    compared_value: f64,
    threshold_value: f64,
    base_summary: String,
) -> TemporalRoutingPromotionGateMetric {
    let passed = compared_value + 1.0e-9 >= threshold_value;
    TemporalRoutingPromotionGateMetric {
        metric_id: metric_id.to_owned(),
        candidate_value,
        reference_value,
        compared_value,
        threshold_value,
        threshold_relation: "min".to_owned(),
        passed,
        summary: format!(
            "{base_summary} Threshold: value >= {threshold_value:.3}; status = {}.",
            if passed { "pass" } else { "fail" }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_temporal_routing_promotion_gate_summary,
        validate_temporal_routing_promotion_gate_summary,
    };

    #[test]
    fn promotion_gate_blocks_npv_only_improvement_when_temporal_metrics_fail() {
        let summary =
            build_temporal_routing_promotion_gate_summary(905.0, 900.0, 10, 14, 1.75, 2_400, 0.08);

        validate_temporal_routing_promotion_gate_summary(&summary)
            .expect("promotion gate should validate");
        assert!(summary.npv_improves_over_reference);
        assert!(!summary.temporal_routing_gate_passed);
        assert_eq!(summary.promotion_decision, "blocked-by-temporal-routing");
        assert_eq!(
            summary.blocking_metric_ids,
            vec![
                "used_period_count".to_owned(),
                "mean_absolute_period_delta".to_owned(),
                "earlier_than_reference_count".to_owned(),
                "period_destination_similarity".to_owned(),
            ]
        );
        assert!(summary.summary.contains("promotion remains blocked"));
    }

    #[test]
    fn promotion_gate_allows_promotion_when_npv_and_alignment_hold() {
        let summary =
            build_temporal_routing_promotion_gate_summary(905.0, 900.0, 13, 14, 0.5, 120, 0.42);

        validate_temporal_routing_promotion_gate_summary(&summary)
            .expect("promotion gate should validate");
        assert!(summary.npv_improves_over_reference);
        assert!(summary.temporal_routing_gate_passed);
        assert_eq!(summary.promotion_decision, "eligible-for-promotion");
    }
}
