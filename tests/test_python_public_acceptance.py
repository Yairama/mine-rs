"""Acceptance suite for the public Python alpha contract."""

from __future__ import annotations

from importlib import metadata
from pathlib import Path
import tomllib
import unittest

import miners
import miners.tools as tools
import numpy as np
import pandas as pd

try:
    from test_python_examples import run_example
except ModuleNotFoundError:
    from tests.test_python_examples import run_example


PYPROJECT_PATH = Path(__file__).resolve().parents[1] / "pyproject.toml"
TOOLS_SURFACE = [
    "inspect_model",
    "validate_model",
    "query_blocks",
    "aggregate_blocks",
    "grade_tonnage",
    "create_scenario",
    "evaluate_scenario",
    "compare_scenarios",
]


def tiny_grid() -> miners.GridDefinition:
    return miners.GridDefinition(
        origin=miners.Coordinate3D(0.0, 0.0, 0.0),
        block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
        shape=(2, 1, 1),
    )


def pandas_schema() -> list[miners.ColumnSchema]:
    return [
        miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
        miners.ColumnSchema("tonnes", "float", unit="t", mining_role="tonnage"),
        miners.ColumnSchema("domain", "text", mining_role="domain"),
    ]


def numpy_schema() -> list[miners.ColumnSchema]:
    return [
        miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
        miners.ColumnSchema("bench", "integer", mining_role="bench"),
        miners.ColumnSchema("selected", "boolean"),
    ]


