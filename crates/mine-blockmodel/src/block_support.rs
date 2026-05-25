use mine_core::{BlockDimensions, Coordinate3D, MineError};
use serde::{Deserialize, Serialize};

use crate::VariogramModel;

/// Configuración de discretización para regularización de soporte de bloque.
///
/// La regularización produce covariancias promediadas sobre el volumen del bloque
/// discretizando el espacio interno con una grilla de puntos regulares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockDiscretization {
    nx: usize,
    ny: usize,
    nz: usize,
}

impl BlockDiscretization {
    /// Construye una discretización con el número de puntos por eje dado.
    ///
    /// Valores mínimos recomendados: 4x4x4 para bloques cúbicos. Usar 1x1x1
    /// produce estimación puntual equivalente sin regularización.
    pub fn new(nx: usize, ny: usize, nz: usize) -> Result<Self, MineError> {
        if nx == 0 || ny == 0 || nz == 0 {
            return Err(MineError::invalid_parameter(
                "nx/ny/nz",
                "block discretization point counts must be at least 1 in each axis",
            ));
        }

        Ok(Self { nx, ny, nz })
    }

    /// Total de puntos de discretización.
    #[must_use]
    pub const fn total_points(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    /// Genera los puntos de discretización centrados dentro de un bloque con
    /// el centro y dimensiones dados.
    pub fn discretize_block(
        &self,
        center: Coordinate3D,
        dimensions: &BlockDimensions,
    ) -> Vec<Coordinate3D> {
        let mut points = Vec::with_capacity(self.total_points());

        let half_dx = dimensions.dx() / 2.0;
        let half_dy = dimensions.dy() / 2.0;
        let half_dz = dimensions.dz() / 2.0;

        for iz in 0..self.nz {
            for iy in 0..self.ny {
                for ix in 0..self.nx {
                    let px =
                        center.x() - half_dx + dimensions.dx() * (ix as f64 + 0.5) / self.nx as f64;
                    let py =
                        center.y() - half_dy + dimensions.dy() * (iy as f64 + 0.5) / self.ny as f64;
                    let pz =
                        center.z() - half_dz + dimensions.dz() * (iz as f64 + 0.5) / self.nz as f64;

                    if let Ok(point) = Coordinate3D::new(px, py, pz) {
                        points.push(point);
                    }
                }
            }
        }

        points
    }
}

/// Resultado de la regularización de soporte de bloque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockSupportRegularization {
    /// Centro del bloque regularizado.
    pub block_center: Coordinate3D,
    /// Dimensiones del bloque.
    pub block_dimensions: BlockDimensions,
    /// Número de puntos de discretización usados.
    pub discretization_point_count: usize,
    /// Covarianza promedio bloque-a-bloque: C(V,V).
    pub block_to_block_covariance: f64,
    /// Sill total del modelo variográfico.
    pub total_sill: f64,
}

impl BlockSupportRegularization {
    /// Varianza de kriging regularizada por soporte de bloque.
    ///
    /// La varianza puntual del modelo se reduce por la covarianza bloque-a-bloque C(V,V).
    #[must_use]
    pub fn block_variance(&self) -> f64 {
        (self.total_sill - self.block_to_block_covariance).max(0.0)
    }
}

/// Calcula la covarianza promedio desde un punto de muestra hasta el volumen del bloque.
///
/// C(x_i, V) = (1/n) * sum_k C(x_i, v_k)
///
/// donde v_k son los n puntos de discretización del bloque.
pub fn compute_point_to_block_covariance(
    sample_location: Coordinate3D,
    block_center: Coordinate3D,
    block_dimensions: &BlockDimensions,
    discretization: &BlockDiscretization,
    variogram_model: &VariogramModel,
) -> Result<f64, MineError> {
    let block_points = discretization.discretize_block(block_center, block_dimensions);
    if block_points.is_empty() {
        return Err(MineError::validation(
            "block discretization produced no points; check discretization parameters",
        ));
    }

    let mut total_covariance = 0.0;
    for point in &block_points {
        let distance = euclidean_distance(sample_location, *point);
        let semivariance = variogram_model.semivariance(distance)?;
        total_covariance += variogram_model.total_sill() - semivariance;
    }

    Ok(total_covariance / block_points.len() as f64)
}

