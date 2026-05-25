use std::collections::{BTreeMap, BTreeSet};

use mine_core::{ColumnId, MineError};
use serde::{Deserialize, Serialize};

/// Política explícita para composites residuales más cortos que la longitud objetivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositeResidualPolicy {
    /// Conserva el composite residual aunque sea más corto que la longitud objetivo.
    Keep,
    /// Descarta el composite residual cuando queda por debajo de la longitud objetivo.
    Drop,
}

/// Configuración mínima para compositing determinista de intervalos 1D.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositingOptions {
    target_length: f64,
    residual_policy: CompositeResidualPolicy,
    split_on_domain_change: bool,
}

impl CompositingOptions {
    /// Construye opciones validadas para el engine de compositing.
    pub fn new(
        target_length: f64,
        residual_policy: CompositeResidualPolicy,
        split_on_domain_change: bool,
    ) -> Result<Self, MineError> {
        if !target_length.is_finite() || target_length <= 0.0 {
            return Err(MineError::invalid_parameter(
                "target_length",
                "composite target length must be finite and greater than zero",
            ));
        }

        Ok(Self {
            target_length,
            residual_policy,
            split_on_domain_change,
        })
    }

    /// Longitud objetivo del composite.
    #[must_use]
    pub const fn target_length(&self) -> f64 {
        self.target_length
    }

    /// Política de residual aplicada.
    #[must_use]
    pub const fn residual_policy(&self) -> CompositeResidualPolicy {
        self.residual_policy
    }

    /// Indica si un cambio de dominio debe cerrar el composite actual.
    #[must_use]
    pub const fn split_on_domain_change(&self) -> bool {
        self.split_on_domain_change
    }
}

/// Intervalo elemental de entrada para compositing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntervalSample {
    /// Identificador estable del intervalo fuente.
    pub sample_id: String,
    /// Inicio del intervalo.
    pub from: f64,
    /// Fin del intervalo.
    pub to: f64,
    /// Dominio opcional del intervalo.
    pub domain: Option<String>,
    /// Valores numéricos asociados al intervalo.
    pub values: BTreeMap<ColumnId, f64>,
}

impl IntervalSample {
    /// Construye un intervalo validando soporte, dominio y valores.
    pub fn new(
        sample_id: impl Into<String>,
        from: f64,
        to: f64,
        domain: Option<String>,
        values: BTreeMap<ColumnId, f64>,
    ) -> Result<Self, MineError> {
        let sample_id = sample_id.into();
        if sample_id.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "sample_id",
                "interval sample id must not be empty",
            ));
        }
        if !from.is_finite() || !to.is_finite() {
            return Err(MineError::invalid_parameter(
                "interval",
                "interval sample bounds must be finite",
            ));
        }
        if to <= from {
            return Err(MineError::invalid_parameter(
                "interval",
                "interval sample end must be greater than start",
            ));
        }
        if let Some(domain) = &domain
            && domain.trim().is_empty()
        {
            return Err(MineError::invalid_parameter(
                "domain",
                "interval sample domain must not be empty when provided",
            ));
        }
        for (column_id, value) in &values {
            if !value.is_finite() {
                return Err(MineError::invalid_parameter(
                    "values",
                    format!("interval sample value for `{column_id}` must be finite"),
                ));
            }
        }

        Ok(Self {
            sample_id,
            from,
            to,
            domain,
            values,
        })
    }
}

/// Contribución parcial de un intervalo fuente a un composite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeContribution {
    /// Identificador del intervalo fuente.
    pub sample_id: String,
    /// Inicio del tramo aportado.
    pub from: f64,
    /// Fin del tramo aportado.
    pub to: f64,
    /// Longitud efectiva aportada al composite.
    pub contribution_length: f64,
    /// Peso relativo dentro del composite.
    pub weight: f64,
}

