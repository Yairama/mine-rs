use std::collections::BTreeSet;

use mine_core::{Coordinate3D, MineError};
use serde::{Deserialize, Serialize};

use crate::declustering::SpatialSample;

/// Elipsoide anisotrópico explícito para búsqueda espacial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchAnisotropy {
    major_range: f64,
    semi_major_range: f64,
    minor_range: f64,
    azimuth_degrees: f64,
    dip_degrees: f64,
    roll_degrees: f64,
}

impl SearchAnisotropy {
    /// Construye una anisotropía validando rangos y orientación.
    pub fn new(
        major_range: f64,
        semi_major_range: f64,
        minor_range: f64,
        azimuth_degrees: f64,
        dip_degrees: f64,
        roll_degrees: f64,
    ) -> Result<Self, MineError> {
        validate_positive_finite("major_range", major_range)?;
        validate_positive_finite("semi_major_range", semi_major_range)?;
        validate_positive_finite("minor_range", minor_range)?;
        if major_range < semi_major_range || semi_major_range < minor_range {
            return Err(MineError::validation(
                "search anisotropy ranges must satisfy major_range >= semi_major_range >= minor_range",
            ));
        }
        validate_finite("azimuth_degrees", azimuth_degrees)?;
        validate_finite("dip_degrees", dip_degrees)?;
        validate_finite("roll_degrees", roll_degrees)?;

        Ok(Self {
            major_range,
            semi_major_range,
            minor_range,
            azimuth_degrees,
            dip_degrees,
            roll_degrees,
        })
    }

    /// Radio mayor del elipsoide.
    #[must_use]
    pub const fn major_range(&self) -> f64 {
        self.major_range
    }

    /// Radio intermedio del elipsoide.
    #[must_use]
    pub const fn semi_major_range(&self) -> f64 {
        self.semi_major_range
    }

    /// Radio menor del elipsoide.
    #[must_use]
    pub const fn minor_range(&self) -> f64 {
        self.minor_range
    }

    /// Rotación alrededor de `z` antes de aplicar `dip` y `roll`.
    #[must_use]
    pub const fn azimuth_degrees(&self) -> f64 {
        self.azimuth_degrees
    }

    /// Rotación alrededor de `y` luego del azimuth.
    #[must_use]
    pub const fn dip_degrees(&self) -> f64 {
        self.dip_degrees
    }

    /// Rotación alrededor de `x` luego de azimuth y dip.
    #[must_use]
    pub const fn roll_degrees(&self) -> f64 {
        self.roll_degrees
    }

    fn anisotropic_distance(&self, target: Coordinate3D, sample: Coordinate3D) -> f64 {
        let dx = sample.x() - target.x();
        let dy = sample.y() - target.y();
        let dz = sample.z() - target.z();

        let azimuth = (-self.azimuth_degrees).to_radians();
        let dip = (-self.dip_degrees).to_radians();
        let roll = (-self.roll_degrees).to_radians();

        let (x1, y1, z1) = rotate_z(dx, dy, dz, azimuth);
        let (x2, y2, z2) = rotate_y(x1, y1, z1, dip);
        let (x3, y3, z3) = rotate_x(x2, y2, z2, roll);

        ((x3 / self.major_range).powi(2)
            + (y3 / self.semi_major_range).powi(2)
            + (z3 / self.minor_range).powi(2))
        .sqrt()
    }
}

/// Límites explícitos de muestras para un neighborhood o pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleCountLimits {
    min_samples: usize,
    max_samples: usize,
}

impl SampleCountLimits {
    /// Construye límites validados para la selección.
    pub fn new(min_samples: usize, max_samples: usize) -> Result<Self, MineError> {
        if min_samples == 0 {
            return Err(MineError::invalid_parameter(
                "min_samples",
                "sample count minimum must be greater than zero",
            ));
        }
        if max_samples == 0 {
            return Err(MineError::invalid_parameter(
                "max_samples",
                "sample count maximum must be greater than zero",
            ));
        }
        if min_samples > max_samples {
            return Err(MineError::validation(
                "sample count limits require min_samples <= max_samples",
            ));
        }

        Ok(Self {
            min_samples,
            max_samples,
        })
    }

