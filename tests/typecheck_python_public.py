"""Static consumer of the supported public Python surface."""

from pathlib import Path
from typing import assert_type

import miners


grid = miners.GridDefinition(
    miners.Coordinate3D(0.0, 0.0, 0.0),
    miners.BlockDimensions(10.0, 10.0, 10.0),
    (2, 1, 1),
)
schema = [miners.ColumnSchema("tonnes", "float", mining_role="tonnage")]
model = miners.BlockModel(grid, schema, float_columns={"tonnes": [1.0, 2.0]})

assert_type(grid.ijk_to_linear((1, 0, 0)), int)
assert_type(grid.linear_to_ijk(1), tuple[int, int, int])
assert_type(grid.ijk_to_xyz((1, 0, 0)), miners.Coordinate3D)
assert_type(grid.xyz_to_ijk(miners.Coordinate3D(15.0, 5.0, 5.0)), tuple[int, int, int])
assert_type(miners.write_csv(model, Path("model.csv")), None)
assert_type(miners.read_csv(Path("model.csv"), grid, schema), miners.BlockModel)
assert_type(miners.write_parquet(model, Path("model.parquet")), None)
assert_type(miners.read_parquet(Path("model.parquet")), miners.BlockModel)
assert_type(
    miners.experimental.experimental_workflow(model).summary().results(),
    miners.experimental.ExperimentalBlockModelResult,
)
assert_type(
    miners.superblock(
        model,
        miners.GridDefinition(
            miners.Coordinate3D(0.0, 0.0, 0.0),
            miners.BlockDimensions(20.0, 10.0, 10.0),
            (1, 1, 1),
        ),
        [miners.AggregationRule.sum("tonnes", "tonnes")],
    ),
    miners.BlockModel,
)
