use std::collections::BTreeSet;

use serde::Serialize;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparabilityGapSource {
    InputProtocol,
    AggregationFormulation,
    BibliographicFormulation,
    RelaxationModel,
    BaselineEvaluation,
    RoundingSearch,
    InstanceVariant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComparabilityGapSummary {
    pub gap_id: String,
    pub gap_source: ComparabilityGapSource,
    pub summary: String,
    pub evidence_fields: Vec<String>,
}

pub fn derive_comparability_gaps(contract: &[ComparabilityGapSummary]) -> Vec<String> {
    contract.iter().map(|gap| gap.summary.clone()).collect()
}

pub fn validate_comparability_gap_contract_consistency(
    contract: &[ComparabilityGapSummary],
    comparability_gaps: &[String],
    scope_label: &str,
) -> Result<(), String> {
    if contract.len() != comparability_gaps.len() {
        return Err(format!(
            "{scope_label} comparability gap contract has {} entries but comparability_gaps has {}.",
            contract.len(),
            comparability_gaps.len()
        ));
    }
    let mut gap_ids = BTreeSet::new();
    for (index, (entry, comparability_gap)) in
        contract.iter().zip(comparability_gaps.iter()).enumerate()
    {
        if entry.gap_id.trim().is_empty() {
            return Err(format!(
                "{scope_label} comparability gap contract entry #{index} is missing `gap_id`."
            ));
        }
        if !gap_ids.insert(entry.gap_id.as_str()) {
            return Err(format!(
                "{scope_label} comparability gap contract repeats gap_id `{}`.",
                entry.gap_id
            ));
        }
        if entry.summary.trim().is_empty() {
            return Err(format!(
                "{scope_label} comparability gap contract entry `{}` is missing summary text.",
                entry.gap_id
            ));
        }
        if entry.evidence_fields.is_empty() {
            return Err(format!(
                "{scope_label} comparability gap contract entry `{}` must surface at least one evidence field.",
                entry.gap_id
            ));
        }
        if entry.summary != *comparability_gap {
            return Err(format!(
                "{scope_label} comparability gap contract entry `{}` drifted from comparability_gaps at the same position.",
                entry.gap_id
            ));
        }
    }
    Ok(())
}
