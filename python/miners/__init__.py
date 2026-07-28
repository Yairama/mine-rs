"""Superficie Python recomendada para `mine-rs`.

La raiz `miners` expone la superficie alpha recomendada. Las APIs
experimentales viven en `miners.experimental`.
"""

from __future__ import annotations

from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as package_version
from os import PathLike
from os import fspath

from . import experimental

from ._native import BasicStatistics
from ._native import AggregationRule
from ._native import BlockDimensions
from ._native import BlockModel
from ._native import ColumnNullCount
from ._native import ColumnSchema
from ._native import ColumnSummary
from ._native import Coordinate3D
from ._native import DistributionRule
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
from ._native import read_csv as _read_csv
from ._native import read_parquet as _read_parquet
from ._native import subblock
from ._native import superblock
from ._native import validate_duplicate_coordinates
from ._native import validate_duplicate_indices
from ._native import write_csv as _write_csv
from ._native import write_parquet as _write_parquet


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


def read_csv(
    path: str | PathLike[str],
    grid: GridDefinition,
    schema: list[ColumnSchema],
    metadata: dict[str, str] | None = None,
    index_columns: tuple[str, str, str] = ("i", "j", "k"),
) -> BlockModel:
    """Lee CSV mediante el contrato explícito de grilla y schema del core Rust."""

    return _read_csv(fspath(path), grid, schema, metadata, index_columns)


def write_csv(
    model: BlockModel,
    path: str | PathLike[str],
    index_columns: tuple[str, str, str] = ("i", "j", "k"),
    columns: list[str] | None = None,
) -> None:
    """Escribe un modelo a CSV mediante `mine-io`."""

    _write_csv(model, fspath(path), index_columns, columns)


def read_parquet(path: str | PathLike[str]) -> BlockModel:
    """Lee Parquet con grilla, schema y metadata embebidos por `mine-io`."""

    return _read_parquet(fspath(path))


def write_parquet(model: BlockModel, path: str | PathLike[str]) -> None:
    """Escribe un modelo a Parquet mediante `mine-io`."""

    _write_parquet(model, fspath(path))


__all__ = [
    "BasicStatistics",
    "AggregationRule",
    "BlockDimensions",
    "BlockModel",
    "ColumnNullCount",
    "ColumnSchema",
    "ColumnSummary",
    "Coordinate3D",
    "DistributionRule",
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
    "read_csv",
    "read_parquet",
    "subblock",
    "superblock",
    "validate_duplicate_coordinates",
    "validate_duplicate_indices",
    "write_csv",
    "write_parquet",
]
