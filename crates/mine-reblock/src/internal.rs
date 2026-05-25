use mine_blockmodel::ColumnData;
use mine_core::{ColumnId, ColumnLogicalType, GridDefinition, MineError};

pub(crate) enum AggregationBuffer {
    Integers(Vec<i64>),
    Floats(Vec<f64>),
    Booleans(Vec<bool>),
    Texts(Vec<String>),
}

impl AggregationBuffer {
    pub(crate) fn new(logical_type: ColumnLogicalType) -> Result<Self, MineError> {
        match logical_type {
            ColumnLogicalType::Integer => Ok(Self::Integers(Vec::new())),
            ColumnLogicalType::Float => Ok(Self::Floats(Vec::new())),
            ColumnLogicalType::Boolean => Ok(Self::Booleans(Vec::new())),
            ColumnLogicalType::Text => Ok(Self::Texts(Vec::new())),
        }
    }

    pub(crate) fn push(&mut self, value: AggregatedValue) -> Result<(), MineError> {
        match (self, value) {
            (Self::Integers(values), AggregatedValue::Integer(value)) => values.push(value),
            (Self::Floats(values), AggregatedValue::Float(value)) => values.push(value),
            (Self::Booleans(values), AggregatedValue::Boolean(value)) => values.push(value),
            (Self::Texts(values), AggregatedValue::Text(value)) => values.push(value),
            _ => {
                return Err(MineError::validation(
                    "aggregation buffer type does not match aggregated value",
                ));
            }
        }

        Ok(())
    }

    pub(crate) fn finish(self) -> ColumnData {
        match self {
            Self::Integers(values) => ColumnData::Integers(values),
            Self::Floats(values) => ColumnData::Floats(values),
            Self::Booleans(values) => ColumnData::Booleans(values),
            Self::Texts(values) => ColumnData::Texts(values),
        }
    }
}

pub(crate) enum AggregatedValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Text(String),
}

pub(crate) fn numeric_value_at(
    column_data: &ColumnData,
    row_index: usize,
    column_id: &ColumnId,
) -> Result<Option<f64>, MineError> {
    match column_data {
        ColumnData::Integers(values) => values
            .get(row_index)
            .map(|value| Some(*value as f64))
            .ok_or_else(|| {
                MineError::validation(format!(
                    "row index `{row_index}` is outside column `{column_id}`"
                ))
            }),
        ColumnData::Floats(values) => {
            values
                .get(row_index)
                .map(|value| Some(*value))
                .ok_or_else(|| {
                    MineError::validation(format!(
                        "row index `{row_index}` is outside column `{column_id}`"
                    ))
                })
        }
        _ => Err(MineError::invalid_parameter(
            "columns",
            format!(
                "weighted aggregation requires numeric columns, but `{column_id}` is not numeric"
            ),
        )),
    }
}

pub(crate) fn validate_superblock_grids(
    source_grid: &GridDefinition,
    target_grid: &GridDefinition,
    tolerance: f64,
) -> Result<(), MineError> {
    let source_origin = source_grid.origin();
    let target_origin = target_grid.origin();

    if (source_origin.x() - target_origin.x()).abs() > tolerance
        || (source_origin.y() - target_origin.y()).abs() > tolerance
        || (source_origin.z() - target_origin.z()).abs() > tolerance
    {
        return Err(MineError::grid(
            "superblock requires source and target grids to share the same origin",
        ));
    }

    let source_rotation = source_grid.rotation_degrees().unwrap_or(0.0);
    let target_rotation = target_grid.rotation_degrees().unwrap_or(0.0);
    if (source_rotation - target_rotation).abs() > tolerance {
        return Err(MineError::grid(
            "superblock requires source and target grids to share the same rotation",
        ));
    }

    validate_axis_ratio(
        source_grid.block_dimensions().dx(),
        target_grid.block_dimensions().dx(),
        source_grid.shape().nx(),
        target_grid.shape().nx(),
        tolerance,
        "x",
    )?;
    validate_axis_ratio(
        source_grid.block_dimensions().dy(),
        target_grid.block_dimensions().dy(),
        source_grid.shape().ny(),
        target_grid.shape().ny(),
        tolerance,
        "y",
    )?;
    validate_axis_ratio(
        source_grid.block_dimensions().dz(),
        target_grid.block_dimensions().dz(),
        source_grid.shape().nz(),
        target_grid.shape().nz(),
        tolerance,
        "z",
    )?;

    Ok(())
}

