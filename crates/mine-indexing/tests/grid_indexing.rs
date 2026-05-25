//! Tests de integración para los flujos públicos de `mine-indexing`.

use mine_core::{BlockDimensions, Coordinate3D, GridDefinition, GridShape, MineError};
use mine_indexing::{
    GridIndex, NeighborConnectivity, ijk_to_linear, ijk_to_xyz, linear_to_ijk, neighboring_blocks,
    xyz_to_ijk,
};

#[test]
fn convert_center_coordinate_to_index() {
    let index = xyz_to_ijk(
        &sample_grid(),
        Coordinate3D::new(105.0, 202.5, 310.0).expect("coordinate should be valid"),
        0.0,
    )
    .expect("coordinate should map to a valid block");

    assert_eq!(index, GridIndex::new(0, 0, 0));
}

#[test]
fn map_internal_border_to_upper_block() {
    let index = xyz_to_ijk(
        &sample_grid(),
        Coordinate3D::new(110.0, 205.0, 320.0).expect("coordinate should be valid"),
        0.0,
    )
    .expect("coordinate on border should map to valid block");

    assert_eq!(index, GridIndex::new(1, 1, 1));
}

#[test]
fn reject_outside_coordinate() {
    let error = xyz_to_ijk(
        &sample_grid(),
        Coordinate3D::new(90.0, 202.5, 310.0).expect("coordinate should be valid"),
        0.0,
    )
    .expect_err("outside coordinate should fail");

    assert_eq!(
        error,
        MineError::grid("coordinate `x=90` is outside the grid extent `100..140`")
    );
}

#[test]
fn allow_coordinate_within_tolerance_on_upper_extent() {
    let index = xyz_to_ijk(
        &sample_grid(),
        Coordinate3D::new(140.0001, 214.9999, 339.9999).expect("coordinate should be valid"),
        0.001,
    )
    .expect("coordinate should be accepted within tolerance");

    assert_eq!(index, GridIndex::new(3, 2, 1));
}

#[test]
fn roundtrip_index_through_center_coordinate() {
    let original = GridIndex::new(2, 1, 1);
    let coordinate = ijk_to_xyz(&sample_grid(), original).expect("index should be valid");
    let recovered =
        xyz_to_ijk(&sample_grid(), coordinate, 1e-9).expect("roundtrip should recover index");

    assert_eq!(recovered, original);
}

#[test]
fn convert_rotated_center_coordinate_to_index() {
    let index = xyz_to_ijk(
        &rotated_grid(),
        Coordinate3D::new(97.5, 205.0, 310.0).expect("coordinate should be valid"),
        1e-9,
    )
    .expect("rotated coordinate should map to a valid block");

    assert_eq!(index, GridIndex::new(0, 0, 0));
}

#[test]
fn convert_rotated_index_to_center_coordinate() {
    let coordinate =
        ijk_to_xyz(&rotated_grid(), GridIndex::new(1, 0, 0)).expect("index should be valid");

    assert!(
        coordinate
            .is_within_tolerance(
                &Coordinate3D::new(97.5, 215.0, 310.0).expect("coordinate should be valid"),
                1e-9,
            )
            .expect("tolerance should be valid")
    );
}

#[test]
fn roundtrip_rotated_index_through_center_coordinate() {
    let original = GridIndex::new(3, 2, 1);
    let coordinate = ijk_to_xyz(&angled_rotated_grid(), original).expect("index should be valid");
    let recovered = xyz_to_ijk(&angled_rotated_grid(), coordinate, 1e-9)
        .expect("roundtrip should recover rotated index");

    assert_eq!(recovered, original);
}

#[test]
fn convert_between_ijk_and_linear_index() {
    let index = GridIndex::new(1, 2, 1);
    let linear = ijk_to_linear(&sample_grid(), index).expect("index should linearize");
    let recovered = linear_to_ijk(&sample_grid(), linear).expect("linear index should decode");

    assert_eq!(linear, 21);
    assert_eq!(recovered, index);
}

