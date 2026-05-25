use super::*;

/// Opciones mínimas para exportar un `BlockModel` a VTU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VtuWriteOptions {
    selected_columns: Option<Vec<ColumnId>>,
}

impl VtuWriteOptions {
    /// Construye opciones de escritura VTU.
    pub fn new(selected_columns: Option<Vec<ColumnId>>) -> Result<Self, MineError> {
        validate_selected_columns(selected_columns.as_deref())?;

        Ok(Self { selected_columns })
    }

    /// Devuelve las columnas seleccionadas o `None` para exportar todas las compatibles con VTU.
    #[must_use]
    pub fn selected_columns(&self) -> Option<&[ColumnId]> {
        self.selected_columns.as_deref()
    }
}

/// Escribe un `BlockModel` regular a VTU ASCII para visualización en ParaView.
pub fn write_block_model_vtu(
    model: &BlockModel,
    path: impl AsRef<Path>,
    options: &VtuWriteOptions,
) -> Result<(), MineError> {
    ensure_dense_model_for_columnar_export(model, "write_block_model_vtu")?;
    ensure_unrotated_grid_for_vtu(model.grid())?;

    let selected_columns = resolve_vtu_selected_columns(model, options.selected_columns())?;
    let mut file = File::create(path.as_ref())
        .map_err(|error| io_error(format!("unable to create VTU file: {error}")))?;
    let block_count = model.block_count();
    let point_count = block_count
        .checked_mul(8)
        .ok_or_else(|| MineError::numeric("VTU point count overflowed"))?;

    writeln!(file, "<?xml version=\"1.0\"?>")
        .map_err(|error| io_error(format!("unable to write VTU header: {error}")))?;
    writeln!(
        file,
        "<VTKFile type=\"UnstructuredGrid\" version=\"0.1\" byte_order=\"LittleEndian\">"
    )
    .map_err(|error| io_error(format!("unable to write VTU header: {error}")))?;
    writeln!(file, "  <UnstructuredGrid>")
        .map_err(|error| io_error(format!("unable to write VTU grid: {error}")))?;
    writeln!(
        file,
        "    <Piece NumberOfPoints=\"{point_count}\" NumberOfCells=\"{block_count}\">"
    )
    .map_err(|error| io_error(format!("unable to write VTU piece: {error}")))?;
    write_vtu_cell_data(&mut file, model, &selected_columns)?;
    write_vtu_points(&mut file, model.grid(), block_count)?;
    write_vtu_cells(&mut file, block_count)?;
    writeln!(file, "    </Piece>")
        .map_err(|error| io_error(format!("unable to finalize VTU piece: {error}")))?;
    writeln!(file, "  </UnstructuredGrid>")
        .map_err(|error| io_error(format!("unable to finalize VTU grid: {error}")))?;
    writeln!(file, "</VTKFile>")
        .map_err(|error| io_error(format!("unable to finalize VTU file: {error}")))?;

    Ok(())
}

