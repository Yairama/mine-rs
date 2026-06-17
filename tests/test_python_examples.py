"""Regression tests for executable public Python examples."""

from __future__ import annotations

from contextlib import redirect_stdout
import io
from pathlib import Path
import runpy
import unittest

import numpy as np


EXAMPLES_DIR = Path(__file__).resolve().parents[1] / "examples" / "python"


def run_example(name: str) -> dict[str, object]:
    namespace = runpy.run_path(str(EXAMPLES_DIR / name))

    with redirect_stdout(io.StringIO()):
        return namespace["main"]()


class PythonExamplesTest(unittest.TestCase):
    def test_pandas_example_remains_executable(self) -> None:
        result = run_example("pandas_load_validate_analyze_export.py")
        exported = result["exported"]

        self.assertFalse(result["report"].has_errors())
        self.assertEqual(result["summary"].block_count, 2)
        self.assertEqual(result["stats"].total_tonnage, 27.0)
        self.assertEqual(
            sorted(row.group_value for row in result["grouped"]),
            ["ore", "waste"],
        )
        self.assertEqual(list(exported.columns), ["cu", "tonnes", "domain"])
        self.assertEqual(exported.iloc[1]["domain"], "ore")

    def test_numpy_example_remains_executable(self) -> None:
        result = run_example("numpy_load_validate_export.py")
        exported = result["exported"]

        self.assertFalse(result["report"].has_errors())
        self.assertEqual(set(exported.keys()), {"cu", "bench", "selected"})
        self.assertEqual(exported["cu"].dtype, np.float64)
        self.assertEqual(exported["bench"].dtype, np.int64)
        self.assertEqual(exported["selected"].dtype, np.bool_)
        np.testing.assert_array_equal(exported["bench"], np.array([10, 20], dtype=np.int64))

    def test_tools_example_remains_executable(self) -> None:
        result = run_example("tools_workflow.py")

        self.assertTrue(result["inspected"]["success"])
        self.assertTrue(result["validated"]["success"])
        self.assertEqual(result["validated"]["output"]["report"]["issues"], [])
        self.assertEqual(result["queried"]["output"]["total_matches"], 1)
        self.assertEqual(result["queried"]["output"]["rows"][0]["values"]["domain"], "ore")
        self.assertEqual(len(result["aggregated"]["output"]["groups"]), 2)


if __name__ == "__main__":
    unittest.main()
