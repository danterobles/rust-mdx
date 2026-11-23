//! Lógica pura de Mdx: documentos, I/O, workspace, búsqueda y configuración.
//!
//! Este crate nunca depende de `iced`.

#![deny(clippy::unwrap_used, clippy::expect_used)]
// NFR-5 aplica al código de producción; los tests pueden usar unwrap.
#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::assert_is_empty)
)]

pub mod assets;
pub mod config;
pub mod document;
pub mod error;
pub mod export;
pub mod fileops;
pub mod history;
pub mod io;
pub mod link;
pub mod outline;
pub mod paths;
pub mod search;
pub mod session;
pub mod watcher;
pub mod workspace;

pub use document::{DiskState, Document, DocumentId, LineEnding};
pub use error::CoreError;

/// Devuelve la versión del workspace (única fuente: `[workspace.package]`).
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_is_semver_like() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