    /// Mínimo requerido para aceptar una selección.
    #[must_use]
    pub const fn min_samples(&self) -> usize {
        self.min_samples
    }

    /// Máximo permitido luego del truncamiento por distancia.
    #[must_use]
    pub const fn max_samples(&self) -> usize {
        self.max_samples
    }
}

/// Filtro de búsqueda reutilizable por estimadores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchNeighborhood {
    /// Geometría anisotrópica de búsqueda.
    pub anisotropy: SearchAnisotropy,
    /// Dominios permitidos; `None` indica selección global.
    pub allowed_domains: Option<Vec<String>>,
}

impl SearchNeighborhood {
    /// Construye un neighborhood validando dominios explícitos.
    pub fn new(
        anisotropy: SearchAnisotropy,
        allowed_domains: Option<Vec<String>>,
    ) -> Result<Self, MineError> {
        if let Some(allowed_domains) = &allowed_domains {
            if allowed_domains.is_empty() {
                return Err(MineError::invalid_parameter(
                    "allowed_domains",
                    "search neighborhood allowed_domains must not be empty when provided",
                ));
            }

            let mut seen = BTreeSet::new();
            for domain in allowed_domains {
                if domain.trim().is_empty() {
                    return Err(MineError::invalid_parameter(
                        "allowed_domains",
                        "search neighborhood domains must not be empty",
                    ));
                }
                if !seen.insert(domain.clone()) {
                    return Err(MineError::validation(
                        "search neighborhood allowed_domains must not contain duplicates",
                    ));
                }
            }
        }

        Ok(Self {
            anisotropy,
            allowed_domains,
        })
    }

    fn domain_matches(&self, sample: &SpatialSample) -> bool {
        match &self.allowed_domains {
            None => true,
            Some(allowed_domains) => sample
                .domain
                .as_ref()
                .is_some_and(|domain| allowed_domains.iter().any(|allowed| allowed == domain)),
        }
    }
}

/// Pass de estimación ordenado por prioridad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimationPass {
    /// Identificador estable del pass.
    pub pass_id: String,
    /// Neighborhood aplicado en este pass.
    pub neighborhood: SearchNeighborhood,
    /// Límites min/max de muestras del pass.
    pub sample_limits: SampleCountLimits,
}

impl EstimationPass {
    /// Construye un pass validando el identificador.
    pub fn new(
        pass_id: impl Into<String>,
        neighborhood: SearchNeighborhood,
        sample_limits: SampleCountLimits,
    ) -> Result<Self, MineError> {
        let pass_id = pass_id.into();
        if pass_id.trim().is_empty() {
            return Err(MineError::invalid_parameter(
                "pass_id",
                "estimation pass id must not be empty",
            ));
        }

        Ok(Self {
            pass_id,
            neighborhood,
            sample_limits,
        })
    }
}

/// Sample seleccionado dentro de un neighborhood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborhoodSample {
    /// Índice del sample dentro de la entrada original.
    pub sample_index: usize,
    /// Identificador estable del sample.
    pub sample_id: String,
    /// Dominio del sample, cuando existe.
    pub domain: Option<String>,
    /// Distancia euclidiana al target.
    pub euclidean_distance: f64,
    /// Distancia anisotrópica normalizada del elipsoide.
    pub anisotropic_distance: f64,
}

/// Resultado serializable de una búsqueda en un neighborhood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborhoodSelection {
    /// Cantidad total de candidatos antes del truncamiento.
    pub candidate_count: usize,
    /// Cantidad de samples devueltos.
    pub selected_count: usize,
    /// Indica si la selección cumple el mínimo requerido.
    pub satisfies_minimum: bool,
    /// Indica si hubo truncamiento por `max_samples`.
    pub truncated: bool,
    /// Samples seleccionados ordenados de forma reproducible.
    pub samples: Vec<NeighborhoodSample>,
}