fn resolve_vtu_selected_columns(
    model: &BlockModel,
    selected_columns: Option<&[ColumnId]>,
) -> Result<Vec<ColumnId>, MineError> {
    let selected_columns = match selected_columns {
        Some(selected_columns) => selected_columns
            .iter()
            .map(|column_id| {
                let column_schema = model.schema().get(column_id).ok_or_else(|| {
                    MineError::schema(format!(
                        "column `{column_id}` does not exist in block model schema"
                    ))
                })?;

                if column_schema.logical_type() == ColumnLogicalType::Text {
                    return Err(MineError::schema(format!(
                        "column `{column_id}` has logical type `text`, which is not supported by the VTU exporter"
                    )));
                }

                Ok(column_id.clone())
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => model
            .schema()
            .iter()
            .filter(|(_, column_schema)| column_schema.logical_type() != ColumnLogicalType::Text)
            .map(|(column_id, _)| column_id.clone())
            .collect(),
    };

    Ok(selected_columns)
}

fn ensure_unrotated_grid_for_vtu(grid: &GridDefinition) -> Result<(), MineError> {
    match grid.rotation_degrees() {
        Some(rotation_degrees) if rotation_degrees != 0.0 => Err(MineError::grid(
            "write_block_model_vtu does not support rotated grids yet",
        )),
        _ => Ok(()),
    }
}

fn write_vtu_cell_data(
    writer: &mut File,
    model: &BlockModel,
    selected_columns: &[ColumnId],
) -> Result<(), MineError> {
    writeln!(writer, "      <CellData>")
        .map_err(|error| io_error(format!("unable to write VTU cell data header: {error}")))?;

    for column_id in selected_columns {
        let column_data = model.column(column_id).ok_or_else(|| {
            MineError::schema(format!(
                "column `{column_id}` is present in schema but missing from block model storage"
            ))
        })?;

        writeln!(
            writer,
            "        <DataArray type=\"{}\" Name=\"{}\" format=\"ascii\">{}</DataArray>",
            vtu_data_type(column_data),
            column_id.as_str(),
            vtu_values(column_data)
        )
        .map_err(|error| io_error(format!("unable to write VTU cell data array: {error}")))?;
    }

    writeln!(writer, "      </CellData>")
        .map_err(|error| io_error(format!("unable to finalize VTU cell data: {error}")))?;

    Ok(())
}

fn write_vtu_points(
    writer: &mut File,
    grid: &GridDefinition,
    block_count: usize,
) -> Result<(), MineError> {
    let origin = grid.origin();
    let dimensions = grid.block_dimensions();

    writeln!(writer, "      <Points>")
        .map_err(|error| io_error(format!("unable to write VTU points header: {error}")))?;
    write!(
        writer,
        "        <DataArray type=\"Float64\" NumberOfComponents=\"3\" format=\"ascii\">"
    )
    .map_err(|error| io_error(format!("unable to write VTU points header: {error}")))?;

    for linear_index in 0..block_count {
        let index = linear_to_ijk(grid, linear_index)?;
        let min_x = origin.x() + index.i() as f64 * dimensions.dx();
        let max_x = min_x + dimensions.dx();
        let min_y = origin.y() + index.j() as f64 * dimensions.dy();
        let max_y = min_y + dimensions.dy();
        let min_z = origin.z() + index.k() as f64 * dimensions.dz();
        let max_z = min_z + dimensions.dz();
        let corners = [
            (min_x, min_y, min_z),
            (max_x, min_y, min_z),
            (max_x, max_y, min_z),
            (min_x, max_y, min_z),
            (min_x, min_y, max_z),
            (max_x, min_y, max_z),
            (max_x, max_y, max_z),
            (min_x, max_y, max_z),
        ];

        for (x, y, z) in corners {
            write!(writer, "{x} {y} {z} ").map_err(|error| {
                io_error(format!("unable to write VTU point coordinates: {error}"))
            })?;
        }
    }

    writeln!(writer, "</DataArray>")
        .map_err(|error| io_error(format!("unable to finalize VTU point coordinates: {error}")))?;
    writeln!(writer, "      </Points>")
        .map_err(|error| io_error(format!("unable to finalize VTU points: {error}")))?;

    Ok(())
}

fn write_vtu_cells(writer: &mut File, block_count: usize) -> Result<(), MineError> {
    writeln!(writer, "      <Cells>")
        .map_err(|error| io_error(format!("unable to write VTU cells header: {error}")))?;
    write!(
        writer,
        "        <DataArray type=\"Int64\" Name=\"connectivity\" format=\"ascii\">"
    )
    .map_err(|error| io_error(format!("unable to write VTU connectivity header: {error}")))?;
    for block_index in 0..block_count {
        let first_point = block_index
            .checked_mul(8)
            .ok_or_else(|| MineError::numeric("VTU connectivity overflowed"))?;
        for offset in 0..8 {
            write!(writer, "{} ", first_point + offset)
                .map_err(|error| io_error(format!("unable to write VTU connectivity: {error}")))?;
        }
    }
    writeln!(writer, "</DataArray>")
        .map_err(|error| io_error(format!("unable to finalize VTU connectivity: {error}")))?;

    write!(
        writer,
        "        <DataArray type=\"Int64\" Name=\"offsets\" format=\"ascii\">"
    )
    .map_err(|error| io_error(format!("unable to write VTU offsets header: {error}")))?;
    for block_index in 0..block_count {
        write!(writer, "{} ", (block_index + 1) * 8)
            .map_err(|error| io_error(format!("unable to write VTU offsets: {error}")))?;
    }
    writeln!(writer, "</DataArray>")
        .map_err(|error| io_error(format!("unable to finalize VTU offsets: {error}")))?;

    write!(
        writer,
        "        <DataArray type=\"UInt8\" Name=\"types\" format=\"ascii\">"
    )
    .map_err(|error| io_error(format!("unable to write VTU types header: {error}")))?;
    for _ in 0..block_count {
        write!(writer, "12 ")
            .map_err(|error| io_error(format!("unable to write VTU types: {error}")))?;
    }
    writeln!(writer, "</DataArray>")
        .map_err(|error| io_error(format!("unable to finalize VTU types: {error}")))?;
    writeln!(writer, "      </Cells>")
        .map_err(|error| io_error(format!("unable to finalize VTU cells: {error}")))?;

    Ok(())
}

fn vtu_data_type(column_data: &ColumnData) -> &'static str {
    match column_data {
        ColumnData::Integers(_) => "Int64",
        ColumnData::Floats(_) => "Float64",
        ColumnData::Booleans(_) => "UInt8",
        ColumnData::Texts(_) => "String",
    }
}

fn vtu_values(column_data: &ColumnData) -> String {
    match column_data {
        ColumnData::Integers(values) => values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" "),
        ColumnData::Floats(values) => values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" "),
        ColumnData::Booleans(values) => values
            .iter()
            .map(|value| if *value { "1" } else { "0" })
            .collect::<Vec<_>>()
            .join(" "),
        ColumnData::Texts(values) => values.join(" "),
    }
}
