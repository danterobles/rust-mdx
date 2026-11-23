//! Operaciones sobre archivos y carpetas del usuario: crear, renombrar, papelera y «revelar».
//!
//! Regla de seguridad (CLAUDE.md §7): **nunca** se borra con `remove_file`/`remove_dir_all`;
//! eliminar siempre envía a la papelera del sistema.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::CoreError;

const INVALID_CHARS: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn invalid(reason: &str) -> CoreError {
    CoreError::InvalidName(reason.to_owned())
}

/// Valida un nombre de archivo/carpeta (un solo componente) de forma portable entre sistemas.
/// No toca el disco.
///
/// # Errors
/// [`CoreError::InvalidName`] con el motivo, en español.
pub fn validate_name(name: &str) -> Result<(), CoreError> {
    if name.trim().is_empty() {
        return Err(invalid("el nombre no puede estar vacío"));
    }
    if name == "." || name == ".." {
        return Err(invalid("«.» y «..» no son nombres válidos"));
    }
    if let Some(c) = name
        .chars()
        .find(|c| INVALID_CHARS.contains(c) || c.is_control())
    {
        return Err(invalid(&format!(
            "el carácter «{}» no está permitido",
            c.escape_default()
        )));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(invalid("no puede terminar en punto ni en espacio"));
    }
    if name.len() > 255 {
        return Err(invalid("es demasiado largo (máximo 255 bytes)"));
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err(invalid("es un nombre reservado del sistema"));
    }
    Ok(())
}

/// Crea un archivo vacío `name` dentro de `dir`. Si el nombre no tiene extensión se añade `.md`.
/// Nunca sobrescribe: si existe devuelve [`CoreError::AlreadyExists`].
///
/// # Errors
/// [`CoreError::InvalidName`], [`CoreError::AlreadyExists`] o errores de E/S.
pub fn create_file(dir: &Path, name: &str) -> Result<PathBuf, CoreError> {
    validate_name(name)?;
    let name = if name.contains('.') {
        name.to_owned()
    } else {
        format!("{name}.md")
    };
    let path = dir.join(name);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| CoreError::from_io(&e, &path))?;
    Ok(path)
}

/// Crea la carpeta `name` dentro de `parent`.
///
/// # Errors
/// [`CoreError::InvalidName`], [`CoreError::AlreadyExists`] o errores de E/S.
pub fn create_dir(parent: &Path, name: &str) -> Result<PathBuf, CoreError> {
    validate_name(name)?;
    let path = parent.join(name);
    fs::create_dir(&path).map_err(|e| CoreError::from_io(&e, &path))?;
    Ok(path)
}

/// Renombra `path` a `new_name` dentro del mismo directorio y devuelve la ruta nueva.
/// No sobrescribe un destino existente (permite cambiar solo mayúsculas/minúsculas).
///
/// # Errors
/// [`CoreError::InvalidName`], [`CoreError::AlreadyExists`] o errores de E/S.
pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf, CoreError> {
    validate_name(new_name)?;
    let target = path.with_file_name(new_name);
    if target == path {
        return Ok(target);
    }
    if target.exists() {
        // En sistemas de archivos que no distinguen mayúsculas, `a.md` → `A.md` es el mismo
        // archivo y es un renombrado legítimo.
        let same = matches!(
            (fs::canonicalize(&target), fs::canonicalize(path)),
            (Ok(a), Ok(b)) if a == b
        );
        if !same {
            return Err(CoreError::AlreadyExists(target));
        }
    }
    fs::rename(path, &target).map_err(|e| CoreError::from_io(&e, path))?;
    Ok(target)
}

/// Envía `path` a la papelera del sistema (nunca lo borra directamente).
///
/// # Errors
/// [`CoreError::NotFound`] si no existe; [`CoreError::Io`] si la papelera falla.
pub fn move_to_trash(path: &Path) -> Result<(), CoreError> {
    move_to_trash_with(path, |p| trash::delete(p).map_err(|e| e.to_string()))
}

/// Núcleo de [`move_to_trash`] con el mecanismo inyectable (para probar sin tocar la papelera).
fn move_to_trash_with(
    path: &Path,
    trasher: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), CoreError> {
    if fs::symlink_metadata(path).is_err() {
        return Err(CoreError::NotFound(path.to_path_buf()));
    }
    trasher(path).map_err(|e| CoreError::Io(format!("{}: {e}", path.display())))
}

