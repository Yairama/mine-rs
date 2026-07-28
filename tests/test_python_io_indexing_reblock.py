"""End-to-end tests for Python IO, grid indexing and reblocking."""

from __future__ import annotations

from pathlib import Path

import miners
import pandas as pd
import pytest


def grid(shape: tuple[int, int, int], dx: float = 10.0) -> miners.GridDefinition:
    return miners.GridDefinition(
        origin=miners.Coordinate3D(100.0, 200.0, 300.0),
        block_dimensions=miners.BlockDimensions(dx, 10.0, 10.0),
        shape=shape,
    )


def schema() -> list[miners.ColumnSchema]:
    return [
        miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
        miners.ColumnSchema("tonnes", "float", unit="t", mining_role="tonnage"),
        miners.ColumnSchema("domain", "text", mining_role="domain"),
    ]


def source_model() -> miners.BlockModel:
    return miners.BlockModel(
        grid=grid((4, 1, 1)),
        schema=schema(),
        metadata={"source": "python-e2e"},
        float_columns={
            "cu": [0.5, 1.5, 2.0, 4.0],
            "tonnes": [10.0, 30.0, 20.0, 20.0],
        },
        text_columns={"domain": ["ore", "ore", "waste", "waste"]},
    )


def test_grid_indexing_roundtrip_delegates_to_rust() -> None:
    model_grid = grid((2, 2, 1))

    coordinate = model_grid.ijk_to_xyz((1, 1, 0))

    assert (coordinate.x, coordinate.y, coordinate.z) == (115.0, 215.0, 305.0)
    assert model_grid.xyz_to_ijk(coordinate) == (1, 1, 0)
    assert model_grid.ijk_to_linear((1, 1, 0)) == 3
    assert model_grid.linear_to_ijk(3) == (1, 1, 0)

    with pytest.raises(miners.MineError, match="greater than or equal to zero"):
        model_grid.ijk_to_linear((-1, 0, 0))
    with pytest.raises(miners.MineError, match="greater than or equal to zero"):
        model_grid.linear_to_ijk(-1)


def test_csv_and_parquet_file_roundtrips(tmp_path: Path) -> None:
    model = source_model()
    csv_path = tmp_path / "model.csv"
    parquet_path = tmp_path / "model.parquet"

    miners.write_csv(model, csv_path, index_columns=("ix", "iy", "iz"))
    csv_model = miners.read_csv(
        csv_path,
        grid((4, 1, 1)),
        schema(),
        metadata={"source": "csv"},
        index_columns=("ix", "iy", "iz"),
    )
    miners.write_parquet(model, parquet_path)
    parquet_model = miners.read_parquet(parquet_path)

    expected = model.to_pandas()
    pd.testing.assert_frame_equal(csv_model.to_pandas(), expected)
    pd.testing.assert_frame_equal(parquet_model.to_pandas(), expected)
    assert csv_model.summary().metadata_keys == ["source"]
    assert parquet_model.summary().metadata_keys == ["source"]


def test_csv_writer_rejects_sparse_model_without_creating_false_roundtrip(tmp_path: Path) -> None:
    sparse = miners.BlockModel(
        grid=grid((3, 1, 1)),
        schema=schema(),
        float_columns={"cu": [0.5, 2.0], "tonnes": [10.0, 20.0]},
        text_columns={"domain": ["ore", "waste"]},
        materialized_linear_indices=[0, 2],
    )
    path = tmp_path / "sparse.csv"

    with pytest.raises(miners.MineError, match="does not reconstruct sparse layouts"):
        miners.write_csv(sparse, path)

    assert not path.exists()


def test_superblock_and_subblock_execute_rust_rules() -> None:
    model = source_model()
    coarse_grid = grid((2, 1, 1), dx=20.0)

    coarse = miners.superblock(
        model,
        coarse_grid,
        [
            miners.AggregationRule.weighted_average("cu", "cu", "tonnes"),
            miners.AggregationRule.sum("tonnes", "tonnes"),
            miners.AggregationRule.majority("domain", "domain"),
        ],
    )
    coarse_data = coarse.to_pandas()

    assert coarse.summary().shape == (2, 1, 1)
    assert list(coarse_data["cu"]) == [1.25, 3.0]
    assert list(coarse_data["tonnes"]) == [40.0, 40.0]
    assert list(coarse_data["domain"]) == ["ore", "waste"]

    refined = miners.subblock(
        coarse,
        grid((4, 1, 1)),
        [
            miners.DistributionRule.replicate("cu", "cu"),
            miners.DistributionRule.split_equally("tonnes", "tonnes"),
            miners.DistributionRule.replicate("domain", "domain"),
        ],
    )
    refined_data = refined.to_pandas()

    assert refined.summary().shape == (4, 1, 1)
    assert list(refined_data["cu"]) == [1.25, 1.25, 3.0, 3.0]
    assert list(refined_data["tonnes"]) == [20.0, 20.0, 20.0, 20.0]
    assert list(refined_data["domain"]) == ["ore", "ore", "waste", "waste"]


def test_reblocking_rejects_rules_that_corrupt_mining_semantics() -> None:
    model = source_model()

    with pytest.raises(miners.MineError, match="weighted average"):
        miners.superblock(
            model,
            grid((2, 1, 1), dx=20.0),
            [miners.AggregationRule.sum("cu", "cu")],
        )

    with pytest.raises(miners.MineError, match="use replicate"):
        miners.subblock(
            model,
            grid((8, 1, 1), dx=5.0),
            [miners.DistributionRule.split_equally("cu", "cu")],
        )

    with pytest.raises(miners.MineError, match="use sum"):
        miners.superblock(
            model,
            grid((2, 1, 1), dx=20.0),
            [miners.AggregationRule.weighted_average("tonnes", "tonnes", "tonnes")],
        )


def test_new_routes_are_on_the_recommended_root_surface() -> None:
    expected = {
        "AggregationRule",
        "DistributionRule",
        "read_csv",
        "write_csv",
        "read_parquet",
        "write_parquet",
        "superblock",
        "subblock",
    }

    assert expected <= set(miners.__all__)