/// Resultado de evaluar un pass individual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimationPassEvaluation {
    /// Identificador del pass evaluado.
    pub pass_id: String,
    /// Cantidad total de candidatos encontrados.
    pub candidate_count: usize,
    /// Cantidad de samples finalmente seleccionados.
    pub selected_count: usize,
    /// Indica si el pass satisface el mínimo configurado.
    pub satisfies_minimum: bool,
    /// Indica si hubo truncamiento por máximo.
    pub truncated: bool,
}

/// Resultado de resolver una secuencia de estimation passes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimationPassSelection {
    /// Identificador del primer pass aceptado, si existe.
    pub selected_pass_id: Option<String>,
    /// Evaluaciones realizadas en orden de prioridad.
    pub evaluations: Vec<EstimationPassEvaluation>,
    /// Samples del pass seleccionado; vacío si ningún pass califica.
    pub samples: Vec<NeighborhoodSample>,
}

/// Selecciona samples dentro de un neighborhood y aplica truncamiento por distancia.
pub fn select_samples_in_neighborhood(
    target: Coordinate3D,
    samples: &[SpatialSample],
    neighborhood: &SearchNeighborhood,
    sample_limits: &SampleCountLimits,
) -> Result<NeighborhoodSelection, MineError> {
    validate_unique_sample_ids(samples)?;

    let mut selected = samples
        .iter()
        .enumerate()
        .filter_map(|(sample_index, sample)| {
            if !neighborhood.domain_matches(sample) {
                return None;
            }

            let anisotropic_distance = neighborhood
                .anisotropy
                .anisotropic_distance(target, sample.location);
            if anisotropic_distance > 1.0 + 1.0e-12 {
                return None;
            }

            let euclidean_distance = euclidean_distance(target, sample.location);
            Some(NeighborhoodSample {
                sample_index,
                sample_id: sample.sample_id.clone(),
                domain: sample.domain.clone(),
                euclidean_distance,
                anisotropic_distance,
            })
        })
        .collect::<Vec<_>>();

    selected.sort_by(|left, right| {
        left.anisotropic_distance
            .total_cmp(&right.anisotropic_distance)
            .then_with(|| left.euclidean_distance.total_cmp(&right.euclidean_distance))
            .then_with(|| left.sample_id.cmp(&right.sample_id))
            .then_with(|| left.sample_index.cmp(&right.sample_index))
    });

    let candidate_count = selected.len();
    if selected.len() > sample_limits.max_samples() {
        selected.truncate(sample_limits.max_samples());
    }
    let selected_count = selected.len();

    Ok(NeighborhoodSelection {
        candidate_count,
        selected_count,
        satisfies_minimum: selected_count >= sample_limits.min_samples(),
        truncated: candidate_count > selected_count,
        samples: selected,
    })
}

/// Evalúa passes en orden y selecciona el primero que cumpla el mínimo configurado.
pub fn select_samples_by_estimation_passes(
    target: Coordinate3D,
    samples: &[SpatialSample],
    passes: &[EstimationPass],
) -> Result<EstimationPassSelection, MineError> {
    if passes.is_empty() {
        return Err(MineError::invalid_parameter(
            "passes",
            "estimation pass selection requires at least one pass",
        ));
    }
    validate_unique_pass_ids(passes)?;

    let mut evaluations = Vec::with_capacity(passes.len());
    for pass in passes {
        let selection = select_samples_in_neighborhood(
            target,
            samples,
            &pass.neighborhood,
            &pass.sample_limits,
        )?;
        evaluations.push(EstimationPassEvaluation {
            pass_id: pass.pass_id.clone(),
            candidate_count: selection.candidate_count,
            selected_count: selection.selected_count,
            satisfies_minimum: selection.satisfies_minimum,
            truncated: selection.truncated,
        });

        if selection.satisfies_minimum {
            return Ok(EstimationPassSelection {
                selected_pass_id: Some(pass.pass_id.clone()),
                evaluations,
                samples: selection.samples,
            });
        }
    }

    Ok(EstimationPassSelection {
        selected_pass_id: None,
        evaluations,
        samples: Vec::new(),
    })
}

