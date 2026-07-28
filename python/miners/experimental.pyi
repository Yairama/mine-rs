from dataclasses import dataclass
from typing import Any, Self

from . import (
    BasicStatistics,
    BlockModel,
    GradeTonnagePoint,
    GroupedStatistics,
    ModelSummary,
    ValidationReport,
)

@dataclass(slots=True)
class ExperimentalBlockModelResult:
    validation: ValidationReport | None = None
    summary: ModelSummary | None = None
    basic_statistics: BasicStatistics | None = None
    grouped_statistics: list[GroupedStatistics] | None = None
    grade_tonnage: list[GradeTonnagePoint] | None = None
    dataframe: Any | None = None

class ExperimentalBlockModelWorkflow:
    def __init__(self, model: BlockModel) -> None: ...
    def validate(self, **kwargs: object) -> Self: ...
    def summary(self) -> Self: ...
    def basic_statistics(self, tonnage_column: str) -> Self: ...
    def grouped_statistics(self, group_by: str, tonnage_column: str) -> Self: ...
    def grade_tonnage(
        self, grade_column: str, tonnage_column: str, cutoffs: list[float]
    ) -> Self: ...
    def to_pandas(self, columns: list[str] | None = None) -> Self: ...
    def results(self) -> ExperimentalBlockModelResult: ...

def experimental_workflow(model: BlockModel) -> ExperimentalBlockModelWorkflow: ...

__all__ = [
    "ExperimentalBlockModelResult",
    "ExperimentalBlockModelWorkflow",
    "experimental_workflow",
]