/// Composite resultante con trazabilidad hacia los intervalos fuente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeInterval {
    /// Identificador estable del composite.
    pub composite_id: String,
    /// Inicio del composite.
    pub from: f64,
    /// Fin del composite.
    pub to: f64,
    /// Longitud efectiva del composite.
    pub length: f64,
    /// Dominio único cuando el composite no mezcla dominios; `None` si mezcla o no se provee.
    pub domain: Option<String>,
    /// Valores compuestos por promedio ponderado en longitud.
    pub values: BTreeMap<ColumnId, f64>,
    /// Trazabilidad a intervalos fuente.
    pub contributions: Vec<CompositeContribution>,
}

/// Máscara explícita de dominios permitidos para workflows de compositing y estimación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainMask {
    allowed_domains: Vec<String>,
    include_untagged: bool,
}

impl DomainMask {
    /// Construye una máscara validando dominios explícitos y política sobre intervalos sin dominio.
    pub fn new(allowed_domains: Vec<String>, include_untagged: bool) -> Result<Self, MineError> {
        let allowed_domains = allowed_domains
            .into_iter()
            .map(|domain| {
                if domain.trim().is_empty() {
                    Err(MineError::invalid_parameter(
                        "allowed_domains",
                        "domain mask values must not be empty",
                    ))
                } else {
                    Ok(domain)
                }
            })
            .collect::<Result<BTreeSet<_>, _>>()?
            .into_iter()
            .collect::<Vec<_>>();

        if allowed_domains.is_empty() && !include_untagged {
            return Err(MineError::invalid_parameter(
                "allowed_domains",
                "domain mask must include at least one allowed domain or enable untagged intervals",
            ));
        }

        Ok(Self {
            allowed_domains,
            include_untagged,
        })
    }

    /// Dominios aceptados por la máscara.
    #[must_use]
    pub fn allowed_domains(&self) -> &[String] {
        &self.allowed_domains
    }

    /// Indica si los intervalos sin dominio son aceptados.
    #[must_use]
    pub const fn include_untagged(&self) -> bool {
        self.include_untagged
    }
}

/// Resultado serializable del filtrado por máscara de dominio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainFilterReport {
    /// Máscara usada para el filtrado.
    pub mask: DomainMask,
    /// Intervalos aceptados por la máscara.
    pub selected_samples: Vec<IntervalSample>,
    /// Intervalos excluidos por dominio fuera de máscara.
    pub excluded_sample_ids: Vec<String>,
    /// Intervalos sin dominio explícito detectados durante el filtrado.
    pub untagged_sample_ids: Vec<String>,
}

/// Código estable de issue detectado en la auditoría de dominios de composites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositeDomainAuditIssueCode {
    /// El composite mezcla más de un dominio fuente.
    MixedDomains,
    /// El composite contiene contribuciones de dominios fuera de máscara.
    OutOfMaskDomain,
    /// El composite contiene contribuciones sin dominio explícito.
    UntaggedContribution,
}

/// Issue estructurado detectado al auditar composites respecto de una máscara de dominio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeDomainAuditIssue {
    /// Composite afectado.
    pub composite_id: String,
    /// Código estable del issue.
    pub code: CompositeDomainAuditIssueCode,
    /// Samples fuente involucrados.
    pub sample_ids: Vec<String>,
    /// Dominios observados en la contribución auditada.
    pub domains: Vec<String>,
}

/// Reporte serializable de consistencia entre composites, samples y máscara de dominio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeDomainAuditReport {
    /// Máscara auditada.
    pub mask: DomainMask,
    /// Issues detectados en los composites revisados.
    pub issues: Vec<CompositeDomainAuditIssue>,
}

