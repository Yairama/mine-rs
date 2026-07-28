"""Public typing contract for `miners`.

`miners.MineError` is the single public exception raised for invalid inputs and
binding/tool contract mismatches. Validation findings are returned through
`ValidationReport` in the normal validation flow.
"""

from os import PathLike
from typing import Any, Self, final

from . import experimental

@final
class PythonBindingSurface:
    @property
    def binding_layer(self) -> str: ...
    @property
    def package_version(self) -> str: ...
    @property
    def sdk_layers(self) -> list[str]: ...
    @property
    def tool_layer(self) -> str: ...
    @property
    def available_tools(self) -> list[str]: ...

    def __repr__(self) -> str: ...


@final
class Coordinate3D:
    def __new__(cls, x: float, y: float, z: float) -> Self: ...
    @property
    def x(self) -> float: ...
    @property
    def y(self) -> float: ...
    @property
    def z(self) -> float: ...
    def __repr__(self) -> str: ...


@final
class BlockDimensions:
    def __new__(cls, dx: float, dy: float, dz: float) -> Self: ...
    @property
    def dx(self) -> float: ...
    @property
    def dy(self) -> float: ...
    @property
    def dz(self) -> float: ...
    def volume(self) -> float: ...
    def __repr__(self) -> str: ...


@final
class GridDefinition:
    def __new__(
        cls,
        origin: Coordinate3D,
        block_dimensions: BlockDimensions,
        shape: tuple[int, int, int],
        rotation_degrees: float | None = None,
    ) -> Self: ...
    @property
    def origin(self) -> Coordinate3D: ...
    @property
    def block_dimensions(self) -> BlockDimensions: ...
    @property
    def shape(self) -> tuple[int, int, int]: ...
    @property
    def rotation_degrees(self) -> float | None: ...
    def xyz_to_ijk(
        self, coordinate: Coordinate3D, tolerance: float = 1e-9
    ) -> tuple[int, int, int]: ...
    def ijk_to_xyz(self, index: tuple[int, int, int]) -> Coordinate3D: ...
    def ijk_to_linear(self, index: tuple[int, int, int]) -> int: ...
    def linear_to_ijk(self, linear_index: int) -> tuple[int, int, int]: ...
    def __repr__(self) -> str: ...


@final
class ColumnSchema:
    def __new__(
        cls,
        name: str,
        logical_type: str,
        unit: str | None = None,
        nullable: bool = False,
        mining_role: str = "other",
    ) -> Self: ...
    @property
    def name(self) -> str: ...
    @property
    def logical_type(self) -> str: ...
    @property
    def unit(self) -> str | None: ...
    @property
    def nullable(self) -> bool: ...
    @property
    def mining_role(self) -> str: ...
    def __repr__(self) -> str: ...


@final
class ColumnSummary:
    @property
    def name(self) -> str: ...
    @property
    def logical_type(self) -> str: ...
    @property
    def unit(self) -> str | None: ...
    @property
    def nullable(self) -> bool: ...
    @property
    def mining_role(self) -> str: ...
    @property
    def row_count(self) -> int: ...
    @property
    def null_count(self) -> int: ...
    @property
    def approximate_memory_bytes(self) -> int: ...


@final
class ModelSummary:
    @property
    def block_count(self) -> int: ...
    @property
    def column_count(self) -> int: ...
    @property
    def shape(self) -> tuple[int, int, int]: ...
    @property
    def rotation_degrees(self) -> float | None: ...
    @property
    def approximate_memory_bytes(self) -> int: ...
    @property
    def metadata_keys(self) -> list[str]: ...
    def columns(self) -> list[ColumnSummary]: ...
    def extent(self) -> tuple[tuple[float, float, float], tuple[float, float, float]]: ...


