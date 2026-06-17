from __future__ import annotations

import pandas as pd

import miners


def main() -> dict[str, object]:
    grid = miners.GridDefinition(
        origin=miners.Coordinate3D(0.0, 0.0, 0.0),
        block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
        shape=(2, 1, 1),
    )
    schema = [
        miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
        miners.ColumnSchema("tonnes", "float", unit="t", mining_role="tonnage"),
        miners.ColumnSchema("domain", "text", mining_role="domain"),
    ]
    dataframe = pd.DataFrame(
        {
            "cu": [0.8, 1.1],
            "tonnes": [12.0, 15.0],
            "domain": ["waste", "ore"],
        }
    )

    model = miners.load_from_pandas(
        dataframe=dataframe,
        grid=grid,
        schema=schema,
        metadata={"source": "inline-example"},
    )
    report = model.validate(required_columns=[("cu", "float"), ("tonnes", "float")])
    summary = model.summary()
    stats = model.basic_statistics("tonnes")
    grouped = model.grouped_statistics("domain", "tonnes")
    exported = miners.export_to_pandas(model, columns=["cu", "tonnes", "domain"])

    print("validation_errors:", report.error_count())
    print("block_count:", summary.block_count)
    print("total_tonnage:", stats.total_tonnage)
    print("domains:", [row.group_value for row in grouped])
    print(exported)

    return {
        "report": report,
        "summary": summary,
        "stats": stats,
        "grouped": grouped,
        "exported": exported,
    }


if __name__ == "__main__":
    main()
