"""Superficie opt-in para tools deterministas de `miners`."""

from __future__ import annotations

from typing import Any

from .. import _native


def inspect_model(*args: Any, **kwargs: Any) -> Any:
    return _native.inspect_model(*args, **kwargs)


def validate_model(*args: Any, **kwargs: Any) -> Any:
    return _native.validate_model(*args, **kwargs)


def query_blocks(*args: Any, **kwargs: Any) -> Any:
    return _native.query_blocks(*args, **kwargs)


def aggregate_blocks(*args: Any, **kwargs: Any) -> Any:
    return _native.aggregate_blocks(*args, **kwargs)


def grade_tonnage(*args: Any, **kwargs: Any) -> Any:
    return _native.grade_tonnage(*args, **kwargs)


def create_scenario(*args: Any, **kwargs: Any) -> Any:
    return _native.create_scenario(*args, **kwargs)


def evaluate_scenario(*args: Any, **kwargs: Any) -> Any:
    return _native.evaluate_scenario(*args, **kwargs)


def compare_scenarios(*args: Any, **kwargs: Any) -> Any:
    return _native.compare_scenarios(*args, **kwargs)


__all__ = [
    "inspect_model",
    "validate_model",
    "query_blocks",
    "aggregate_blocks",
    "grade_tonnage",
    "create_scenario",
    "evaluate_scenario",
    "compare_scenarios",
]
