//! Parsing CLI compartido para los bins benchmark de runtime (MR-209 / MR-211).
//!
//! Contrato común: `[--include-full] [--quiet] [output_path]`, con rutas
//! relativas rebasadas contra la raíz del repo según la política MR-202.

use std::path::PathBuf;

/// Opciones CLI comunes de los bins benchmark de runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkCliOptions {
    /// Incluir instancias pesadas full-scale (ej. `mclaughlin` 2.14M bloques).
    pub include_full: bool,
    /// Suprimir output de progreso en stdout.
    pub quiet: bool,
    /// Ruta de salida explícita del reporte (opcional).
    pub output_path: Option<PathBuf>,
}

/// Parsea los argumentos CLI comunes de los bins benchmark de runtime.
///
/// # Errores
///
/// Retorna un mensaje de error si encuentra flags desconocidos o más de una
/// ruta de salida posicional.
pub fn parse_benchmark_cli_args(args: &[String]) -> Result<BenchmarkCliOptions, String> {
    let mut options = BenchmarkCliOptions {
        include_full: false,
        quiet: false,
        output_path: None,
    };
    for arg in args {
        match arg.as_str() {
            "--include-full" => options.include_full = true,
            "--quiet" => options.quiet = true,
            other if other.starts_with("--") => {
                return Err(format!("unknown flag `{other}`"));
            }
            other => {
                if options.output_path.is_some() {
                    return Err("at most one output path is supported".to_owned());
                }
                options.output_path = Some(PathBuf::from(other));
            }
        }
    }
    Ok(options)
}