@final
class ValidationIssue:
    @property
    def severity(self) -> str: ...
    @property
    def code(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def location(self) -> str | None: ...
    @property
    def affected_count(self) -> int | None: ...
    @property
    def recommendation(self) -> str | None: ...


@final
class ValidationReport:
    """Validation findings returned by explicit validation APIs."""

    def has_errors(self) -> bool: ...
    def error_count(self) -> int: ...
    def warning_count(self) -> int: ...
    def issues(self) -> list[ValidationIssue]: ...
    def to_json(self) -> str: ...
    def to_pandas(self) -> Any: ...


@final
class ColumnNullCount:
    @property
    def name(self) -> str: ...
    @property
    def null_count(self) -> int: ...


@final
class WeightedGradeStatistic:
    @property
    def name(self) -> str: ...
    @property
    def unit(self) -> str | None: ...
    @property
    def average_grade(self) -> float | None: ...
    @property
    def contained_metal(self) -> float | None: ...


@final
class BasicStatistics:
    @property
    def block_count(self) -> int: ...
    @property
    def tonnage_column(self) -> str: ...
    @property
    def total_tonnage(self) -> float: ...
    def null_counts(self) -> list[ColumnNullCount]: ...
    def grade_statistics(self) -> list[WeightedGradeStatistic]: ...


@final
class GroupedStatistics:
    @property
    def group_by(self) -> str: ...
    @property
    def group_value(self) -> str: ...
    @property
    def block_count(self) -> int: ...
    @property
    def tonnage_column(self) -> str: ...
    @property
    def total_tonnage(self) -> float: ...
    def grade_statistics(self) -> list[WeightedGradeStatistic]: ...


@final
class GradeTonnagePoint:
    @property
    def cutoff(self) -> float: ...
    @property
    def block_count(self) -> int: ...
    @property
    def tonnage(self) -> float: ...
    @property
    def average_grade(self) -> float | None: ...
    @property
    def contained_metal(self) -> float | None: ...
    @property
    def tonnage_percentage(self) -> float | None: ...


@final
class BlockModel:
    def __new__(
        cls,
        grid: GridDefinition,
        schema: list[ColumnSchema],
        metadata: dict[str, str] | None = None,
        float_columns: dict[str, list[float]] | None = None,
        integer_columns: dict[str, list[int]] | None = None,
        boolean_columns: dict[str, list[bool]] | None = None,
        text_columns: dict[str, list[str]] | None = None,
        materialized_linear_indices: list[int] | None = None,
    ) -> Self: ...
    @staticmethod
    def from_pandas(
        dataframe: Any,
        grid: GridDefinition,
        schema: list[ColumnSchema],
        metadata: dict[str, str] | None = None,
    ) -> BlockModel: ...
    @staticmethod
    def from_numpy(
        grid: GridDefinition,
        schema: list[ColumnSchema],
        metadata: dict[str, str] | None = None,
        float_columns: dict[str, Any] | None = None,
        integer_columns: dict[str, Any] | None = None,
        boolean_columns: dict[str, Any] | None = None,
    ) -> BlockModel: ...
    def block_count(self) -> int: ...
    def summary(self) -> ModelSummary: ...
    def basic_statistics(self, tonnage_column: str) -> BasicStatistics: ...
    def grouped_statistics(
        self, group_by: str, tonnage_column: str
    ) -> list[GroupedStatistics]: ...
    def grade_tonnage(
        self, grade_column: str, tonnage_column: str, cutoffs: list[float]
    ) -> list[GradeTonnagePoint]: ...
    def validate(
        self,
        required_columns: list[tuple[str, str]] | None = None,
        tolerance: float = 1e-9,
        validate_schema: bool = True,
        validate_grid: bool = True,
        validate_missing_blocks: bool = True,
        validate_extents: bool = True,
        validate_values: bool = True,
        allow_sparse: bool = False,
    ) -> ValidationReport: ...
    def to_pandas(self, columns: list[str] | None = None) -> Any: ...
    def to_numpy(self, columns: list[str] | None = None) -> dict[str, Any]: ...


@final
class AggregationRule:
    @staticmethod
    def sum(output_column: str, column: str) -> AggregationRule: ...
    @staticmethod
    def weighted_average(
        output_column: str, value_column: str, weight_column: str
    ) -> AggregationRule: ...
    @staticmethod
    def minimum(output_column: str, column: str) -> AggregationRule: ...
    @staticmethod
    def maximum(output_column: str, column: str) -> AggregationRule: ...
    @staticmethod
    def first(output_column: str, column: str) -> AggregationRule: ...
    @staticmethod
    def majority(output_column: str, column: str) -> AggregationRule: ...


@final
class DistributionRule:
    @staticmethod
    def split_equally(output_column: str, column: str) -> DistributionRule: ...
    @staticmethod
    def replicate(output_column: str, column: str) -> DistributionRule: ...


class MineError(Exception):
    """Single public exception surface for the Python SDK."""



def binding_surface() -> PythonBindingSurface: ...
def load_from_pandas(
    dataframe: Any,
    grid: GridDefinition,
    schema: list[ColumnSchema],
    metadata: dict[str, str] | None = None,
) -> BlockModel: ...
def load_from_numpy(
    grid: GridDefinition,
    schema: list[ColumnSchema],
    metadata: dict[str, str] | None = None,
    float_columns: dict[str, Any] | None = None,
    integer_columns: dict[str, Any] | None = None,
    boolean_columns: dict[str, Any] | None = None,
) -> BlockModel: ...
def export_to_pandas(
    model: BlockModel, columns: list[str] | None = None
) -> Any: ...
def export_to_numpy(
    model: BlockModel, columns: list[str] | None = None
) -> dict[str, Any]: ...
def read_csv(
    path: str | PathLike[str],
    grid: GridDefinition,
    schema: list[ColumnSchema],
    metadata: dict[str, str] | None = None,
    index_columns: tuple[str, str, str] = ("i", "j", "k"),
) -> BlockModel: ...
def write_csv(
    model: BlockModel,
    path: str | PathLike[str],
    index_columns: tuple[str, str, str] = ("i", "j", "k"),
    columns: list[str] | None = None,
) -> None: ...
def read_parquet(path: str | PathLike[str]) -> BlockModel: ...
def write_parquet(model: BlockModel, path: str | PathLike[str]) -> None: ...
def superblock(
    model: BlockModel,
    target_grid: GridDefinition,
    rules: list[AggregationRule],
) -> BlockModel: ...
def subblock(
    model: BlockModel,
    target_grid: GridDefinition,
    rules: list[DistributionRule],
) -> BlockModel: ...
def validate_duplicate_indices(
    indices: list[tuple[int, int, int]],
) -> ValidationReport: ...

__version__: str
__all__ = [
    "AggregationRule",
    "BasicStatistics",
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
    "binding_surface",
    "experimental",
    "export_to_numpy",
    "export_to_pandas",
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
def validate_duplicate_coordinates(
    grid: GridDefinition,
    coordinates: list[tuple[float, float, float]],
    tolerance: float = 1e-9,
) -> ValidationReport: ...
