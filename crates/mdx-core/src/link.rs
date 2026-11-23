//! Clasificación de enlaces de la vista previa.

use std::path::{Path, PathBuf};

/// Extensiones que Mdx abre en el editor (sin distinguir mayúsculas).
pub const MARKDOWN_EXTENSIONS: [&str; 4] = ["md", "markdown", "mdown", "mkd"];

/// Qué hacer con un enlace pulsado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// `http`/`https`: abrir en el navegador del sistema.
    External(String),
    /// Archivo Markdown local: abrir en el editor.
    Markdown(PathBuf),
    /// Ancla interna, esquema no soportado o archivo no Markdown.
    Unsupported,
}

/// Clasifica `url` resolviendo rutas relativas contra `base_dir` (directorio del documento).
#[must_use]
pub fn classify(url: &str, base_dir: Option<&Path>) -> LinkTarget {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return LinkTarget::External(url.to_owned());
    }
    if url.is_empty() || url.starts_with('#') || has_scheme(url) {
        return LinkTarget::Unsupported;
    }
    // Quita ancla y consulta: `otro.md#seccion`, `otro.md?x=1`.
    let without_fragment = url.split(['#', '?']).next().unwrap_or("");
    let decoded = percent_decode(without_fragment);
    let path = Path::new(&decoded);
    if !is_markdown(path) {
        return LinkTarget::Unsupported;
    }
    if path.is_absolute() {
        return LinkTarget::Markdown(path.to_path_buf());
    }
    match base_dir {
        Some(dir) => LinkTarget::Markdown(dir.join(path)),
        None => LinkTarget::Unsupported,
    }
}

/// `true` si la ruta tiene extensión Markdown.
#[must_use]
pub fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        MARKDOWN_EXTENSIONS
            .iter()
            .any(|m| m.eq_ignore_ascii_case(e))
    })
}

/// Esquema tipo `mailto:`, `file:`, `ftp:`. Una letra seguida de `:` (unidad de Windows) no cuenta.
fn has_scheme(url: &str) -> bool {
    match url.find(':') {
        Some(i) if i > 1 => {
            let scheme = &url[..i];
            scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        _ => false,
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[allow(clippy::unnecessary_wraps)]
    fn base() -> Option<&'static Path> {
        Some(Path::new("/docs"))
    }

    #[test]
    fn http_and_https_are_external() {
        assert_eq!(
            classify("https://example.com/a?b=1#c", base()),
            LinkTarget::External("https://example.com/a?b=1#c".into())
        );
        assert!(matches!(
            classify("HTTP://x.org", None),
            LinkTarget::External(_)
        ));
    }

    #[test]
    fn relative_markdown_resolves_against_base() {
        assert_eq!(
            classify("guia/otro.md", base()),
            LinkTarget::Markdown(PathBuf::from("/docs/guia/otro.md"))
        );
        assert_eq!(
            classify("../README.MD#intro", base()),
            LinkTarget::Markdown(PathBuf::from("/docs/../README.MD"))
        );
        assert_eq!(
            classify("mi%20nota.md", base()),
            LinkTarget::Markdown(PathBuf::from("/docs/mi nota.md"))
        );
    }

    #[test]
    fn unsupported_targets() {
        assert_eq!(classify("#seccion", base()), LinkTarget::Unsupported);
        assert_eq!(classify("mailto:a@b.c", base()), LinkTarget::Unsupported);
        assert_eq!(
            classify("file:///etc/passwd", base()),
            LinkTarget::Unsupported
        );
        assert_eq!(classify("imagen.png", base()), LinkTarget::Unsupported);
        assert_eq!(classify("", base()), LinkTarget::Unsupported);
        // Sin documento guardado no hay directorio base para resolver.
        assert_eq!(classify("otro.md", None), LinkTarget::Unsupported);
    }
}
