from typing import Any

class PythonBindingSurface:
    binding_layer: str
    sdk_layers: list[str]
    tool_layer: str

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
    def has_errors(self) -> bool: ...
    def error_count(self) -> int: ...
    def warning_count(self) -> int: ...
    def issues(self) -> list[ValidationIssue]: ...
    def to_json(self) -> str: ...


class ExperimentalBlockModelResult:
    validation: ValidationReport | None
    summary: ModelSummary | None
    basic_statistics: BasicStatistics | None
    grouped_statistics: list[GroupedStatistics] | None
    grade_tonnage: list[GradeTonnagePoint] | None
    dataframe: Any | None


class ExperimentalBlockModelWorkflow:
    def __init__(self, model: BlockModel) -> None: ...
    def validate(self, **kwargs: Any) -> ExperimentalBlockModelWorkflow: ...
    def summary(self) -> ExperimentalBlockModelWorkflow: ...
    def basic_statistics(
        self, tonnage_column: str
    ) -> ExperimentalBlockModelWorkflow: ...
    def grouped_statistics(
        self, group_by: str, tonnage_column: str
    ) -> ExperimentalBlockModelWorkflow: ...
    def grade_tonnage(
        self, grade_column: str, tonnage_column: str, cutoffs: list[float]
    ) -> ExperimentalBlockModelWorkflow: ...
    def to_pandas(
        self, columns: list[str] | None = None
    ) -> ExperimentalBlockModelWorkflow: ...
    def results(self) -> ExperimentalBlockModelResult: ...


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


class MineError(Exception): ...


def binding_surface() -> PythonBindingSurface: ...
def validate_duplicate_indices(
    indices: list[tuple[int, int, int]],
) -> ValidationReport: ...
def validate_duplicate_coordinates(
    grid: GridDefinition,
    coordinates: list[tuple[float, float, float]],
    tolerance: float = 1e-9,
) -> ValidationReport: ...
def experimental_workflow(model: BlockModel) -> ExperimentalBlockModelWorkflow: ...
