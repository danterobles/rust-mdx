//! Tareas de desarrollo: `cargo xtask <tarea>`.
//!
//! - `gen-fixtures`: genera `fixtures/large_1mb.md` y `fixtures/large_5mb.md` (ignorados por git).
//! - `size`: compila en release y verifica NFR-1 (binario < 15 MB).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const MAX_BINARY_BYTES: u64 = 15 * 1024 * 1024;

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    let result = match task.as_deref() {
        Some("gen-fixtures") => gen_fixtures(),
        Some("size") => size(),
        None | Some("help") => {
            eprintln!("Uso: cargo xtask <tarea>\nTareas: gen-fixtures, size");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("Tarea desconocida: {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf)
}

/// Un bloque Markdown variado (~1 KB) que se repite hasta alcanzar el tamaño pedido.
fn block(i: usize) -> String {
    format!(
        "## Sección {i}\n\n\
         Párrafo con *cursiva*, **negrita**, `código` y un [enlace](https://example.com/{i}). \
         Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
         incididunt ut labore et dolore magna aliqua. Ñandú, canción, árbol y corazón.\n\n\
         - elemento uno\n- elemento dos\n  - anidado\n- [x] tarea hecha\n- [ ] tarea pendiente\n\n\
         > Cita de la sección {i}.\n\n\
         ```rust\nfn seccion_{i}() -> usize {{\n    {i}\n}}\n```\n\n\
         | a | b | c |\n|---|---|---|\n| {i} | {} | {} |\n\n",
        i * 2,
        i * 3
    )
}

fn gen_fixtures() -> Result<(), String> {
    let dir = workspace_root().join("fixtures");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (name, target) in [
        ("large_1mb.md", 1024 * 1024),
        ("large_5mb.md", 5 * 1024 * 1024),
    ] {
        let mut text = String::from("# Fixture grande\n\n");
        let mut i = 0;
        while text.len() < target {
            text.push_str(&block(i));
            i += 1;
        }
        let path = dir.join(name);
        fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{} ({} bytes)", path.display(), text.len());
    }
    Ok(())
}

fn size() -> Result<(), String> {
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--release", "-p", "mdx-app"])
        .current_dir(workspace_root())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Falló la compilación release".into());
    }
    let exe = if cfg!(windows) { "mdx.exe" } else { "mdx" };
    let path = workspace_root().join("target").join("release").join(exe);
    let bytes = fs::metadata(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    #[allow(clippy::cast_precision_loss)]
    let mb = bytes as f64 / (1024.0 * 1024.0);
    println!(
        "{}: {bytes} bytes ({mb:.2} MB); límite NFR-1: 15 MB",
        path.display()
    );
    if bytes > MAX_BINARY_BYTES {
        return Err(format!("NFR-1 incumplido: {mb:.2} MB > 15 MB"));
    }
    Ok(())
}