pub(crate) fn validate_subblock_grids(
    source_grid: &GridDefinition,
    target_grid: &GridDefinition,
    tolerance: f64,
) -> Result<usize, MineError> {
    let source_origin = source_grid.origin();
    let target_origin = target_grid.origin();

    if (source_origin.x() - target_origin.x()).abs() > tolerance
        || (source_origin.y() - target_origin.y()).abs() > tolerance
        || (source_origin.z() - target_origin.z()).abs() > tolerance
    {
        return Err(MineError::grid(
            "subblock requires source and target grids to share the same origin",
        ));
    }

    let source_rotation = source_grid.rotation_degrees().unwrap_or(0.0);
    let target_rotation = target_grid.rotation_degrees().unwrap_or(0.0);
    if (source_rotation - target_rotation).abs() > tolerance {
        return Err(MineError::grid(
            "subblock requires source and target grids to share the same rotation",
        ));
    }

    let factor_x = validate_subblock_axis_ratio(
        source_grid.block_dimensions().dx(),
        target_grid.block_dimensions().dx(),
        source_grid.shape().nx(),
        target_grid.shape().nx(),
        tolerance,
        "x",
    )?;
    let factor_y = validate_subblock_axis_ratio(
        source_grid.block_dimensions().dy(),
        target_grid.block_dimensions().dy(),
        source_grid.shape().ny(),
        target_grid.shape().ny(),
        tolerance,
        "y",
    )?;
    let factor_z = validate_subblock_axis_ratio(
        source_grid.block_dimensions().dz(),
        target_grid.block_dimensions().dz(),
        source_grid.shape().nz(),
        target_grid.shape().nz(),
        tolerance,
        "z",
    )?;

    Ok(factor_x * factor_y * factor_z)
}

fn validate_axis_ratio(
    source_dimension: f64,
    target_dimension: f64,
    source_count: usize,
    target_count: usize,
    tolerance: f64,
    axis_name: &str,
) -> Result<(), MineError> {
    let ratio = target_dimension / source_dimension;
    let rounded_ratio = ratio.round();

    if !ratio.is_finite() || ratio < 1.0 || (ratio - rounded_ratio).abs() > tolerance {
        return Err(MineError::grid(format!(
            "superblock target dimension on axis `{axis_name}` must be an integer multiple of the source dimension"
        )));
    }

    let factor = rounded_ratio as usize;
    if factor == 0 || !source_count.is_multiple_of(factor) || target_count != source_count / factor
    {
        return Err(MineError::grid(format!(
            "superblock target shape on axis `{axis_name}` is not aligned with the source grid"
        )));
    }

    Ok(())
}

fn validate_subblock_axis_ratio(
    source_dimension: f64,
    target_dimension: f64,
    source_count: usize,
    target_count: usize,
    tolerance: f64,
    axis_name: &str,
) -> Result<usize, MineError> {
    let ratio = source_dimension / target_dimension;
    let rounded_ratio = ratio.round();

    if !ratio.is_finite() || ratio < 1.0 || (ratio - rounded_ratio).abs() > tolerance {
        return Err(MineError::grid(format!(
            "subblock target dimension on axis `{axis_name}` must divide the source dimension exactly"
        )));
    }

    let factor = rounded_ratio as usize;
    if factor == 0 || target_count != source_count * factor {
        return Err(MineError::grid(format!(
            "subblock target shape on axis `{axis_name}` is not aligned with the source grid"
        )));
    }

    Ok(factor)
}

pub(crate) fn validate_non_negative_finite(
    name: &'static str,
    value: f64,
) -> Result<(), MineError> {
    if !value.is_finite() || value < 0.0 {
        return Err(MineError::numeric(format!(
            "{name} must be finite and greater than or equal to zero"
        )));
    }

    Ok(())
}
