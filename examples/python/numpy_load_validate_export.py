from __future__ import annotations

import numpy as np

import miners


def main() -> dict[str, object]:
    grid = miners.GridDefinition(
        origin=miners.Coordinate3D(0.0, 0.0, 0.0),
        block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
        shape=(2, 1, 1),
    )
    schema = [
        miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
        miners.ColumnSchema("bench", "integer", mining_role="bench"),
        miners.ColumnSchema("selected", "boolean"),
    ]

    model = miners.load_from_numpy(
        grid=grid,
        schema=schema,
        metadata={"source": "inline-example"},
        float_columns={"cu": np.array([0.8, 1.1], dtype=np.float64)},
        integer_columns={"bench": np.array([10, 20], dtype=np.int64)},
        boolean_columns={"selected": np.array([False, True], dtype=np.bool_)},
    )
    report = model.validate(required_columns=[("cu", "float"), ("bench", "integer")])
    exported = miners.export_to_numpy(model, columns=["cu", "bench", "selected"])

    print("validation_errors:", report.error_count())
    print("cu_dtype:", exported["cu"].dtype)
    print("bench_dtype:", exported["bench"].dtype)
    print("selected_dtype:", exported["selected"].dtype)

    return {"report": report, "exported": exported}


if __name__ == "__main__":
    main()