class PythonPublicAcceptanceTest(unittest.TestCase):
    def test_install_import_and_version_contract(self) -> None:
        with PYPROJECT_PATH.open("rb") as pyproject_file:
            expected_version = tomllib.load(pyproject_file)["project"]["version"]

        surface = miners.binding_surface()

        self.assertEqual(miners.__version__, expected_version)
        self.assertEqual(metadata.version("miners"), expected_version)
        self.assertEqual(surface.package_version, expected_version)
        self.assertEqual(surface.binding_layer, "mine-python")
        self.assertEqual(surface.tool_layer, "mine-tools")
        self.assertIn("load_from_pandas", miners.__all__)
        self.assertIn("load_from_numpy", miners.__all__)

    def test_workflow_helpers_cover_supported_alpha_path(self) -> None:
        pandas_model = miners.load_from_pandas(
            dataframe=pd.DataFrame(
                {
                    "cu": [0.8, 1.1],
                    "tonnes": [12.0, 15.0],
                    "domain": ["waste", "ore"],
                }
            ),
            grid=tiny_grid(),
            schema=pandas_schema(),
            metadata={"source": "acceptance"},
        )
        pandas_report = pandas_model.validate(
            required_columns=[("cu", "float"), ("tonnes", "float")]
        )
        pandas_exported = miners.export_to_pandas(
            pandas_model, columns=["cu", "tonnes", "domain"]
        )

        self.assertFalse(pandas_report.has_errors())
        self.assertEqual(pandas_model.summary().block_count, 2)
        self.assertEqual(pandas_model.basic_statistics("tonnes").total_tonnage, 27.0)
        self.assertEqual(
            sorted(row.group_value for row in pandas_model.grouped_statistics("domain", "tonnes")),
            ["ore", "waste"],
        )
        self.assertEqual(len(pandas_model.grade_tonnage("cu", "tonnes", [0.7, 1.0])), 2)
        self.assertEqual(list(pandas_exported.columns), ["cu", "tonnes", "domain"])

        numpy_model = miners.load_from_numpy(
            grid=tiny_grid(),
            schema=numpy_schema(),
            metadata={"source": "acceptance"},
            float_columns={"cu": np.array([0.8, 1.1], dtype=np.float64)},
            integer_columns={"bench": np.array([10, 20], dtype=np.int64)},
            boolean_columns={"selected": np.array([False, True], dtype=np.bool_)},
        )
        numpy_report = numpy_model.validate(
            required_columns=[("cu", "float"), ("bench", "integer")]
        )
        numpy_exported = miners.export_to_numpy(
            numpy_model, columns=["cu", "bench", "selected"]
        )

        self.assertFalse(numpy_report.has_errors())
        self.assertEqual(set(numpy_exported.keys()), {"cu", "bench", "selected"})
        self.assertEqual(numpy_exported["cu"].dtype, np.float64)
        self.assertEqual(numpy_exported["bench"].dtype, np.int64)
        self.assertEqual(numpy_exported["selected"].dtype, np.bool_)

    def test_examples_pack_matches_public_alpha_workflows(self) -> None:
        pandas_result = run_example("pandas_load_validate_analyze_export.py")
        numpy_result = run_example("numpy_load_validate_export.py")
        tools_result = run_example("tools_workflow.py")

        self.assertFalse(pandas_result["report"].has_errors())
        self.assertEqual(pandas_result["summary"].block_count, 2)
        self.assertFalse(numpy_result["report"].has_errors())
        self.assertEqual(set(numpy_result["exported"].keys()), {"cu", "bench", "selected"})
        self.assertTrue(tools_result["validated"]["success"])
        self.assertEqual(tools_result["queried"]["output"]["total_matches"], 1)

    def test_deterministic_tools_surface_is_opt_in(self) -> None:
        self.assertEqual(tools.__all__, TOOLS_SURFACE)

        for name in TOOLS_SURFACE:
            with self.subTest(name=name):
                self.assertTrue(hasattr(tools, name))
                self.assertFalse(hasattr(miners, name))

        model = miners.load_from_pandas(
            dataframe=pd.DataFrame(
                {
                    "cu": [0.8, 1.1],
                    "tonnes": [12.0, 15.0],
                    "domain": ["waste", "ore"],
                }
            ),
            grid=tiny_grid(),
            schema=pandas_schema(),
        )

        inspected = tools.inspect_model(model)
        validated = tools.validate_model(model)

        self.assertTrue(inspected["success"])
        self.assertEqual(inspected["metadata"]["tool_name"], "inspect_model")
        self.assertTrue(validated["success"])
        self.assertEqual(validated["output"]["report"]["issues"], [])

    def test_mine_error_is_single_public_exception_for_invalid_user_input(self) -> None:
        public_exception_names = [
            name
            for name in miners.__all__
            if isinstance(getattr(miners, name, None), type)
            and issubclass(getattr(miners, name), Exception)
        ]

        self.assertEqual(public_exception_names, ["MineError"])

        with self.assertRaises(miners.MineError):
            miners.load_from_pandas(
                dataframe=pd.DataFrame({"cu": [0.8, 1.1], "tonnes": [12.0, 15.0]}),
                grid=tiny_grid(),
                schema=pandas_schema(),
            )

    def test_validation_findings_return_validation_report_instead_of_raising(self) -> None:
        model = miners.BlockModel(
            grid=tiny_grid(),
            schema=[
                miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
                miners.ColumnSchema("tonnes", "float", unit="t", mining_role="tonnage"),
            ],
            float_columns={"cu": [0.8, 1.1], "tonnes": [-1.0, 15.0]},
        )

        try:
            report = model.validate()
        except miners.MineError as error:  # pragma: no cover - contract failure path
            self.fail(f"validate() should return ValidationReport findings, not raise: {error}")

        self.assertIsInstance(report, miners.ValidationReport)
        self.assertTrue(report.has_errors())
        self.assertEqual(report.issues()[0].code, "invalid_tonnage_value")

    def test_schema_mismatched_tool_input_raises_mine_error(self) -> None:
        with self.assertRaisesRegex(miners.MineError, "Rust contract"):
            tools.create_scenario({"scenario_id": "scenario-01"})

    def test_non_json_serializable_tool_input_raises_mine_error(self) -> None:
        with self.assertRaisesRegex(miners.MineError, "JSON-serializable"):
            tools.create_scenario(object())


if __name__ == "__main__":
    unittest.main()
