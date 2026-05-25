//! Plantillas de talud variables para generación de precedencias geotécnicas.
//!
//! Permite definir ángulos de talud distintos por sector azimutal y derivar
//! automáticamente los offsets de precedencia adecuados desde las dimensiones
//! del modelo de bloques.
//!
//! # Relación entre talud y precedencia
//!
//! Para un bloque en posición `(i, j, k)`, un bloque predecesor en
//! `(i + di, j + dj, k + dk)` es válido si:
//!
//! ```text
//! horizontal_distance / (dk * block_height) <= 1 / tan(slope_angle)
//! ```
//!
//! donde `horizontal_distance = sqrt((di*dx)^2 + (dj*dy)^2)`.
//!
//! La dirección azimutal del offset se usa para seleccionar el ángulo correcto
//! dentro de la plantilla de talud variable.

use std::f64::consts::PI;

use mine_core::{BlockDimensions, MineError};
use serde::{Deserialize, Serialize};

use crate::precedence::{BlockPrecedenceTemplate, PrecedenceOffset};

// ── Sector azimutal ───────────────────────────────────────────────────────────

/// Regla de ángulo de talud asociada a un sector azimutal explícito.
///
/// El sector cubre el rango `[azimuth_from_degrees, azimuth_to_degrees)`.
/// Los ángulos azimutales se miden en grados desde el norte (0°), girando
/// hacia el este (90°).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlopeAngleRule {
    /// Inicio del sector azimutal (inclusive), en grados [0, 360).
    azimuth_from_degrees: f64,
    /// Fin del sector azimutal (exclusive), en grados (0, 360].
    azimuth_to_degrees: f64,
    /// Ángulo de talud en grados (0, 90).
    slope_angle_degrees: f64,
}

impl SlopeAngleRule {
    /// Construye una regla de talud para un sector azimutal explícito.
    ///
    /// # Errores
    ///
    /// Retorna error si:
    /// - `azimuth_from_degrees` no está en [0, 360)
    /// - `azimuth_to_degrees` no está en (0, 360]
    /// - `azimuth_from_degrees >= azimuth_to_degrees`
    /// - `slope_angle_degrees` no está en (0, 90)
    pub fn new(
        azimuth_from_degrees: f64,
        azimuth_to_degrees: f64,
        slope_angle_degrees: f64,
    ) -> Result<Self, MineError> {
        if !azimuth_from_degrees.is_finite()
            || azimuth_from_degrees < 0.0
            || azimuth_from_degrees >= 360.0
        {
            return Err(MineError::invalid_parameter(
                "azimuth_from_degrees",
                "azimuth_from_degrees must be in [0, 360)",
            ));
        }
        if !azimuth_to_degrees.is_finite()
            || azimuth_to_degrees <= 0.0
            || azimuth_to_degrees > 360.0
        {
            return Err(MineError::invalid_parameter(
                "azimuth_to_degrees",
                "azimuth_to_degrees must be in (0, 360]",
            ));
        }
        if azimuth_from_degrees >= azimuth_to_degrees {
            return Err(MineError::invalid_parameter(
                "azimuth_from_degrees",
                "azimuth_from_degrees must be strictly less than azimuth_to_degrees",
            ));
        }
        if !slope_angle_degrees.is_finite()
            || slope_angle_degrees <= 0.0
            || slope_angle_degrees >= 90.0
        {
            return Err(MineError::invalid_parameter(
                "slope_angle_degrees",
                "slope_angle_degrees must be in (0, 90)",
            ));
        }

        Ok(Self {
            azimuth_from_degrees,
            azimuth_to_degrees,
            slope_angle_degrees,
        })
    }

    /// Inicio del sector azimutal (inclusive), en grados.
    #[must_use]
    pub const fn azimuth_from_degrees(&self) -> f64 {
        self.azimuth_from_degrees
    }

    /// Fin del sector azimutal (exclusive), en grados.
    #[must_use]
    pub const fn azimuth_to_degrees(&self) -> f64 {
        self.azimuth_to_degrees
    }

    /// Ángulo de talud en grados.
    #[must_use]
    pub const fn slope_angle_degrees(&self) -> f64 {
        self.slope_angle_degrees
    }

    /// Verifica si un azimuth cae dentro del sector.
    fn contains_azimuth(&self, azimuth: f64) -> bool {
        azimuth >= self.azimuth_from_degrees && azimuth < self.azimuth_to_degrees
    }
}

