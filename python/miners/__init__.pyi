"""Public typing contract for `miners`.

`miners.MineError` is the single public exception raised for invalid inputs and
binding/tool contract mismatches. Validation findings are returned through
`ValidationReport` in the normal validation flow.
"""

from typing import Any

from . import experimental

class PythonBindingSurface:
    binding_layer: str
    package_version: str
    sdk_layers: list[str]
    tool_layer: str
    available_tools: list[str]

    def __repr__(self) -> str: ...


class Coordinate3D:
    x: float
    y: float
    z: float

    def __init__(self, x: float, y: float, z: float) -> None: ...


class BlockDimensions:
    dx: float
    dy: float
    dz: float

    def __init__(self, dx: float, dy: float, dz: float) -> None: ...
    def volume(self) -> float: ...


class GridDefinition:
    origin: Coordinate3D
    block_dimensions: BlockDimensions
    shape: tuple[int, int, int]
    rotation_degrees: float | None

    def __init__(
        self,
        origin: Coordinate3D,
        block_dimensions: BlockDimensions,
        shape: tuple[int, int, int],
        rotation_degrees: float | None = None,
    ) -> None: ...


class ColumnSchema:
    name: str
    logical_type: str
    unit: str | None
    nullable: bool
    mining_role: str

    def __init__(
        self,
        name: str,
        logical_type: str,
        unit: str | None = None,
        nullable: bool = False,
        mining_role: str = "other",
    ) -> None: ...


class ColumnSummary:
    name: str
    logical_type: str
    unit: str | None
    nullable: bool
    mining_role: str
    row_count: int
    approximate_memory_bytes: int


class ModelSummary:
    block_count: int
    column_count: int
    shape: tuple[int, int, int]
    rotation_degrees: float | None
    approximate_memory_bytes: int
    metadata_keys: list[str]

    def columns(self) -> list[ColumnSummary]: ...
    def extent(self) -> tuple[tuple[float, float, float], tuple[float, float, float]]: ...


class ValidationIssue:
    severity: str
    code: str
    message: str
    location: str | None
    affected_count: int | None
    recommendation: str | None


class ValidationReport:
    """Validation findings returned by explicit validation APIs."""

    def has_errors(self) -> bool: ...
    def error_count(self) -> int: ...
    def warning_count(self) -> int: ...
    def issues(self) -> list[ValidationIssue]: ...
    def to_json(self) -> str: ...


class BlockModel:
    def __init__(
        self,
        grid: GridDefinition,
        schema: list[ColumnSchema],
        metadata: dict[str, str] | None = None,
        float_columns: dict[str, list[float]] | None = None,
        integer_columns: dict[str, list[int]] | None = None,
        boolean_columns: dict[str, list[bool]] | None = None,
        text_columns: dict[str, list[str]] | None = None,
    ) -> None: ...
    def block_count(self) -> int: ...
    def summary(self) -> ModelSummary: ...
    def validate(
        self, required_columns: list[tuple[str, str]] | None = None
    ) -> ValidationReport: ...


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
def validate_duplicate_indices(
    indices: list[tuple[int, int, int]],
) -> ValidationReport: ...
def validate_duplicate_coordinates(
    grid: GridDefinition,
    coordinates: list[tuple[float, float, float]],
    tolerance: float = 1e-9,
) -> ValidationReport: ...