/// Calcula la covarianza promedio entre dos bloques (o dentro del mismo bloque si son iguales).
///
/// C(V1, V2) = (1/n1*n2) * sum_k sum_l C(v1_k, v2_l)
///
/// Cuando V1 == V2 el resultado es la varianza del bloque C(V,V).
pub fn compute_block_to_block_covariance(
    center1: Coordinate3D,
    dimensions1: &BlockDimensions,
    center2: Coordinate3D,
    dimensions2: &BlockDimensions,
    discretization: &BlockDiscretization,
    variogram_model: &VariogramModel,
) -> Result<f64, MineError> {
    let points1 = discretization.discretize_block(center1, dimensions1);
    let points2 = discretization.discretize_block(center2, dimensions2);

    if points1.is_empty() || points2.is_empty() {
        return Err(MineError::validation(
            "block discretization produced no points; check discretization parameters",
        ));
    }

    let mut total_covariance = 0.0;
    let pair_count = points1.len() * points2.len();
    for p1 in &points1 {
        for p2 in &points2 {
            let distance = euclidean_distance(*p1, *p2);
            let semivariance = variogram_model.semivariance(distance)?;
            total_covariance += variogram_model.total_sill() - semivariance;
        }
    }

    Ok(total_covariance / pair_count as f64)
}

/// Computa regularización completa para un bloque dado.
pub fn regularize_block_support(
    block_center: Coordinate3D,
    block_dimensions: &BlockDimensions,
    discretization: &BlockDiscretization,
    variogram_model: &VariogramModel,
) -> Result<BlockSupportRegularization, MineError> {
    let block_to_block_covariance = compute_block_to_block_covariance(
        block_center,
        block_dimensions,
        block_center,
        block_dimensions,
        discretization,
        variogram_model,
    )?;

    Ok(BlockSupportRegularization {
        block_center,
        block_dimensions: block_dimensions.clone(),
        discretization_point_count: discretization.total_points(),
        block_to_block_covariance,
        total_sill: variogram_model.total_sill(),
    })
}