// ── Plantilla de talud variable ───────────────────────────────────────────────

/// Plantilla de talud variable con reglas por sector azimutal.
///
/// Cubre los 360° del círculo usando sectores explícitos más un ángulo
/// por defecto para los azimuts no cubiertos por ninguna regla.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableSlopeTemplate {
    rules: Vec<SlopeAngleRule>,
    default_slope_angle_degrees: f64,
    /// Extensión horizontal máxima (en bloques) a considerar en cada dirección.
    max_horizontal_reach_blocks: usize,
}

impl VariableSlopeTemplate {
    /// Construye una plantilla de talud variable.
    ///
    /// # Parámetros
    ///
    /// - `rules`: reglas de talud por sector azimutal (pueden cubrir subconjunto del círculo).
    /// - `default_slope_angle_degrees`: ángulo de talud por defecto para azimuts no cubiertos.
    /// - `max_horizontal_reach_blocks`: cuántos bloques de radio horizontal máximo se consideran.
    ///
    /// # Errores
    ///
    /// Retorna error si:
    /// - `default_slope_angle_degrees` no está en (0, 90)
    /// - `max_horizontal_reach_blocks` es 0
    pub fn new(
        rules: Vec<SlopeAngleRule>,
        default_slope_angle_degrees: f64,
        max_horizontal_reach_blocks: usize,
    ) -> Result<Self, MineError> {
        if !default_slope_angle_degrees.is_finite()
            || default_slope_angle_degrees <= 0.0
            || default_slope_angle_degrees >= 90.0
        {
            return Err(MineError::invalid_parameter(
                "default_slope_angle_degrees",
                "default_slope_angle_degrees must be in (0, 90)",
            ));
        }
        if max_horizontal_reach_blocks == 0 {
            return Err(MineError::invalid_parameter(
                "max_horizontal_reach_blocks",
                "max_horizontal_reach_blocks must be at least 1",
            ));
        }

        Ok(Self {
            rules,
            default_slope_angle_degrees,
            max_horizontal_reach_blocks,
        })
    }

    /// Construye una plantilla con talud uniforme (igual ángulo en todas las direcciones).
    ///
    /// Equivalente a una plantilla sin reglas sectoriales, donde el ángulo
    /// por defecto se aplica a todos los azimuts.
    pub fn uniform(
        slope_angle_degrees: f64,
        max_horizontal_reach_blocks: usize,
    ) -> Result<Self, MineError> {
        Self::new(vec![], slope_angle_degrees, max_horizontal_reach_blocks)
    }

    /// Retorna las reglas de talud sectoriales.
    #[must_use]
    pub fn rules(&self) -> &[SlopeAngleRule] {
        &self.rules
    }

    /// Retorna el ángulo de talud por defecto.
    #[must_use]
    pub const fn default_slope_angle_degrees(&self) -> f64 {
        self.default_slope_angle_degrees
    }

    /// Retorna la extensión horizontal máxima.
    #[must_use]
    pub const fn max_horizontal_reach_blocks(&self) -> usize {
        self.max_horizontal_reach_blocks
    }

    /// Resuelve el ángulo de talud para un azimut dado.
    ///
    /// Usa la primera regla cuyo sector cubra el azimut. Si ninguna lo cubre,
    /// usa el ángulo por defecto.
    #[must_use]
    pub fn slope_angle_for_azimuth(&self, azimuth_degrees: f64) -> f64 {
        let azimuth = normalize_azimuth(azimuth_degrees);
        for rule in &self.rules {
            if rule.contains_azimuth(azimuth) {
                return rule.slope_angle_degrees;
            }
        }
        self.default_slope_angle_degrees
    }
}

// ── Derivación de plantilla de precedencias ───────────────────────────────────

