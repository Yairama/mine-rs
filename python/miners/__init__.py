"""Superficie Python inicial para `mine-rs`."""

from __future__ import annotations

from dataclasses import dataclass

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
from ._native import binding_surface
from ._native import validate_duplicate_coordinates
from ._native import validate_duplicate_indices


@dataclass(slots=True)
class ExperimentalBlockModelResult:
    """Resultados acumulados por el workflow experimental.

    La API es deliberadamente explicita: no infiere columnas de tonelaje o ley y solo
    encadena llamadas ya disponibles en `BlockModel`.
    """

    validation: ValidationReport | None = None
    summary: ModelSummary | None = None
    basic_statistics: BasicStatistics | None = None
    grouped_statistics: list[GroupedStatistics] | None = None
    grade_tonnage: list[GradeTonnagePoint] | None = None
    dataframe: object | None = None


class ExperimentalBlockModelWorkflow:
    """Workflow experimental estilo fluent API sobre `BlockModel`.

    Limites:
    - no ejecuta calculos nuevos; delega a la superficie estable existente;
    - no infiere columnas criticas como ley o tonelaje;
    - cada paso guarda solo el ultimo resultado calculado.
    """

    def __init__(self, model: BlockModel) -> None:
        self._model = model
        self._result = ExperimentalBlockModelResult()

    def validate(self, **kwargs: object) -> ExperimentalBlockModelWorkflow:
        self._result.validation = self._model.validate(**kwargs)
        return self

    def summary(self) -> ExperimentalBlockModelWorkflow:
        self._result.summary = self._model.summary()
        return self

    def basic_statistics(self, tonnage_column: str) -> ExperimentalBlockModelWorkflow:
        self._result.basic_statistics = self._model.basic_statistics(tonnage_column)
        return self

    def grouped_statistics(
        self, group_by: str, tonnage_column: str
    ) -> ExperimentalBlockModelWorkflow:
        self._result.grouped_statistics = self._model.grouped_statistics(
            group_by, tonnage_column
        )
        return self

    def grade_tonnage(
        self, grade_column: str, tonnage_column: str, cutoffs: list[float]
    ) -> ExperimentalBlockModelWorkflow:
        self._result.grade_tonnage = self._model.grade_tonnage(
            grade_column, tonnage_column, cutoffs
        )
        return self

    def to_pandas(
        self, columns: list[str] | None = None
    ) -> ExperimentalBlockModelWorkflow:
        self._result.dataframe = self._model.to_pandas(columns=columns)
        return self

    def results(self) -> ExperimentalBlockModelResult:
        return self._result


def experimental_workflow(model: BlockModel) -> ExperimentalBlockModelWorkflow:
    """Crea un workflow experimental encadenable sobre `BlockModel`."""

    return ExperimentalBlockModelWorkflow(model)


__all__ = [
    "BasicStatistics",
    "BlockDimensions",
    "BlockModel",
    "ExperimentalBlockModelResult",
    "ExperimentalBlockModelWorkflow",
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
    "experimental_workflow",
    "binding_surface",
    "validate_duplicate_coordinates",
    "validate_duplicate_indices",
]
