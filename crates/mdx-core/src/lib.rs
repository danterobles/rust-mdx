//! Lógica pura de Mdx: documentos, I/O, workspace, búsqueda y configuración.
//!
//! Este crate nunca depende de `iced`.

#![deny(clippy::unwrap_used, clippy::expect_used)]

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