/// Si `doc_path` es `from` o está dentro de `from`, devuelve su ruta equivalente bajo `to`.
/// Sirve para mantener la ruta de un documento abierto al renombrar él o una carpeta padre.
#[must_use]
pub fn rebase_path(doc_path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    doc_path.strip_prefix(from).ok().map(|rest| {
        if rest.as_os_str().is_empty() {
            to.to_path_buf()
        } else {
            to.join(rest)
        }
    })
}

/// Programa y argumentos para mostrar `path` en el gestor de archivos del sistema.
#[must_use]
pub fn reveal_command(path: &Path) -> (String, Vec<String>) {
    let p = path.display().to_string();
    if cfg!(target_os = "macos") {
        ("open".into(), vec!["-R".into(), p])
    } else if cfg!(target_os = "windows") {
        ("explorer".into(), vec![format!("/select,{p}")])
    } else {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        ("xdg-open".into(), vec![dir.display().to_string()])
    }
}

/// Muestra `path` en Finder / Explorador / gestor de archivos.
///
/// # Errors
/// [`CoreError::Io`] si no se pudo lanzar el programa.
pub fn reveal(path: &Path) -> Result<(), CoreError> {
    let (program, args) = reveal_command(path);
    Command::new(&program)
        .args(&args)
        .spawn()
        .map(|_| ())
        .map_err(|e| CoreError::Io(format!("{program}: {e}")))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CoreError> + Send + 'static,
) -> Result<T, CoreError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?
}

/// Versión asíncrona de [`create_file`].
///
/// # Errors
/// Ver [`create_file`].
pub async fn create_file_async(dir: PathBuf, name: String) -> Result<PathBuf, CoreError> {
    blocking(move || create_file(&dir, &name)).await
}

/// Versión asíncrona de [`create_dir`].
///
/// # Errors
/// Ver [`create_dir`].
pub async fn create_dir_async(parent: PathBuf, name: String) -> Result<PathBuf, CoreError> {
    blocking(move || create_dir(&parent, &name)).await
}

/// Versión asíncrona de [`rename`].
///
/// # Errors
/// Ver [`rename`].
pub async fn rename_async(path: PathBuf, new_name: String) -> Result<PathBuf, CoreError> {
    blocking(move || rename(&path, &new_name)).await
}

/// Versión asíncrona de [`move_to_trash`].
///
/// # Errors
/// Ver [`move_to_trash`].
pub async fn move_to_trash_async(path: PathBuf) -> Result<(), CoreError> {
    blocking(move || move_to_trash(&path)).await
}

