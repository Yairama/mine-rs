"""Tests para la superficie opt-in `miners.tools`."""

from __future__ import annotations

from types import SimpleNamespace
import unittest
from unittest.mock import Mock
from unittest.mock import sentinel

import miners
import miners.tools as tools


TOOL_NAMES = [
    "inspect_model",
    "validate_model",
    "query_blocks",
    "aggregate_blocks",
    "grade_tonnage",
    "create_scenario",
    "evaluate_scenario",
    "compare_scenarios",
]


class MinersToolsSurfaceTest(unittest.TestCase):
    def test_tools_namespace_is_opt_in_and_uses_fixed_names(self) -> None:
        self.assertEqual(tools.__all__, TOOL_NAMES)

        for name in TOOL_NAMES:
            with self.subTest(name=name):
                self.assertTrue(hasattr(tools, name))
                self.assertFalse(hasattr(miners, name))

    def test_tools_namespace_delegates_to_native_bindings(self) -> None:
        fake_native = SimpleNamespace(
            **{name: Mock(name=name, return_value=(name, sentinel.response)) for name in TOOL_NAMES}
        )
        original_native = tools._native

        try:
            tools._native = fake_native

            for name in TOOL_NAMES:
                with self.subTest(name=name):
                    result = getattr(tools, name)(sentinel.model, option=sentinel.option)

                    self.assertEqual(result, (name, sentinel.response))
                    getattr(fake_native, name).assert_called_once_with(
                        sentinel.model, option=sentinel.option
                    )
        finally:
            tools._native = original_native


if __name__ == "__main__":
    unittest.main()
