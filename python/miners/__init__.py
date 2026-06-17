"""Superficie Python recomendada para `mine-rs`.

La raiz `miners` expone la superficie alpha recomendada. Las APIs
experimentales viven en `miners.experimental`.
"""

from __future__ import annotations

from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as package_version

from . import experimental

from ._native import BasicStatistics
from ._native import BlockDimensions
from ._native import BlockModel
from ._native import ColumnNullCount
from ._native import ColumnSchema
from ._native import Coordinate3D
from ._native import GradeTonnagePoint
from ._native import GridDefinition
from ._native import GroupedStatistics
from ._native import MineError
from ._native import ModelSummary
from ._native import PyBindingSurface as PythonBindingSurface
from ._native import ValidationIssue
from ._native import ValidationReport
from ._native import WeightedGradeStatistic
from ._native import __version__ as _native_version
from ._native import binding_surface
from ._native import validate_duplicate_coordinates
from ._native import validate_duplicate_indices


try:
    __version__ = package_version("miners")
except PackageNotFoundError:
    __version__ = _native_version

# `miners.MineError` remains the single public exception surface for invalid
# user input and binding/tool contract mismatches. Normal validation findings
# are returned explicitly in `ValidationReport`.

def load_from_pandas(
    dataframe: object,
    grid: GridDefinition,
    schema: list[ColumnSchema],
    metadata: dict[str, str] | None = None,
) -> BlockModel:
    """Construye un `BlockModel` desde un `DataFrame` con una llamada explícita."""

    return BlockModel.from_pandas(
        dataframe=dataframe,
        grid=grid,
        schema=schema,
        metadata=metadata,
    )


def load_from_numpy(
    grid: GridDefinition,
    schema: list[ColumnSchema],
    metadata: dict[str, str] | None = None,
    float_columns: dict[str, object] | None = None,
    integer_columns: dict[str, object] | None = None,
    boolean_columns: dict[str, object] | None = None,
) -> BlockModel:
    """Construye un `BlockModel` desde arrays `numpy` con una llamada explícita."""

    return BlockModel.from_numpy(
        grid=grid,
        schema=schema,
        metadata=metadata,
        float_columns=float_columns,
        integer_columns=integer_columns,
        boolean_columns=boolean_columns,
    )


def export_to_pandas(
    model: BlockModel, columns: list[str] | None = None
) -> object:
    """Exporta columnas del modelo a `pandas` sin ocultar el intercambio de datos."""

    return model.to_pandas(columns=columns)


def export_to_numpy(
    model: BlockModel, columns: list[str] | None = None
) -> object:
    """Exporta columnas del modelo a `numpy` sin ocultar el intercambio de datos."""

    return model.to_numpy(columns=columns)


__all__ = [
    "BasicStatistics",
    "BlockDimensions",
    "BlockModel",
    "ColumnNullCount",
    "ColumnSchema",
    "Coordinate3D",
    "GradeTonnagePoint",
    "GridDefinition",
    "GroupedStatistics",
    "MineError",
    "ModelSummary",
    "PythonBindingSurface",
    "ValidationIssue",
    "ValidationReport",
    "WeightedGradeStatistic",
    "__version__",
    "export_to_numpy",
    "export_to_pandas",
    "experimental",
    "binding_surface",
    "load_from_numpy",
    "load_from_pandas",
    "validate_duplicate_coordinates",
    "validate_duplicate_indices",
]