/// Deriva una `BlockPrecedenceTemplate` desde una `VariableSlopeTemplate` y
/// las dimensiones de bloque del modelo.
///
/// Para cada offset horizontal `(di, dj)` dentro del radio `max_horizontal_reach_blocks`,
/// determina el azimut, consulta el ángulo de talud, y calcula el `dk` mínimo
/// que satisface la restricción de talud. Incluye todos los offsets con `dk >= 1`
/// que son geometricamente válidos.
///
/// # Fórmula
///
/// ```text
/// horizontal_dist = sqrt((di * block_dx)^2 + (dj * block_dy)^2)
/// dk_min = ceil(horizontal_dist * tan(slope_angle) / block_dz)
/// ```
///
/// # Errores
///
/// Retorna error si no se genera ningún offset válido (reach demasiado pequeño o
/// slope_angle demasiado pequeño para generar offsets `dk >= 1`).
pub fn derive_precedence_template_from_slope(
    block_dims: &BlockDimensions,
    slope_template: &VariableSlopeTemplate,
) -> Result<BlockPrecedenceTemplate, MineError> {
    let dx = block_dims.dx();
    let dy = block_dims.dy();
    let dz = block_dims.dz();
    let max_reach = slope_template.max_horizontal_reach_blocks;

    let mut offsets = Vec::new();

    let max_reach_i = max_reach as isize;
    let max_reach_j = max_reach as isize;

    for di in -max_reach_i..=max_reach_i {
        for dj in -max_reach_j..=max_reach_j {
            // Skip the vertical (directly above) case: handled separately
            if di == 0 && dj == 0 {
                // Directly above: always add dk=1 (required for bench ordering)
                let offset = PrecedenceOffset::new(0, 0, 1).expect("dk=1 is valid");
                if !offsets.contains(&offset) {
                    offsets.push(offset);
                }
                continue;
            }

            let horizontal_dist = ((di as f64 * dx).powi(2) + (dj as f64 * dy).powi(2)).sqrt();

            // Compute azimuth: angle from north (Y+) measured clockwise
            // di → East (+X), dj → North (+Y)
            let azimuth_degrees = horizontal_azimuth_degrees(di as f64 * dx, dj as f64 * dy);
            let slope_angle_deg = slope_template.slope_angle_for_azimuth(azimuth_degrees);
            let slope_angle_rad = slope_angle_deg * PI / 180.0;

            // Minimum vertical distance required
            // tan(slope) = vertical / horizontal → vertical = horizontal * tan(slope)
            let required_vertical_dist = horizontal_dist * slope_angle_rad.tan();
            let dk_min = (required_vertical_dist / dz).ceil() as isize;

            // Include all dk values from dk_min to a sensible maximum
            // We limit to max_reach for the vertical direction as well
            let dk_limit = max_reach_i.max(dk_min);
            for dk in dk_min..=dk_limit {
                if dk >= 1 {
                    if let Ok(offset) = PrecedenceOffset::new(di, dj, dk) {
                        if !offsets.contains(&offset) {
                            offsets.push(offset);
                        }
                    }
                }
            }
        }
    }

    if offsets.is_empty() {
        return Err(MineError::invalid_parameter(
            "slope_template",
            "variable slope template produces no valid precedence offsets; \
             increase max_horizontal_reach_blocks or slope_angle_degrees",
        ));
    }

    BlockPrecedenceTemplate::new(offsets)
}

/// Normaliza un azimut al rango [0, 360).
fn normalize_azimuth(degrees: f64) -> f64 {
    let mut a = degrees % 360.0;
    if a < 0.0 {
        a += 360.0;
    }
    a
}

/// Calcula el azimut (Norte = 0°, Este = 90°) para un vector horizontal (dx_east, dy_north).
fn horizontal_azimuth_degrees(dx_east: f64, dy_north: f64) -> f64 {
    // atan2(east, north) gives azimuth from north
    let radians = dx_east.atan2(dy_north);
    let degrees = radians * 180.0 / PI;
    normalize_azimuth(degrees)
}

#[cfg(test)]
mod tests {
    use mine_core::{BlockDimensions, MineError};

    use super::*;

    fn block_dims_10m() -> BlockDimensions {
        BlockDimensions::new(10.0, 10.0, 10.0).expect("block dims should be valid")
    }

    #[test]
    fn uniform_45_degree_slope_includes_direct_above() {
        let dims = block_dims_10m();
        let template =
            VariableSlopeTemplate::uniform(45.0, 2).expect("template should be valid");
        let precedence =
            derive_precedence_template_from_slope(&dims, &template).expect("should succeed");

        // Offset (0, 0, 1) must always be present
        let offsets = precedence.predecessor_offsets();
        let has_direct_above = offsets
            .iter()
            .any(|o| o.di() == 0 && o.dj() == 0 && o.dk() == 1);
        assert!(has_direct_above, "direct above offset must be included");
    }

