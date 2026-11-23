//! Ubicación de los archivos de configuración y de estado de Mdx.

use std::ffi::OsString;
use std::path::PathBuf;

use directories::ProjectDirs;

/// Archivos que Mdx lee y escribe fuera de los documentos del usuario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppFiles {
    /// `config.toml` (lo edita el usuario; Mdx solo lo lee).
    pub config_file: PathBuf,
    /// `session.toml` (lo gestiona Mdx).
    pub session_file: PathBuf,
}

/// Resuelve las rutas según el sistema operativo (`directories`). Las variables de entorno
/// `MDX_CONFIG_DIR` y `MDX_STATE_DIR` las sustituyen (útil para pruebas y entornos portables).
/// `None` si no hay directorio personal.
#[must_use]
pub fn app_files() -> Option<AppFiles> {
    resolve(
        std::env::var_os("MDX_CONFIG_DIR"),
        std::env::var_os("MDX_STATE_DIR"),
        ProjectDirs::from("mx", "doitconsulting", "mdx").as_ref(),
    )
}

fn resolve(
    config_env: Option<OsString>,
    state_env: Option<OsString>,
    dirs: Option<&ProjectDirs>,
) -> Option<AppFiles> {
    let config_dir = config_env
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs.map(|d| d.config_dir().to_path_buf()))?;
    let state_dir = state_env
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            dirs.map(|d| {
                // `state_dir` solo existe en Linux; en el resto se usa el directorio de datos.
                d.state_dir()
                    .unwrap_or_else(|| d.data_local_dir())
                    .to_path_buf()
            })
        })?;
    Some(AppFiles {
        config_file: config_dir.join("config.toml"),
        session_file: state_dir.join("session.toml"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_overrides_win() {
        let files = resolve(Some("/cfg".into()), Some("/st".into()), None).unwrap();
        assert_eq!(files.config_file, PathBuf::from("/cfg/config.toml"));
        assert_eq!(files.session_file, PathBuf::from("/st/session.toml"));
    }

    #[test]
    fn without_overrides_or_home_there_are_no_paths() {
        assert_eq!(resolve(None, None, None), None);
        assert_eq!(
            resolve(Some(OsString::new()), Some(OsString::new()), None),
            None
        );
    }

    #[test]
    fn os_directories_are_used_by_default_and_end_in_the_expected_files() {
        let dirs = ProjectDirs::from("mx", "doitconsulting", "mdx");
        if let Some(files) = resolve(None, None, dirs.as_ref()) {
            assert!(files.config_file.ends_with("config.toml"));
            assert!(files.session_file.ends_with("session.toml"));
        }
    }
}