/// Construye composites 1D con weighting por longitud y trazabilidad explícita.
pub fn composite_intervals(
    samples: &[IntervalSample],
    options: &CompositingOptions,
) -> Result<Vec<CompositeInterval>, MineError> {
    if samples.is_empty() {
        return Err(MineError::invalid_parameter(
            "samples",
            "compositing requires at least one interval sample",
        ));
    }

    let mut samples = samples.to_vec();
    samples.sort_by(|left, right| {
        left.from
            .total_cmp(&right.from)
            .then(left.to.total_cmp(&right.to))
    });
    validate_samples(&samples)?;

    let value_keys = samples[0].values.keys().cloned().collect::<Vec<_>>();
    for sample in &samples[1..] {
        let sample_keys = sample.values.keys().cloned().collect::<Vec<_>>();
        if sample_keys != value_keys {
            return Err(MineError::schema(
                "all interval samples must expose the same numeric value columns",
            ));
        }
    }

    let mut composites = Vec::new();
    let mut current = WorkingComposite::new();
    let mut previous_to = None::<f64>;

    for sample in samples {
        if let Some(previous_to) = previous_to
            && sample.from > previous_to
            && current.has_content()
        {
            finalize_composite(&mut composites, &mut current, options)?;
        }

        if options.split_on_domain_change()
            && current.has_content()
            && current.domain_changed(&sample.domain)
        {
            finalize_composite(&mut composites, &mut current, options)?;
        }

        let mut cursor = sample.from;
        while cursor < sample.to {
            let take_length = (options.target_length() - current.length).min(sample.to - cursor);
            current.push(&sample, cursor, cursor + take_length);
            cursor += take_length;

            if current.reached_target(options.target_length()) {
                finalize_composite(&mut composites, &mut current, options)?;
            }
        }

        previous_to = Some(sample.to);
    }

    if current.has_content() {
        finalize_composite(&mut composites, &mut current, options)?;
    }

    Ok(composites)
}

/// Filtra intervalos usando una máscara de dominio dura y reporta exclusiones explícitas.
#[must_use]
pub fn filter_interval_samples_by_domain_mask(
    samples: &[IntervalSample],
    mask: &DomainMask,
) -> DomainFilterReport {
    let mut selected_samples = Vec::new();
    let mut excluded_sample_ids = Vec::new();
    let mut untagged_sample_ids = Vec::new();

    for sample in samples {
        match &sample.domain {
            Some(domain) if mask.allowed_domains.contains(domain) => {
                selected_samples.push(sample.clone());
            }
            Some(_) => excluded_sample_ids.push(sample.sample_id.clone()),
            None => {
                untagged_sample_ids.push(sample.sample_id.clone());
                if mask.include_untagged() {
                    selected_samples.push(sample.clone());
                } else {
                    excluded_sample_ids.push(sample.sample_id.clone());
                }
            }
        }
    }

    DomainFilterReport {
        mask: mask.clone(),
        selected_samples,
        excluded_sample_ids,
        untagged_sample_ids,
    }
}

/// Audita composites respecto de una máscara de dominio y sus intervalos fuente.
pub fn audit_composite_domains(
    samples: &[IntervalSample],
    composites: &[CompositeInterval],
    mask: &DomainMask,
) -> Result<CompositeDomainAuditReport, MineError> {
    let mut sample_domains = BTreeMap::<String, Option<String>>::new();
    for sample in samples {
        if sample_domains
            .insert(sample.sample_id.clone(), sample.domain.clone())
            .is_some()
        {
            return Err(MineError::validation(format!(
                "duplicate interval sample id `{}` found while auditing composite domains",
                sample.sample_id
            )));
        }
    }

    let mut issues = Vec::new();
    for composite in composites {
        let mut sample_ids = Vec::new();
        let mut domains = BTreeSet::<String>::new();
        let mut out_of_mask_domains = BTreeSet::<String>::new();
        let mut has_untagged = false;

        for contribution in &composite.contributions {
            let Some(domain) = sample_domains.get(&contribution.sample_id) else {
                return Err(MineError::validation(format!(
                    "composite `{}` references unknown sample `{}`",
                    composite.composite_id, contribution.sample_id
                )));
            };

            sample_ids.push(contribution.sample_id.clone());
            match domain {
                Some(domain) => {
                    domains.insert(domain.clone());
                    if !mask.allowed_domains.contains(domain) {
                        out_of_mask_domains.insert(domain.clone());
                    }
                }
                None => has_untagged = true,
            }
        }

        if domains.len() > 1 {
            issues.push(CompositeDomainAuditIssue {
                composite_id: composite.composite_id.clone(),
                code: CompositeDomainAuditIssueCode::MixedDomains,
                sample_ids: sample_ids.clone(),
                domains: domains.iter().cloned().collect(),
            });
        }

        if !out_of_mask_domains.is_empty() {
            issues.push(CompositeDomainAuditIssue {
                composite_id: composite.composite_id.clone(),
                code: CompositeDomainAuditIssueCode::OutOfMaskDomain,
                sample_ids: sample_ids.clone(),
                domains: out_of_mask_domains.into_iter().collect(),
            });
        }

        if has_untagged && !mask.include_untagged() {
            issues.push(CompositeDomainAuditIssue {
                composite_id: composite.composite_id.clone(),
                code: CompositeDomainAuditIssueCode::UntaggedContribution,
                sample_ids,
                domains: Vec::new(),
            });
        }
    }

    Ok(CompositeDomainAuditReport {
        mask: mask.clone(),
        issues,
    })
}

