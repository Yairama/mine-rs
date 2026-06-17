"""Smoke test para la instalación local del paquete `miners`."""

from importlib import metadata
from pathlib import Path
import tomllib
import unittest

import miners
import miners.tools as tools
import numpy as np
import pandas as pd


class MinersSmokeTest(unittest.TestCase):
    def test_package_version_contract(self) -> None:
        pyproject_path = Path(__file__).resolve().parents[1] / "pyproject.toml"

        with pyproject_path.open("rb") as pyproject_file:
            expected_version = tomllib.load(pyproject_file)["project"]["version"]

        surface = miners.binding_surface()

        self.assertEqual(miners.__version__, expected_version)
        self.assertEqual(metadata.version("miners"), expected_version)
        self.assertEqual(surface.package_version, expected_version)

    def test_binding_surface(self) -> None:
        surface = miners.binding_surface()

        self.assertEqual(surface.binding_layer, "mine-python")
        self.assertEqual(surface.package_version, miners.__version__)
        self.assertEqual(surface.sdk_layers, ["mine-core", "mine-sdk"])
        self.assertEqual(surface.tool_layer, "mine-tools")
        self.assertEqual(
            surface.available_tools,
            [
                "inspect_model",
                "validate_model",
                "query_blocks",
                "aggregate_blocks",
                "grade_tonnage",
                "create_scenario",
                "evaluate_scenario",
                "compare_scenarios",
            ],
        )

    def test_core_types_and_block_model(self) -> None:
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
            ],
            metadata={"source": "synthetic"},
            float_columns={
                "cu": [0.8, 1.1],
                "tonnes": [12.0, 15.0],
            },
        )

        summary = model.summary()
        report = model.validate(required_columns=[("cu", "float"), ("tonnes", "float")])

        self.assertEqual(model.block_count(), 2)
        self.assertEqual(summary.block_count, 2)
        self.assertEqual(summary.column_count, 2)
        self.assertEqual(summary.metadata_keys, ["source"])
        self.assertFalse(report.has_errors())

    def test_validation_reports_invalid_tonnage(self) -> None:
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
            ],
            float_columns={
                "cu": [0.8, 1.1],
                "tonnes": [-1.0, 15.0],
            },
        )

        report = model.validate()

        self.assertTrue(report.has_errors())
        self.assertEqual(report.error_count(), 1)
        self.assertEqual(report.issues()[0].code, "invalid_tonnage_value")

    def test_validation_reports_invalid_recovery(self) -> None:
        grid = miners.GridDefinition(
            origin=miners.Coordinate3D(0.0, 0.0, 0.0),
            block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
            shape=(2, 1, 1),
        )

        model = miners.BlockModel(
            grid=grid,
            schema=[miners.ColumnSchema("recovery", "float", mining_role="recovery")],
            float_columns={"recovery": [1.1, 0.8]},
        )

        report = model.validate()

        self.assertTrue(report.has_errors())
        self.assertEqual(report.error_count(), 1)
        self.assertEqual(report.issues()[0].code, "invalid_recovery_value")

    def test_validation_can_disable_value_checks(self) -> None:
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
            ],
            float_columns={
                "cu": [0.8, 1.1],
                "tonnes": [-1.0, 15.0],
            },
        )

        report = model.validate(validate_values=False)

        self.assertFalse(report.has_errors())
        self.assertEqual(report.error_count(), 0)
        self.assertEqual(report.issues(), [])

    def test_rotated_grid_validation_is_supported(self) -> None:
        grid = miners.GridDefinition(
            origin=miners.Coordinate3D(100.0, 200.0, 300.0),
            block_dimensions=miners.BlockDimensions(10.0, 5.0, 20.0),
            shape=(2, 1, 1),
            rotation_degrees=90.0,
        )

        model = miners.BlockModel(
            grid=grid,
            schema=[miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade")],
            float_columns={"cu": [0.8, 1.1]},
        )

        summary = model.summary()
        report = model.validate()

        self.assertEqual(summary.rotation_degrees, 90.0)
        self.assertFalse(report.has_errors())
        self.assertEqual(report.issues(), [])

    def test_sparse_block_model_can_allow_missing_blocks(self) -> None:
        grid = miners.GridDefinition(
            origin=miners.Coordinate3D(0.0, 0.0, 0.0),
            block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
            shape=(3, 1, 1),
        )

        model = miners.BlockModel(
            grid=grid,
            schema=[
                miners.ColumnSchema("cu", "float", unit="%Cu", mining_role="grade"),
                miners.ColumnSchema("tonnes", "float", unit="t", mining_role="tonnage"),
            ],
            float_columns={
                "cu": [0.8, 1.1],
                "tonnes": [12.0, 15.0],
            },
            materialized_linear_indices=[0, 1],
        )

        default_report = model.validate()
        sparse_report = model.validate(allow_sparse=True)

        self.assertEqual(model.block_count(), 2)
        self.assertTrue(default_report.has_errors())
        self.assertEqual(default_report.issues()[0].code, "missing_blocks_detected")
        self.assertEqual(
            [issue.code for issue in default_report.issues()],
            ["missing_blocks_detected", "incomplete_extent"],
        )
        self.assertFalse(sparse_report.has_errors())
        self.assertEqual(sparse_report.issues(), [])

    def test_duplicate_validators_report_pre_materialization_conflicts(self) -> None:
        index_report = miners.validate_duplicate_indices([(0, 0, 0), (1, 0, 0), (0, 0, 0)])
        grid = miners.GridDefinition(
            origin=miners.Coordinate3D(0.0, 0.0, 0.0),
            block_dimensions=miners.BlockDimensions(10.0, 10.0, 10.0),
            shape=(2, 1, 1),
        )
        coordinate_report = miners.validate_duplicate_coordinates(
            grid,
            [(5.0, 5.0, 5.0), (6.0, 5.5, 5.0), (15.0, 5.0, 5.0)],
        )

        self.assertTrue(index_report.has_errors())
        self.assertEqual(index_report.issues()[0].code, "duplicate_block_detected")
        self.assertTrue(coordinate_report.has_errors())
        self.assertEqual(coordinate_report.issues()[0].code, "duplicate_block_detected")
        self.assertEqual(coordinate_report.issues()[0].location, "coordinates")

    def test_pandas_roundtrip_and_python_analytics(self) -> None:
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

        self.assertIn("load_from_pandas", miners.__all__)
        self.assertIn("export_to_pandas", miners.__all__)

        model = miners.load_from_pandas(
            dataframe=dataframe,
            grid=grid,
            schema=schema,
            metadata={"source": "dataframe"},
        )

        report = model.validate(required_columns=[("cu", "float"), ("tonnes", "float")])
        stats = model.basic_statistics("tonnes")
        grouped = model.grouped_statistics("domain", "tonnes")
        curve = model.grade_tonnage("cu", "tonnes", [0.7, 1.0])
        exported = miners.export_to_pandas(model, columns=["cu", "domain"])

        self.assertFalse(report.has_errors())
        self.assertEqual(list(exported.columns), ["cu", "domain"])
        self.assertEqual(exported.iloc[1]["domain"], "ore")
        self.assertEqual(stats.total_tonnage, 27.0)
        self.assertEqual(len(grouped), 2)
        self.assertEqual(len(curve), 2)
        self.assertEqual(curve[1].block_count, 1)

    def test_validation_report_to_pandas(self) -> None:
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
            ],
            float_columns={
                "cu": [0.8, 1.1],
                "tonnes": [-1.0, 15.0],
            },
        )

        issues = model.validate().to_pandas()

        self.assertEqual(list(issues["code"]), ["invalid_tonnage_value"])
        self.assertEqual(list(issues["severity"]), ["error"])

    def test_tool_bindings_return_serializable_contract_envelopes(self) -> None:
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
        curve = tools.grade_tonnage(
            model,
            {
                "grade_column": "cu",
                "tonnage_column": "tonnes",
                "cutoffs": [0.7, 1.0],
            },
        )
        created = tools.create_scenario(
            {
                "scenario_id": "scenario-01",
                "model_id": "model-01",
                "periods": [
                    {"label": "P1", "target_tonnage": 1000.0, "target_blocks": None},
                    {"label": "P2", "target_tonnage": 1200.0, "target_blocks": None},
                ],
                "assumptions": {},
            }
        )
        evaluated = tools.evaluate_scenario(
            {
                "scenario": created["output"]["scenario"],
                "period_inputs": [
                    {"period_label": "P1", "revenue": 100.0, "cost": 40.0},
                    {"period_label": "P2", "revenue": 150.0, "cost": 50.0},
                ],
                "discount_rate_per_period": 0.1,
            }
        )
        compared = tools.compare_scenarios(
            {
                "base": evaluated["output"]["report"],
                "candidate": evaluated["output"]["report"],
            }
        )

        self.assertTrue(inspected["success"])
        self.assertEqual(inspected["metadata"]["tool_name"], "inspect_model")
        self.assertTrue(validated["success"])
        self.assertEqual(validated["output"]["report"]["issues"], [])
        self.assertTrue(queried["success"])
        self.assertEqual(queried["output"]["total_matches"], 1)
        self.assertEqual(queried["output"]["rows"][0]["values"]["domain"], "ore")
        self.assertTrue(aggregated["success"])
        self.assertEqual(len(aggregated["output"]["groups"]), 2)
        self.assertTrue(curve["success"])
        self.assertEqual(curve["output"]["summary"]["cutoff_count"], 2)
        self.assertTrue(created["success"])
        self.assertEqual(created["output"]["scenario"]["scenario_id"], "scenario-01")
        self.assertTrue(evaluated["success"])
        self.assertEqual(evaluated["output"]["report"]["scenario_id"], "scenario-01")
        self.assertTrue(compared["success"])
        self.assertEqual(compared["output"]["npv_delta"], 0.0)

    def test_experimental_workflow_is_explicit_and_chainable(self) -> None:
        self.assertIn("experimental", miners.__all__)
        self.assertFalse(hasattr(miners, "experimental_workflow"))
        self.assertFalse(hasattr(miners, "ExperimentalBlockModelWorkflow"))

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

        result = (
            miners.experimental.experimental_workflow(model)
            .validate(required_columns=[("cu", "float"), ("tonnes", "float")])
            .summary()
            .basic_statistics("tonnes")
            .grouped_statistics("domain", "tonnes")
            .grade_tonnage("cu", "tonnes", [0.7, 1.0])
            .to_pandas(columns=["cu", "domain"])
            .results()
        )

        self.assertIsNotNone(result.validation)
        self.assertFalse(result.validation.has_errors())
        self.assertIsNotNone(result.summary)
        self.assertEqual(result.summary.block_count, 2)
        self.assertIsNotNone(result.basic_statistics)
        self.assertEqual(result.basic_statistics.total_tonnage, 27.0)
        self.assertIsNotNone(result.grouped_statistics)
        self.assertEqual(len(result.grouped_statistics), 2)
        self.assertIsNotNone(result.grade_tonnage)
        self.assertEqual(len(result.grade_tonnage), 2)
        self.assertIsNotNone(result.dataframe)
        self.assertEqual(list(result.dataframe.columns), ["cu", "domain"])

    def test_numpy_roundtrip_preserves_types_shape_and_copy_semantics(self) -> None:
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
        cu = np.array([0.8, 1.1], dtype=np.float64)
        bench = np.array([10, 20], dtype=np.int64)
        selected = np.array([False, True], dtype=np.bool_)

        self.assertIn("load_from_numpy", miners.__all__)
        self.assertIn("export_to_numpy", miners.__all__)

        model = miners.load_from_numpy(
            grid=grid,
            schema=schema,
            metadata={"source": "numpy"},
            float_columns={"cu": cu},
            integer_columns={"bench": bench},
            boolean_columns={"selected": selected},
        )

        exported = miners.export_to_numpy(model, columns=["cu", "bench", "selected"])

        self.assertEqual(set(exported.keys()), {"cu", "bench", "selected"})
        self.assertEqual(exported["cu"].shape, (2,))
        self.assertEqual(exported["bench"].shape, (2,))
        self.assertEqual(exported["selected"].shape, (2,))
        self.assertEqual(exported["cu"].dtype, np.float64)
        self.assertEqual(exported["bench"].dtype, np.int64)
        self.assertEqual(exported["selected"].dtype, np.bool_)
        np.testing.assert_array_equal(exported["cu"], np.array([0.8, 1.1]))
        np.testing.assert_array_equal(exported["bench"], np.array([10, 20]))
        np.testing.assert_array_equal(exported["selected"], np.array([False, True]))

        cu[0] = 9.9
        exported["bench"][0] = 999
        reexported = miners.export_to_numpy(model, columns=["cu", "bench"])

        self.assertEqual(reexported["cu"][0], 0.8)
        self.assertEqual(reexported["bench"][0], 10)


if __name__ == "__main__":
    unittest.main()