    #[test]
    fn uniform_45_degree_slope_includes_diagonal_predecessors() {
        let dims = block_dims_10m();
        // 45° slope: horizontal = vertical, so at di=1, dj=0 (horizontal dist=10m),
        // dk_min = ceil(10 * tan(45°) / 10) = ceil(1.0) = 1
        let template =
            VariableSlopeTemplate::uniform(45.0, 1).expect("template should be valid");
        let precedence =
            derive_precedence_template_from_slope(&dims, &template).expect("should succeed");

        let offsets = precedence.predecessor_offsets();
        assert!(!offsets.is_empty());
        // At 45°, di=1 dj=0 dk=1 should be valid
        assert!(
            offsets
                .iter()
                .any(|o| o.di() == 1 && o.dj() == 0 && o.dk() == 1),
            "offset (1,0,1) should be present for 45° slope"
        );
    }

    #[test]
    fn steep_slope_requires_higher_dk() {
        let dims = block_dims_10m();
        // 60° slope: tan(60°) ≈ 1.73, so at di=1 dj=0: dk_min = ceil(10 * 1.73 / 10) = 2
        let template =
            VariableSlopeTemplate::uniform(60.0, 2).expect("template should be valid");
        let precedence =
            derive_precedence_template_from_slope(&dims, &template).expect("should succeed");

        let offsets = precedence.predecessor_offsets();
        // di=1, dj=0, dk=1 should NOT be present (too shallow for 60° slope)
        let has_shallow = offsets
            .iter()
            .any(|o| o.di() == 1 && o.dj() == 0 && o.dk() == 1);
        assert!(
            !has_shallow,
            "offset (1,0,1) should not be present for 60° slope"
        );
        // di=1, dj=0, dk=2 should be present
        let has_correct = offsets
            .iter()
            .any(|o| o.di() == 1 && o.dj() == 0 && o.dk() == 2);
        assert!(
            has_correct,
            "offset (1,0,2) should be present for 60° slope"
        );
    }

    #[test]
    fn variable_slope_uses_different_angles_per_azimuth() {
        let dims = block_dims_10m();
        // East sector (45°-135°): 45° slope → dk_min=1 for di=1
        // North sector (315°-360° and 0°-45°): 60° slope → dk_min=2 for dj=1
        let rules = vec![
            SlopeAngleRule::new(45.0, 135.0, 45.0).expect("east rule should be valid"),
            SlopeAngleRule::new(315.0, 360.0, 60.0).expect("northwest rule should be valid"),
        ];
        let template =
            VariableSlopeTemplate::new(rules, 60.0, 2).expect("template should be valid");
        let precedence =
            derive_precedence_template_from_slope(&dims, &template).expect("should succeed");
        let offsets = precedence.predecessor_offsets();

        // East direction (di=1, dj=0) → azimuth 90° → 45° slope → dk_min=1
        let east_dk1 = offsets
            .iter()
            .any(|o| o.di() == 1 && o.dj() == 0 && o.dk() == 1);
        assert!(east_dk1, "east direction at 45° slope should allow dk=1");
    }

    #[test]
    fn slope_angle_rule_rejects_invalid_inputs() {
        assert!(SlopeAngleRule::new(-10.0, 90.0, 45.0).is_err(), "negative azimuth");
        assert!(SlopeAngleRule::new(0.0, 0.0, 45.0).is_err(), "zero-width sector");
        assert!(SlopeAngleRule::new(0.0, 90.0, 0.0).is_err(), "zero slope angle");
        assert!(SlopeAngleRule::new(0.0, 90.0, 90.0).is_err(), "90-degree slope angle");
    }

    #[test]
    fn variable_slope_template_rejects_zero_reach() {
        assert!(VariableSlopeTemplate::uniform(45.0, 0).is_err());
    }

    #[test]
    fn slope_angle_for_azimuth_falls_back_to_default() {
        let template = VariableSlopeTemplate::new(
            vec![SlopeAngleRule::new(0.0, 90.0, 30.0).expect("rule should be valid")],
            50.0,
            1,
        )
        .expect("template should be valid");

        assert_eq!(template.slope_angle_for_azimuth(0.0), 30.0); // within [0°, 90°)
        assert_eq!(template.slope_angle_for_azimuth(180.0), 50.0); // falls back to default
    }
}