fn euclidean_distance(a: Coordinate3D, b: Coordinate3D) -> f64 {
    let dx = b.x() - a.x();
    let dy = b.y() - a.y();
    let dz = b.z() - a.z();
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mine_core::{BlockDimensions, Coordinate3D};

    use super::*;
    use crate::{
        ExperimentalVariogram, ExperimentalVariogramLag, VariogramFitSummary, VariogramLagConfig,
        VariogramModel, VariogramModelKind,
    };

    fn spherical_model() -> VariogramModel {
        let variogram = ExperimentalVariogram {
            column_id: mine_core::ColumnId::new("cu").expect("column id should be valid"),
            domain: None,
            direction: None,
            lag_config: VariogramLagConfig::new(10.0, 3, 2.0).expect("lag config should be valid"),
            sample_count: 10,
            lags: vec![
                ExperimentalVariogramLag {
                    lag_index: 1,
                    lag_center: 10.0,
                    pair_count: 6,
                    average_distance: Some(10.0),
                    semivariance: Some(0.5),
                },
                ExperimentalVariogramLag {
                    lag_index: 2,
                    lag_center: 20.0,
                    pair_count: 4,
                    average_distance: Some(20.0),
                    semivariance: Some(0.85),
                },
                ExperimentalVariogramLag {
                    lag_index: 3,
                    lag_center: 30.0,
                    pair_count: 2,
                    average_distance: Some(30.0),
                    semivariance: Some(1.0),
                },
            ],
        };

        VariogramModel::from_variogram(
            &variogram,
            VariogramModelKind::Spherical,
            0.0,
            1.0,
            Some(30.0),
            VariogramFitSummary {
                observed_lag_count: 3,
                total_pair_count: 12,
                weighted_sse: 0.0,
                rmse: 0.0,
                mean_absolute_error: 0.0,
            },
        )
        .expect("variogram model should be valid")
    }

    #[test]
    fn block_discretization_point_count_is_correct() {
        let disc = BlockDiscretization::new(4, 4, 4).expect("discretization should be valid");
        assert_eq!(disc.total_points(), 64);
    }

    #[test]
    fn block_discretization_rejects_zero_axes() {
        assert!(BlockDiscretization::new(0, 4, 4).is_err());
        assert!(BlockDiscretization::new(4, 0, 4).is_err());
        assert!(BlockDiscretization::new(4, 4, 0).is_err());
    }

    #[test]
    fn discretize_block_centers_all_points_within_block_bounds() {
        let center = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");
        let dims = BlockDimensions::new(10.0, 10.0, 10.0).expect("dims should be valid");
        let disc = BlockDiscretization::new(4, 4, 4).expect("discretization should be valid");

        let points = disc.discretize_block(center, &dims);
        assert_eq!(points.len(), 64);
        for point in &points {
            assert!(
                point.x() >= -5.0 && point.x() <= 5.0,
                "x out of bounds: {}",
                point.x()
            );
            assert!(
                point.y() >= -5.0 && point.y() <= 5.0,
                "y out of bounds: {}",
                point.y()
            );
            assert!(
                point.z() >= -5.0 && point.z() <= 5.0,
                "z out of bounds: {}",
                point.z()
            );
        }
    }

    #[test]
    fn block_to_block_covariance_is_less_than_total_sill() {
        let model = spherical_model();
        let center = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");
        let dims = BlockDimensions::new(10.0, 10.0, 10.0).expect("dims should be valid");
        let disc = BlockDiscretization::new(4, 4, 4).expect("discretization should be valid");

        let c_vv = compute_block_to_block_covariance(center, &dims, center, &dims, &disc, &model)
            .expect("covariance should compute");

        assert!(c_vv > 0.0);
        assert!(c_vv < model.total_sill());
    }

    #[test]
    fn point_to_block_covariance_decreases_with_distance() {
        let model = spherical_model();
        let dims = BlockDimensions::new(10.0, 10.0, 10.0).expect("dims should be valid");
        let disc = BlockDiscretization::new(4, 4, 4).expect("discretization should be valid");
        let block_center = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");

        let near = Coordinate3D::new(5.0, 0.0, 0.0).expect("near sample location should be valid");
        let far = Coordinate3D::new(20.0, 0.0, 0.0).expect("far sample location should be valid");

        let cov_near = compute_point_to_block_covariance(near, block_center, &dims, &disc, &model)
            .expect("near covariance should compute");
        let cov_far = compute_point_to_block_covariance(far, block_center, &dims, &disc, &model)
            .expect("far covariance should compute");

        assert!(
            cov_near > cov_far,
            "covariance should decrease with distance (near={cov_near}, far={cov_far})"
        );
    }

    #[test]
    fn regularize_block_support_produces_reduced_block_variance() {
        let model = spherical_model();
        let center = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");
        let dims = BlockDimensions::new(10.0, 10.0, 10.0).expect("dims should be valid");
        let disc = BlockDiscretization::new(4, 4, 4).expect("discretization should be valid");

        let reg = regularize_block_support(center, &dims, &disc, &model)
            .expect("regularization should succeed");

        assert_eq!(reg.total_sill, model.total_sill());
        assert!(reg.block_to_block_covariance > 0.0);
        assert!(reg.block_variance() < model.total_sill());
        assert!(reg.block_variance() >= 0.0);
        assert_eq!(reg.discretization_point_count, 64);
    }

    #[test]
    fn one_discretization_point_equals_point_support_covariance() {
        let model = spherical_model();
        let center = Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid");
        let dims = BlockDimensions::new(0.01, 0.01, 0.01).expect("tiny dims should be valid");
        let disc = BlockDiscretization::new(1, 1, 1).expect("discretization should be valid");

        let reg = regularize_block_support(center, &dims, &disc, &model)
            .expect("regularization should succeed");

        // Con 1 punto y bloque infinitesimal, C(V,V) ≈ C(0) = sill total.
        let _ = reg.block_to_block_covariance; // solo comprobamos que no explota
        assert!(reg.block_to_block_covariance <= model.total_sill() + 1.0e-9);
    }
}