fn validate_positive_finite(parameter: &'static str, value: f64) -> Result<(), MineError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(MineError::invalid_parameter(
            parameter,
            "must be finite and greater than zero",
        ));
    }
    Ok(())
}

fn validate_finite(parameter: &'static str, value: f64) -> Result<(), MineError> {
    if !value.is_finite() {
        return Err(MineError::invalid_parameter(parameter, "must be finite"));
    }
    Ok(())
}

fn validate_unique_sample_ids(samples: &[SpatialSample]) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();
    for sample in samples {
        if !seen.insert(sample.sample_id.clone()) {
            return Err(MineError::validation(
                "search neighborhood input samples must not contain duplicate sample ids",
            ));
        }
    }
    Ok(())
}

fn validate_unique_pass_ids(passes: &[EstimationPass]) -> Result<(), MineError> {
    let mut seen = BTreeSet::new();
    for pass in passes {
        if !seen.insert(pass.pass_id.clone()) {
            return Err(MineError::validation(
                "estimation passes must not contain duplicate pass ids",
            ));
        }
    }
    Ok(())
}

fn euclidean_distance(left: Coordinate3D, right: Coordinate3D) -> f64 {
    let dx = right.x() - left.x();
    let dy = right.y() - left.y();
    let dz = right.z() - left.z();
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn rotate_x(x: f64, y: f64, z: f64, angle_radians: f64) -> (f64, f64, f64) {
    let cosine = angle_radians.cos();
    let sine = angle_radians.sin();
    (x, cosine * y - sine * z, sine * y + cosine * z)
}

fn rotate_y(x: f64, y: f64, z: f64, angle_radians: f64) -> (f64, f64, f64) {
    let cosine = angle_radians.cos();
    let sine = angle_radians.sin();
    (cosine * x + sine * z, y, -sine * x + cosine * z)
}

fn rotate_z(x: f64, y: f64, z: f64, angle_radians: f64) -> (f64, f64, f64) {
    let cosine = angle_radians.cos();
    let sine = angle_radians.sin();
    (cosine * x - sine * y, sine * x + cosine * y, z)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_core::{ColumnId, Coordinate3D};

    use super::*;

    fn sample(point_id: &str, x: f64, y: f64, z: f64, domain: Option<&str>) -> SpatialSample {
        SpatialSample::new(
            point_id,
            Coordinate3D::new(x, y, z).expect("coordinate should be valid"),
            domain.map(str::to_owned),
            BTreeMap::from([(
                ColumnId::new("cu").expect("column id should be valid"),
                x + y + z,
            )]),
        )
        .expect("sample should be valid")
    }

    #[test]
    fn anisotropic_neighborhood_filters_points_by_orientation() {
        let samples = vec![
            sample("x-hit", 2.0, 0.0, 0.0, Some("ore")),
            sample("y-miss", 0.0, 2.0, 0.0, Some("ore")),
        ];
        let neighborhood = SearchNeighborhood::new(
            SearchAnisotropy::new(2.0, 0.5, 0.5, 0.0, 0.0, 0.0)
                .expect("anisotropy should be valid"),
            Some(vec!["ore".to_owned()]),
        )
        .expect("neighborhood should be valid");
        let limits = SampleCountLimits::new(1, 4).expect("limits should be valid");

        let selection = select_samples_in_neighborhood(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &samples,
            &neighborhood,
            &limits,
        )
        .expect("selection should work");

        assert_eq!(selection.candidate_count, 1);
        assert_eq!(selection.selected_count, 1);
        assert!(selection.satisfies_minimum);
        assert_eq!(selection.samples[0].sample_id, "x-hit");
    }

    #[test]
    fn neighborhood_selection_orders_by_distance_and_truncates() {
        let samples = vec![
            sample("far", 2.0, 0.0, 0.0, Some("ore")),
            sample("near", 0.25, 0.0, 0.0, Some("ore")),
            sample("mid", 1.0, 0.0, 0.0, Some("ore")),
        ];
        let neighborhood = SearchNeighborhood::new(
            SearchAnisotropy::new(3.0, 3.0, 3.0, 0.0, 0.0, 0.0)
                .expect("anisotropy should be valid"),
            None,
        )
        .expect("neighborhood should be valid");
        let limits = SampleCountLimits::new(1, 2).expect("limits should be valid");

        let selection = select_samples_in_neighborhood(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &samples,
            &neighborhood,
            &limits,
        )
        .expect("selection should work");

        assert_eq!(selection.candidate_count, 3);
        assert_eq!(selection.selected_count, 2);
        assert!(selection.truncated);
        assert_eq!(selection.samples[0].sample_id, "near");
        assert_eq!(selection.samples[1].sample_id, "mid");
    }

    #[test]
    fn estimation_passes_select_first_priority_that_meets_minimum() {
        let samples = vec![
            sample("p1", 0.5, 0.0, 0.0, Some("ore")),
            sample("p2", 1.5, 0.0, 0.0, Some("ore")),
            sample("p3", 2.5, 0.0, 0.0, Some("ore")),
        ];
        let target = Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid");
        let pass_1 = EstimationPass::new(
            "tight",
            SearchNeighborhood::new(
                SearchAnisotropy::new(1.0, 1.0, 1.0, 0.0, 0.0, 0.0)
                    .expect("anisotropy should be valid"),
                Some(vec!["ore".to_owned()]),
            )
            .expect("neighborhood should be valid"),
            SampleCountLimits::new(2, 3).expect("limits should be valid"),
        )
        .expect("pass should be valid");
        let pass_2 = EstimationPass::new(
            "broad",
            SearchNeighborhood::new(
                SearchAnisotropy::new(2.0, 2.0, 2.0, 0.0, 0.0, 0.0)
                    .expect("anisotropy should be valid"),
                Some(vec!["ore".to_owned()]),
            )
            .expect("neighborhood should be valid"),
            SampleCountLimits::new(2, 3).expect("limits should be valid"),
        )
        .expect("pass should be valid");
        let pass_3 = EstimationPass::new(
            "never-reached",
            SearchNeighborhood::new(
                SearchAnisotropy::new(3.0, 3.0, 3.0, 0.0, 0.0, 0.0)
                    .expect("anisotropy should be valid"),
                Some(vec!["ore".to_owned()]),
            )
            .expect("neighborhood should be valid"),
            SampleCountLimits::new(1, 3).expect("limits should be valid"),
        )
        .expect("pass should be valid");

        let selection =
            select_samples_by_estimation_passes(target, &samples, &[pass_1, pass_2, pass_3])
                .expect("pass selection should work");

        assert_eq!(selection.selected_pass_id, Some("broad".to_owned()));
        assert_eq!(selection.evaluations.len(), 2);
        assert!(!selection.evaluations[0].satisfies_minimum);
        assert!(selection.evaluations[1].satisfies_minimum);
        assert_eq!(selection.samples.len(), 2);
        assert_eq!(selection.samples[0].sample_id, "p1");
        assert_eq!(selection.samples[1].sample_id, "p2");
    }

    #[test]
    fn estimation_passes_require_unique_ids() {
        let pass = EstimationPass::new(
            "duplicate",
            SearchNeighborhood::new(
                SearchAnisotropy::new(1.0, 1.0, 1.0, 0.0, 0.0, 0.0)
                    .expect("anisotropy should be valid"),
                None,
            )
            .expect("neighborhood should be valid"),
            SampleCountLimits::new(1, 1).expect("limits should be valid"),
        )
        .expect("pass should be valid");

        let error = select_samples_by_estimation_passes(
            Coordinate3D::new(0.0, 0.0, 0.0).expect("coordinate should be valid"),
            &[sample("p1", 0.0, 0.0, 0.0, Some("ore"))],
            &[pass.clone(), pass],
        )
        .expect_err("selection should reject duplicate pass ids");

        assert_eq!(
            error,
            MineError::validation("estimation passes must not contain duplicate pass ids")
        );
    }
}