#[test]
fn reject_out_of_bounds_linear_index() {
    let error =
        linear_to_ijk(&sample_grid(), 24).expect_err("linear index outside grid should fail");

    assert_eq!(
        error,
        MineError::grid("linear index `24` is outside grid capacity `24`")
    );
}

#[test]
fn list_face_neighbors_for_corner_block() {
    let neighbors = neighboring_blocks(
        &sample_grid(),
        GridIndex::new(0, 0, 0),
        NeighborConnectivity::Face6,
        None,
    )
    .expect("corner neighbors should be valid");

    assert_eq!(
        neighbors,
        vec![
            GridIndex::new(1, 0, 0),
            GridIndex::new(0, 1, 0),
            GridIndex::new(0, 0, 1),
        ]
    );
}

#[test]
fn list_face_neighbors_for_edge_block() {
    let neighbors = neighboring_blocks(
        &sample_grid(),
        GridIndex::new(1, 0, 0),
        NeighborConnectivity::Face6,
        None,
    )
    .expect("edge neighbors should be valid");

    assert_eq!(neighbors.len(), 4);
    assert!(neighbors.contains(&GridIndex::new(0, 0, 0)));
    assert!(neighbors.contains(&GridIndex::new(2, 0, 0)));
    assert!(neighbors.contains(&GridIndex::new(1, 1, 0)));
    assert!(neighbors.contains(&GridIndex::new(1, 0, 1)));
}

#[test]
fn list_neighbors_for_interior_block_with_multiple_connectivities() {
    let face_neighbors = neighboring_blocks(
        &interior_grid(),
        GridIndex::new(1, 1, 1),
        NeighborConnectivity::Face6,
        None,
    )
    .expect("face neighbors should be valid");
    let edge_neighbors = neighboring_blocks(
        &interior_grid(),
        GridIndex::new(1, 1, 1),
        NeighborConnectivity::Edge18,
        None,
    )
    .expect("edge neighbors should be valid");
    let corner_neighbors = neighboring_blocks(
        &interior_grid(),
        GridIndex::new(1, 1, 1),
        NeighborConnectivity::Corner26,
        None,
    )
    .expect("corner neighbors should be valid");

    assert_eq!(face_neighbors.len(), 6);
    assert_eq!(edge_neighbors.len(), 18);
    assert_eq!(corner_neighbors.len(), 26);
}

#[test]
fn filter_neighbors_with_sparse_occupied_indices() {
    let occupied = [
        ijk_to_linear(&interior_grid(), GridIndex::new(2, 1, 1))
            .expect("neighbor should linearize"),
        ijk_to_linear(&interior_grid(), GridIndex::new(1, 2, 2))
            .expect("neighbor should linearize"),
    ];
    let neighbors = neighboring_blocks(
        &interior_grid(),
        GridIndex::new(1, 1, 1),
        NeighborConnectivity::Corner26,
        Some(&occupied),
    )
    .expect("sparse neighbors should be valid");

    assert_eq!(
        neighbors,
        vec![GridIndex::new(2, 1, 1), GridIndex::new(1, 2, 2)]
    );
}

fn sample_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(100.0, 200.0, 300.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 5.0, 20.0).expect("dimensions should be valid"),
        GridShape::new(4, 3, 2).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid")
}

fn interior_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(0.0, 0.0, 0.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 10.0, 10.0).expect("dimensions should be valid"),
        GridShape::new(3, 3, 3).expect("shape should be valid"),
        None,
    )
    .expect("grid should be valid")
}

fn rotated_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(100.0, 200.0, 300.0).expect("origin should be valid"),
        BlockDimensions::new(10.0, 5.0, 20.0).expect("dimensions should be valid"),
        GridShape::new(4, 3, 2).expect("shape should be valid"),
        Some(90.0),
    )
    .expect("grid should be valid")
}

fn angled_rotated_grid() -> GridDefinition {
    GridDefinition::new(
        Coordinate3D::new(10.0, 20.0, 30.0).expect("origin should be valid"),
        BlockDimensions::new(4.0, 6.0, 8.0).expect("dimensions should be valid"),
        GridShape::new(5, 4, 3).expect("shape should be valid"),
        Some(27.5),
    )
    .expect("grid should be valid")
}
