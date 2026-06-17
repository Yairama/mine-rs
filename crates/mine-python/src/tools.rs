use mine_tools::{
    AggregateBlocksInput, CompareScenariosInput, CreateScenarioInput, EvaluateScenarioInput,
    GradeTonnageInput, InspectModelInput, QueryBlocksInput, ToolResponse, ValidateModelInput,
    aggregate_blocks as run_aggregate_blocks, compare_scenarios as run_compare_scenarios,
    create_scenario as run_create_scenario, evaluate_scenario as run_evaluate_scenario,
    grade_tonnage as run_grade_tonnage, inspect_model as run_inspect_model,
    query_blocks as run_query_blocks, validate_model as run_validate_model,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{PyMineError, blockmodel::PyBlockModel};

#[pyfunction(name = "inspect_model", signature = (model, input=None))]
fn py_inspect_model(
    py: Python<'_>,
    model: &PyBlockModel,
    input: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let input = parse_optional_tool_input::<InspectModelInput>(py, input)?;
    response_to_python(py, &run_inspect_model(&model.inner, &input))
}

#[pyfunction(name = "validate_model", signature = (model, input=None))]
fn py_validate_model(
    py: Python<'_>,
    model: &PyBlockModel,
    input: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let input = parse_optional_tool_input::<ValidateModelInput>(py, input)?;
    response_to_python(py, &run_validate_model(&model.inner, &input))
}

#[pyfunction(name = "query_blocks", signature = (model, input=None))]
fn py_query_blocks(
    py: Python<'_>,
    model: &PyBlockModel,
    input: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let input = parse_optional_tool_input::<QueryBlocksInput>(py, input)?;
    response_to_python(py, &run_query_blocks(&model.inner, &input))
}

#[pyfunction(name = "aggregate_blocks")]
fn py_aggregate_blocks(
    py: Python<'_>,
    model: &PyBlockModel,
    input: Py<PyAny>,
) -> PyResult<Py<PyAny>> {
    let input = parse_required_tool_input::<AggregateBlocksInput>(py, input)?;
    response_to_python(py, &run_aggregate_blocks(&model.inner, &input))
}

#[pyfunction(name = "grade_tonnage")]
fn py_grade_tonnage(py: Python<'_>, model: &PyBlockModel, input: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let input = parse_required_tool_input::<GradeTonnageInput>(py, input)?;
    response_to_python(py, &run_grade_tonnage(&model.inner, &input))
}

#[pyfunction(name = "create_scenario")]
fn py_create_scenario(py: Python<'_>, input: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let input = parse_required_tool_input::<CreateScenarioInput>(py, input)?;
    response_to_python(py, &run_create_scenario(&input))
}

#[pyfunction(name = "evaluate_scenario")]
fn py_evaluate_scenario(py: Python<'_>, input: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let input = parse_required_tool_input::<EvaluateScenarioInput>(py, input)?;
    response_to_python(py, &run_evaluate_scenario(&input))
}

#[pyfunction(name = "compare_scenarios")]
fn py_compare_scenarios(py: Python<'_>, input: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let input = parse_required_tool_input::<CompareScenariosInput>(py, input)?;
    response_to_python(py, &run_compare_scenarios(&input))
}

fn parse_optional_tool_input<T>(py: Python<'_>, input: Option<Py<PyAny>>) -> PyResult<T>
where
    T: Default + DeserializeOwned,
{
    match input {
        Some(input) if !input.bind(py).is_none() => parse_required_tool_input(py, input),
        _ => Ok(T::default()),
    }
}

fn parse_required_tool_input<T>(py: Python<'_>, input: Py<PyAny>) -> PyResult<T>
where
    T: DeserializeOwned,
{
    let json = py.import("json")?;
    let serialized = json
        .call_method1("dumps", (input.bind(py),))
        .map_err(|error| {
            PyErr::new::<PyMineError, _>(format!(
                "tool inputs must be JSON-serializable Python dict/list/scalar structures: {error}"
            ))
        })?
        .extract::<String>()?;

    serde_json::from_str(&serialized).map_err(|error| {
        PyErr::new::<PyMineError, _>(format!(
            "tool input does not match the Rust contract: {error}"
        ))
    })
}

fn response_to_python<T>(py: Python<'_>, response: &ToolResponse<T>) -> PyResult<Py<PyAny>>
where
    T: Serialize,
{
    let serialized = serde_json::to_string(response)
        .map_err(|error| PyErr::new::<PyMineError, _>(error.to_string()))?;
    let json = py.import("json")?;

    Ok(json.call_method1("loads", (serialized,))?.unbind())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(py_inspect_model, module)?)?;
    module.add_function(wrap_pyfunction!(py_validate_model, module)?)?;
    module.add_function(wrap_pyfunction!(py_query_blocks, module)?)?;
    module.add_function(wrap_pyfunction!(py_aggregate_blocks, module)?)?;
    module.add_function(wrap_pyfunction!(py_grade_tonnage, module)?)?;
    module.add_function(wrap_pyfunction!(py_create_scenario, module)?)?;
    module.add_function(wrap_pyfunction!(py_evaluate_scenario, module)?)?;
    module.add_function(wrap_pyfunction!(py_compare_scenarios, module)?)?;
    Ok(())
}
