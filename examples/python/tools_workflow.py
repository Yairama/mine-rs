from __future__ import annotations

import json

import miners
import miners.tools as tools


def main() -> dict[str, object]:
    grid = miners.GridDefinition(
        origin=miners.Coordinate3D(0.0, 0.0, 0.0),
        block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
        shape=(2, 1, 1),
    )
    model = miners.BlockModel(
        grid=grid,
        schema=[
            miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
            miners.ColumnSchema("tonnes", "float", unit="t", mining_role="tonnage"),
            miners.ColumnSchema("domain", "text", mining_role="domain"),
        ],
        float_columns={"cu": [0.8, 1.1], "tonnes": [12.0, 15.0]},
        text_columns={"domain": ["waste", "ore"]},
    )

    inspected = tools.inspect_model(model)
    validated = tools.validate_model(model)
    queried = tools.query_blocks(
        model,
        {
            "filters": [{"kind": "text_match", "column": "domain", "value": "ore"}],
            "selected_columns": ["cu", "domain"],
            "offset": 0,
            "limit": 10,
        },
    )
    aggregated = tools.aggregate_blocks(
        model,
        {"group_by": "domain", "tonnage_column": "tonnes"},
    )

    print(
        json.dumps(
            {
                "inspect_success": inspected["success"],
                "validation_success": validated["success"],
                "total_matches": queried["output"]["total_matches"],
                "groups": len(aggregated["output"]["groups"]),
            },
            indent=2,
        )
    )

    return {
        "inspected": inspected,
        "validated": validated,
        "queried": queried,
        "aggregated": aggregated,
    }


if __name__ == "__main__":
    main()
