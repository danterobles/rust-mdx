//! Documento en memoria: texto normalizado a `\n`, final de línea y BOM originales.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::error::CoreError;

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Identificador estable de un documento durante la vida del proceso.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(u64);

impl DocumentId {
    /// Genera un identificador nuevo (contador atómico, no se persiste).
    #[must_use]
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Final de línea del archivo original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

/// Relación del documento con el disco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskState {
    InSync,
    ChangedExternally,
    Deleted,
}

/// Documento abierto.
#[derive(Clone, Debug)]
pub struct Document {
    pub id: DocumentId,
    /// `None` = «Sin título».
    pub path: Option<PathBuf>,
    /// Contenido normalizado a `\n`.
    pub text: String,
    pub line_ending: LineEnding,
    /// El archivo original empezaba con BOM UTF-8.
    pub has_bom: bool,
    /// Hash (xxh3) del texto normalizado tal como está en disco.
    pub saved_hash: Option<u64>,
    pub disk_mtime: Option<SystemTime>,
    pub disk_state: DiskState,
}

/// Hash de contenido usado para el estado «sin guardar».
#[must_use]
pub fn hash_text(text: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(text.as_bytes())
}

/// Codifica texto normalizado (`\n`) a los bytes que se escriben en disco.
#[must_use]
pub fn encode_text(text: &str, line_ending: LineEnding, has_bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + if has_bom { BOM.len() } else { 0 });
    if has_bom {
        out.extend_from_slice(BOM);
    }
    match line_ending {
        LineEnding::Lf => out.extend_from_slice(text.as_bytes()),
        LineEnding::CrLf => out.extend_from_slice(text.replace('\n', "\r\n").as_bytes()),
    }
    out
}

impl Document {
    /// Documento vacío «Sin título».
    #[must_use]
    pub fn untitled() -> Self {
        Self {
            id: DocumentId::next(),
            path: None,
            text: String::new(),
            line_ending: LineEnding::Lf,
            has_bom: false,
            saved_hash: None,
            disk_mtime: None,
            disk_state: DiskState::InSync,
        }
    }

    /// Construye un documento a partir de los bytes de `path`.
    ///
    /// Detecta BOM y final de línea (`CrLf` si hay al menos un `\r\n`) y normaliza el texto a `\n`.
    ///
    /// # Errors
    /// [`CoreError::InvalidEncoding`] si los bytes no son UTF-8 válido.
    pub fn from_bytes(
        path: &Path,
        bytes: &[u8],
        mtime: Option<SystemTime>,
    ) -> Result<Self, CoreError> {
        let (has_bom, body) = match bytes.strip_prefix(BOM) {
            Some(rest) => (true, rest),
            None => (false, bytes),
        };
        let raw = std::str::from_utf8(body)
            .map_err(|_| CoreError::InvalidEncoding(path.to_path_buf()))?;
        let line_ending = if raw.contains("\r\n") {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        };
        let text = match line_ending {
            LineEnding::Lf => raw.to_owned(),
            LineEnding::CrLf => raw.replace("\r\n", "\n"),
        };
        let saved_hash = Some(hash_text(&text));
        Ok(Self {
            id: DocumentId::next(),
            path: Some(path.to_path_buf()),
            text,
            line_ending,
            has_bom,
            saved_hash,
            disk_mtime: mtime,
            disk_state: DiskState::InSync,
        })
    }

    /// Nombre para mostrar: nombre de archivo o «Sin título».
    #[must_use]
    pub fn display_name(&self, untitled: &str) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| untitled.to_owned(), |n| n.to_string_lossy().into_owned())
    }

    /// `true` si el texto difiere de lo guardado. Un documento sin ruta y vacío no está sucio.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        match self.saved_hash {
            Some(h) => hash_text(&self.text) != h,
            None => !self.text.is_empty(),
        }
    }

    /// Bytes que se escribirían en disco para el texto actual.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        encode_text(&self.text, self.line_ending, self.has_bom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(bytes: &[u8]) -> Document {
        Document::from_bytes(Path::new("t.md"), bytes, None).expect("utf-8 válido")
    }

    #[test]
    fn detects_lf_and_crlf() {
        assert_eq!(open(b"a\nb\n").line_ending, LineEnding::Lf);
        let crlf = open(b"a\r\nb\r\n");
        assert_eq!(crlf.line_ending, LineEnding::CrLf);
        assert_eq!(crlf.text, "a\nb\n");
    }

    #[test]
    fn detects_bom() {
        let d = open(b"\xEF\xBB\xBFhola");
        assert!(d.has_bom);
        assert_eq!(d.text, "hola");
    }

    #[test]
    fn roundtrip_is_byte_identical() {
        // INV-1
        for bytes in [
            &b"a\nb\n"[..],
            b"a\r\nb\r\n",
            b"\xEF\xBB\xBFa\nb",
            b"\xEF\xBB\xBFa\r\nb\r\n",
            b"sin salto final",
            b"",
        ] {
            assert_eq!(open(bytes).to_bytes(), bytes);
        }
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let err = Document::from_bytes(Path::new("x.md"), b"\xFF\xFEabc", None).unwrap_err();
        assert_eq!(err, CoreError::InvalidEncoding(PathBuf::from("x.md")));
    }

    #[test]
    fn dirty_state() {
        let mut d = open(b"hola");
        assert!(!d.is_dirty());
        d.text.push('!');
        assert!(d.is_dirty());
        assert!(!Document::untitled().is_dirty());
        let mut u = Document::untitled();
        u.text.push('x');
        assert!(u.is_dirty());
    }

    #[test]
    fn untitled_display_name() {
        assert_eq!(
            Document::untitled().display_name("Sin título"),
            "Sin título"
        );
        assert_eq!(open(b"").display_name("Sin título"), "t.md");
    }
}