/// Versión asíncrona de [`reveal`].
///
/// # Errors
/// Ver [`reveal`].
pub async fn reveal_async(path: PathBuf) -> Result<(), CoreError> {
    blocking(move || reveal(&path)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn listing(dir: &Path) -> Vec<String> {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn validates_names() {
        for ok in [
            "nota.md",
            "Mi nota",
            "ñandú.md",
            ".oculto",
            "a.b.c",
            "COMA.md",
        ] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", "   ", ".", "..", "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b",
            "a\u{0}b", "nota.", "nota ", "con", "NUL.txt", "lpt1.md",
        ] {
            assert!(
                matches!(validate_name(bad), Err(CoreError::InvalidName(_))),
                "{bad:?} debería ser inválido"
            );
        }
        assert!(validate_name(&"x".repeat(256)).is_err());
    }

    #[test]
    fn create_file_adds_md_and_never_overwrites() {
        // AC4
        let dir = TempDir::new().unwrap();
        let p = create_file(dir.path(), "nueva").unwrap();
        assert_eq!(p, dir.path().join("nueva.md"));
        fs::write(&p, "contenido").unwrap();
        let err = create_file(dir.path(), "nueva.md").unwrap_err();
        assert!(matches!(err, CoreError::AlreadyExists(_)));
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "contenido",
            "no se tocó el existente"
        );
        assert_eq!(
            create_file(dir.path(), "datos.txt").unwrap(),
            dir.path().join("datos.txt")
        );
    }

    #[test]
    fn invalid_name_leaves_disk_untouched() {
        // AC4
        let dir = TempDir::new().unwrap();
        for bad in ["a/b.md", "", "con"] {
            assert!(create_file(dir.path(), bad).is_err());
            assert!(create_dir(dir.path(), bad).is_err());
        }
        assert_eq!(listing(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn create_dir_and_duplicate() {
        let dir = TempDir::new().unwrap();
        let p = create_dir(dir.path(), "docs").unwrap();
        assert!(p.is_dir());
        assert!(matches!(
            create_dir(dir.path(), "docs"),
            Err(CoreError::AlreadyExists(_))
        ));
    }

    #[test]
    fn rename_moves_file_and_refuses_duplicates() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        fs::write(&a, "A").unwrap();
        fs::write(&b, "B").unwrap();

        let err = rename(&a, "b.md").unwrap_err();
        assert!(matches!(err, CoreError::AlreadyExists(_)));
        assert_eq!(fs::read_to_string(&a).unwrap(), "A");
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");

        assert!(matches!(
            rename(&a, "x/y.md"),
            Err(CoreError::InvalidName(_))
        ));
        assert_eq!(listing(dir.path()), ["a.md", "b.md"]);

        let c = rename(&a, "c.md").unwrap();
        assert_eq!(c, dir.path().join("c.md"));
        assert_eq!(fs::read_to_string(&c).unwrap(), "A");
        assert_eq!(listing(dir.path()), ["b.md", "c.md"]);
        assert_eq!(rename(&c, "c.md").unwrap(), c, "mismo nombre: sin cambios");
    }

    #[test]
    fn rename_allows_case_only_change() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("nota.md");
        fs::write(&a, "x").unwrap();
        let b = rename(&a, "NOTA.md").unwrap();
        assert!(listing(dir.path()).contains(&"NOTA.md".to_string()));
        assert_eq!(fs::read_to_string(b).unwrap(), "x");
    }

    #[test]
    fn rename_directory() {
        let dir = TempDir::new().unwrap();
        let d = create_dir(dir.path(), "viejo").unwrap();
        fs::write(d.join("n.md"), "x").unwrap();
        let n = rename(&d, "nuevo").unwrap();
        assert!(n.join("n.md").is_file());
        assert!(!d.exists());
    }

    #[test]
    fn trash_goes_through_the_injected_trasher_and_reports_errors() {
        // AC2: la función solo delega en la papelera; no hay ninguna rama que borre.
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("a.md");
        fs::write(&f, "x").unwrap();
        let mut seen = None;
        move_to_trash_with(&f, |p| {
            seen = Some(p.to_path_buf());
            Ok(())
        })
        .unwrap();
        assert_eq!(seen, Some(f.clone()));
        assert!(
            f.exists(),
            "el trasher inyectado no borra: el archivo sigue ahí"
        );

        let err = move_to_trash_with(&f, |_| Err("sin papelera".into())).unwrap_err();
        assert!(matches!(err, CoreError::Io(m) if m.contains("sin papelera")));
        assert!(f.exists());

        let missing = dir.path().join("no.md");
        assert!(matches!(
            move_to_trash_with(&missing, |_| panic!("no debe llamarse")),
            Err(CoreError::NotFound(_))
        ));
    }

    #[test]
    fn rebase_path_for_open_documents() {
        // AC3
        let from = Path::new("/p/a.md");
        let to = Path::new("/p/b.md");
        assert_eq!(
            rebase_path(Path::new("/p/a.md"), from, to),
            Some(PathBuf::from("/p/b.md"))
        );
        assert_eq!(
            rebase_path(
                Path::new("/p/dir/x.md"),
                Path::new("/p/dir"),
                Path::new("/p/nuevo")
            ),
            Some(PathBuf::from("/p/nuevo/x.md"))
        );
        assert_eq!(rebase_path(Path::new("/p/otro.md"), from, to), None);
        assert_eq!(rebase_path(Path::new("/p/a.md.bak"), from, to), None);
    }

    #[test]
    fn reveal_command_mentions_the_path() {
        let (program, args) = reveal_command(Path::new("/tmp/x/a.md"));
        assert_ne!(program, "");
        assert!(args.iter().any(|a| a.contains("/tmp/x")));
    }
}
