//! Tareas de desarrollo: `cargo xtask <tarea>`.
//! Tareas previstas: `gen-fixtures`, `size` (se implementan en sus historias).

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        None | Some("help") => {
            eprintln!("Uso: cargo xtask <tarea>\nTareas: gen-fixtures, size (pendientes)");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("Tarea desconocida o aún no implementada: {other}");
            ExitCode::FAILURE
        }
    }
}
