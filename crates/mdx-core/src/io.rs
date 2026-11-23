//! Lectura y escritura de documentos en disco.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::document::{Document, LineEnding, encode_text, hash_text};
use crate::error::CoreError;

/// Resultado de un guardado exitoso.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedInfo {
    pub path: PathBuf,
    /// Hash del texto normalizado que se escribió.
    pub saved_hash: u64,
    pub mtime: Option<SystemTime>,
}

/// Lee `path` y lo convierte en [`Document`].
///
/// # Errors
/// [`CoreError::NotFound`], [`CoreError::PermissionDenied`] o [`CoreError::Io`] según el fallo de
/// E/S; [`CoreError::InvalidEncoding`] si el contenido no es UTF-8.
pub async fn read_document(path: &Path) -> Result<Document, CoreError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| CoreError::from_io(&e, path))?;
    let mtime = tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|m| m.modified().ok());
    Document::from_bytes(path, &bytes, mtime)
}

/// Escribe `bytes` en `path` de forma atómica: archivo temporal en el mismo directorio,
/// `fsync` y `rename`. Si algo falla, el destino original queda intacto.
///
/// # Errors
/// Errores de E/S convertidos a [`CoreError`].
pub async fn write_atomic(path: &Path, bytes: Vec<u8>) -> Result<(), CoreError> {
    let target = path.to_path_buf();
    tokio::task::spawn_blocking(move || write_atomic_sync(&target, &bytes, || Ok(())))
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?
}

/// Guarda `text` (normalizado a `\n`) en `path` aplicando `line_ending` y BOM.
///
/// # Errors
/// Ver [`write_atomic`].
pub async fn save_text(
    path: &Path,
    text: &str,
    line_ending: LineEnding,
    has_bom: bool,
) -> Result<SavedInfo, CoreError> {
    let bytes = encode_text(text, line_ending, has_bom);
    write_atomic(path, bytes).await?;
    let mtime = tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|m| m.modified().ok());
    Ok(SavedInfo {
        path: path.to_path_buf(),
        saved_hash: hash_text(text),
        mtime,
    })
}

/// Implementación síncrona; `before_rename` permite simular un fallo a mitad de la operación.
fn write_atomic_sync(
    path: &Path,
    bytes: &[u8],
    before_rename: impl FnOnce() -> std::io::Result<()>,
) -> Result<(), CoreError> {
    let io_err = |e: std::io::Error| CoreError::from_io(&e, path);
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.mdx-tmp-{}", std::process::id()));

    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        before_rename()?;
        fs::rename(&tmp, path)
    })();

    if let Err(e) = result {
        // Solo borramos nuestro propio temporal, nunca archivos del usuario.
        let _ = fs::remove_file(&tmp);
        return Err(io_err(e));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn atomic_failure_keeps_original() {
        // INV-2
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, b"original").unwrap();

        let res = write_atomic_sync(&path, b"nuevo contenido largo", || {
            Err(std::io::Error::other("fallo simulado"))
        });

        assert!(res.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        let leftovers: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "no debe quedar el temporal");
    }

    #[tokio::test]
    async fn write_atomic_replaces_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, b"viejo").unwrap();
        write_atomic(&path, b"nuevo".to_vec()).await.unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"nuevo");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn write_atomic_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nuevo.md");
        write_atomic(&path, b"x".to_vec()).await.unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"x");
    }

    #[tokio::test]
    async fn roundtrip_preserves_crlf_and_bom() {
        // INV-1 / AC6
        for original in [&b"a\nb\n"[..], b"a\r\nb\r\n", b"\xEF\xBB\xBFa\r\nb"] {
            let dir = TempDir::new().unwrap();
            let path = dir.path().join("r.md");
            fs::write(&path, original).unwrap();
            let doc = read_document(&path).await.unwrap();
            save_text(&path, &doc.text, doc.line_ending, doc.has_bom)
                .await
                .unwrap();
            assert_eq!(fs::read(&path).unwrap(), original);
        }
    }

    #[tokio::test]
    async fn edited_crlf_file_stays_crlf() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("e.md");
        fs::write(&path, b"uno\r\ndos").unwrap();
        let doc = read_document(&path).await.unwrap();
        let info = save_text(&path, "uno\ndos\ntres", doc.line_ending, doc.has_bom)
            .await
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"uno\r\ndos\r\ntres");
        assert_eq!(info.saved_hash, hash_text("uno\ndos\ntres"));
    }

    #[tokio::test]
    async fn non_utf8_is_clear_error() {
        // AC7
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bin.md");
        fs::write(&path, b"\xFF\xFE\x00bad").unwrap();
        let err = read_document(&path).await.unwrap_err();
        assert_eq!(err, CoreError::InvalidEncoding(path));
    }

    #[tokio::test]
    async fn missing_file_is_not_found() {
        let dir = TempDir::new().unwrap();
        let err = read_document(&dir.path().join("no.md")).await.unwrap_err();
        assert!(matches!(err, CoreError::NotFound(_)));
    }
}
