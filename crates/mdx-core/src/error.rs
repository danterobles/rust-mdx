//! Errores del núcleo.

use std::path::PathBuf;

/// Error de `mdx-core`. Es `Clone` para poder viajar dentro de mensajes de UI.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("No se encontró el archivo: {0}")]
    NotFound(PathBuf),
    #[error("Permiso denegado: {0}")]
    PermissionDenied(PathBuf),
    #[error("El archivo no es UTF-8 válido: {0}")]
    InvalidEncoding(PathBuf),
    #[error("Nombre inválido: {0}")]
    InvalidName(String),
    #[error("Ya existe: {0}")]
    AlreadyExists(PathBuf),
    #[error("Error de configuración: {0}")]
    Config(String),
    #[error("Error de E/S: {0}")]
    Io(String),
}

impl CoreError {
    /// Convierte un `std::io::Error` asociándolo a la ruta afectada.
    #[must_use]
    pub fn from_io(err: &std::io::Error, path: &std::path::Path) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound(path.to_path_buf()),
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied(path.to_path_buf()),
            std::io::ErrorKind::AlreadyExists => Self::AlreadyExists(path.to_path_buf()),
            _ => Self::Io(format!("{}: {err}", path.display())),
        }
    }
}