fn validate_samples(samples: &[IntervalSample]) -> Result<(), MineError> {
    for window in samples.windows(2) {
        if window[1].from < window[0].to {
            return Err(MineError::validation(
                "interval samples must be non-overlapping after sorting by start coordinate",
            ));
        }
    }

    Ok(())
}

fn finalize_composite(
    composites: &mut Vec<CompositeInterval>,
    current: &mut WorkingComposite,
    options: &CompositingOptions,
) -> Result<(), MineError> {
    if !current.has_content() {
        return Ok(());
    }

    if current.length < options.target_length()
        && matches!(options.residual_policy(), CompositeResidualPolicy::Drop)
    {
        current.reset();
        return Ok(());
    }

    let mut values = BTreeMap::new();
    for (column_id, weighted_sum) in &current.weighted_sums {
        values.insert(column_id.clone(), weighted_sum / current.length);
    }

    let domain = (current.domains.len() == 1)
        .then(|| {
            current
                .domains
                .iter()
                .next()
                .expect("one domain should exist")
                .clone()
        })
        .flatten();
    let contributions = current
        .contributions
        .iter()
        .map(|contribution| CompositeContribution {
            sample_id: contribution.sample_id.clone(),
            from: contribution.from,
            to: contribution.to,
            contribution_length: contribution.length,
            weight: contribution.length / current.length,
        })
        .collect::<Vec<_>>();

    composites.push(CompositeInterval {
        composite_id: format!("composite-{:04}", composites.len() + 1),
        from: current.from.expect("composite start should exist"),
        to: current.to.expect("composite end should exist"),
        length: current.length,
        domain,
        values,
        contributions,
    });
    current.reset();
    Ok(())
}

#[derive(Debug, Default)]
struct WorkingComposite {
    from: Option<f64>,
    to: Option<f64>,
    length: f64,
    weighted_sums: BTreeMap<ColumnId, f64>,
    domains: BTreeSet<Option<String>>,
    contributions: Vec<WorkingContribution>,
}

impl WorkingComposite {
    fn new() -> Self {
        Self::default()
    }

    fn has_content(&self) -> bool {
        self.length > 0.0
    }

    fn domain_changed(&self, candidate: &Option<String>) -> bool {
        self.domains.len() == 1
            && self.domains.iter().next().expect("domain should exist") != candidate
    }

    fn reached_target(&self, target_length: f64) -> bool {
        self.length + 1e-12 >= target_length
    }

    fn push(&mut self, sample: &IntervalSample, from: f64, to: f64) {
        let length = to - from;
        if self.from.is_none() {
            self.from = Some(from);
        }
        self.to = Some(to);
        self.length += length;
        self.domains.insert(sample.domain.clone());
        self.contributions.push(WorkingContribution {
            sample_id: sample.sample_id.clone(),
            from,
            to,
            length,
        });
        for (column_id, value) in &sample.values {
            *self.weighted_sums.entry(column_id.clone()).or_insert(0.0) += value * length;
        }
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug)]
struct WorkingContribution {
    sample_id: String,
    from: f64,
    to: f64,
    length: f64,
}
