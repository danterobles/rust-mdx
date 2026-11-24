//! Imágenes de un documento: resolución de rutas relativas y guardado de imágenes pegadas
//! (US-4.4).

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::CoreError;

/// Carpeta, junto al documento, donde se guardan las imágenes pegadas.
pub const ASSETS_DIR: &str = "assets";
const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];

/// `true` si la extensión corresponde a una imagen que la vista previa sabe dibujar.
#[must_use]
pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// `true` para `.svg` (se dibuja con el widget vectorial).
#[must_use]
pub fn is_svg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

/// Ruta local de la imagen `url` de un documento ubicado en `base_dir`.
///
/// Devuelve `None` para direcciones remotas (`http:`, `https:`, `data:`…), URLs sin ruta o
/// extensiones que no son de imagen. Quita `?consulta` y `#ancla` y decodifica `%20`.
#[must_use]
pub fn resolve_image(url: &str, base_dir: Option<&Path>) -> Option<PathBuf> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let has_scheme = url.find(':').is_some_and(|i| {
        i > 1
            && url[..i]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    if url.is_empty() || has_scheme || lower.starts_with("//") {
        return None;
    }
    let without_extra = url.split(['#', '?']).next().unwrap_or("");
    let decoded = percent_decode(without_extra);
    let path = Path::new(&decoded);
    if !is_supported_image(path) {
        return None;
    }
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    base_dir.map(|dir| dir.join(path))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len() + 1
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

/// Fecha `(año, mes, día)` en UTC de un instante, sin depender de ninguna biblioteca de fechas.
#[must_use]
pub fn civil_date(secs_since_epoch: u64) -> (i64, u32, u32) {
    // Algoritmo de Howard Hinnant (`civil_from_days`).
    let days = i64::try_from(secs_since_epoch / 86_400).unwrap_or(0) + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// `AAAAMMDD` (UTC) del instante dado.
#[must_use]
pub fn date_stamp(at: SystemTime) -> String {
    let secs = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_date(secs);
    format!("{y:04}{m:02}{d:02}")
}

/// Primer nombre libre `<fecha>-<n>.png` (n = 1, 2, …) dado el contenido de la carpeta.
#[must_use]
pub fn next_asset_name(existing: &[String], stamp: &str) -> String {
    let taken = |name: &str| existing.iter().any(|e| e.eq_ignore_ascii_case(name));
    (1..=u32::MAX)
        .map(|n| format!("{stamp}-{n}.png"))
        .find(|name| !taken(name))
        .unwrap_or_else(|| format!("{stamp}-1.png"))
}

/// Codifica píxeles RGBA de `width × height` como PNG.
///
/// # Errors
/// [`CoreError::Io`] si las dimensiones no concuerdan con los datos o falla la codificación.
pub fn encode_png(width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>, CoreError> {
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| CoreError::Io("la imagen del portapapeles está incompleta".to_owned()))?;
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| CoreError::Io(e.to_string()))?;
    Ok(out.into_inner())
}

/// Imagen pegada ya guardada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedImage {
    /// Ruta absoluta del archivo.
    pub path: PathBuf,
    /// Ruta relativa al documento, la que va en el Markdown (`assets/20261123-1.png`).
    pub relative: String,
}

/// Guarda una imagen RGBA del portapapeles como `assets/<fecha>-<n>.png` dentro de `doc_dir`.
///
/// Crea la carpeta si hace falta y nunca sobrescribe un archivo existente.
///
/// # Errors
/// [`CoreError`] si no se puede crear la carpeta, codificar o escribir.
pub fn save_clipboard_image(
    doc_dir: &Path,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    now: SystemTime,
) -> Result<SavedImage, CoreError> {
    let png = encode_png(width, height, rgba)?;
    let dir = doc_dir.join(ASSETS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::from_io(&e, &dir))?;
    let existing: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| CoreError::from_io(&e, &dir))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    let name = next_asset_name(&existing, &date_stamp(now));
    let path = dir.join(&name);
    // `create_new`: ante una carrera con otro proceso, falla en vez de pisar un archivo.
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| CoreError::from_io(&e, &path))?;
        file.write_all(&png)
            .map_err(|e| CoreError::from_io(&e, &path))?;
        file.sync_all().map_err(|e| CoreError::from_io(&e, &path))?;
    }
    Ok(SavedImage {
        path,
        relative: format!("{ASSETS_DIR}/{name}"),
    })
}

/// Versión asíncrona de [`save_clipboard_image`] (corre fuera del hilo de UI).
///
/// # Errors
/// Ver [`save_clipboard_image`].
pub async fn save_clipboard_image_async(
    doc_dir: PathBuf,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> Result<SavedImage, CoreError> {
    tokio::task::spawn_blocking(move || {
        save_clipboard_image(&doc_dir, width, height, rgba, SystemTime::now())
    })
    .await
    .map_err(|e| CoreError::Io(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn relative_images_resolve_against_the_document_directory() {
        // AC1
        let base = Some(Path::new("/docs"));
        assert_eq!(
            resolve_image("img/a.png", base),
            Some(PathBuf::from("/docs/img/a.png"))
        );
        assert_eq!(
            resolve_image("./a.JPG", base),
            Some(PathBuf::from("/docs/./a.JPG"))
        );
        assert_eq!(
            resolve_image("../b.svg#x?y=1", base),
            Some(PathBuf::from("/docs/../b.svg"))
        );
        assert_eq!(
            resolve_image("mi%20foto.png", base),
            Some(PathBuf::from("/docs/mi foto.png"))
        );
        assert_eq!(
            resolve_image("/abs/c.webp", base),
            Some(PathBuf::from("/abs/c.webp"))
        );
        assert_eq!(
            resolve_image("assets/20261123-1.png", base),
            Some(PathBuf::from("/docs/assets/20261123-1.png"))
        );
    }

    #[test]
    fn remote_unsupported_and_unresolvable_images_are_none() {
        let base = Some(Path::new("/docs"));
        for url in [
            "https://example.com/a.png",
            "http://x/a.png",
            "data:image/png;base64,AAAA",
            "file:///etc/a.png",
            "//cdn/a.png",
            "documento.md",
            "sin-extension",
            "",
        ] {
            assert_eq!(resolve_image(url, base), None, "{url}");
        }
        // Sin directorio base (documento sin guardar) no se puede resolver una ruta relativa.
        assert_eq!(resolve_image("a.png", None), None);
        // Una ruta absoluta no necesita directorio base (en Windows lleva letra de unidad).
        let absolute = if cfg!(windows) {
            r"C:\abs\a.png"
        } else {
            "/abs/a.png"
        };
        assert!(resolve_image(absolute, None).is_some());
    }

    #[test]
    fn svg_detection() {
        assert!(is_svg(Path::new("a.SVG")) && !is_svg(Path::new("a.png")));
    }

    #[test]
    fn dates_are_computed_in_utc_without_a_date_library() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(951_782_400), (2000, 2, 29), "bisiesto");
        assert_eq!(civil_date(1_700_000_000), (2023, 11, 14));
        assert_eq!(civil_date(4_102_444_800), (2100, 1, 1));
        assert_eq!(date_stamp(at(1_700_000_000)), "20231114");
        assert_eq!(date_stamp(at(0)), "19700101");
    }

    #[test]
    fn asset_names_take_the_first_free_number() {
        let none: Vec<String> = Vec::new();
        assert_eq!(next_asset_name(&none, "20261123"), "20261123-1.png");
        let some = vec![
            "20261123-1.png".to_owned(),
            "20261123-3.png".to_owned(),
            "otra.png".to_owned(),
            "20261122-1.png".to_owned(),
        ];
        assert_eq!(next_asset_name(&some, "20261123"), "20261123-2.png");
        assert_eq!(next_asset_name(&some, "20261124"), "20261124-1.png");
    }

    fn rgba(w: u32, h: u32) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| [u8::try_from(i % 256).unwrap(), 10, 20, 255])
            .collect()
    }

    #[test]
    fn png_encoding_roundtrips_the_pixels() {
        let png = encode_png(3, 2, rgba(3, 2)).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!((decoded.width(), decoded.height()), (3, 2));
        assert_eq!(decoded.into_raw(), rgba(3, 2));
    }

    #[test]
    fn png_encoding_rejects_inconsistent_dimensions() {
        assert!(matches!(
            encode_png(10, 10, vec![0; 8]),
            Err(CoreError::Io(_))
        ));
    }

    #[test]
    fn pasted_images_are_saved_next_to_the_document_without_overwriting() {
        // AC2
        let dir = TempDir::new().unwrap();
        let now = at(1_700_000_000);
        let first = save_clipboard_image(dir.path(), 2, 2, rgba(2, 2), now).unwrap();
        assert_eq!(first.relative, "assets/20231114-1.png");
        assert_eq!(first.path, dir.path().join("assets/20231114-1.png"));
        assert!(first.path.is_file());
        let second = save_clipboard_image(dir.path(), 2, 2, rgba(2, 2), now).unwrap();
        assert_eq!(second.relative, "assets/20231114-2.png");
        // Otro día: vuelve a empezar en 1 y el primero sigue intacto.
        let next_day =
            save_clipboard_image(dir.path(), 2, 2, rgba(2, 2), at(1_700_000_000 + 86_400)).unwrap();
        assert_eq!(next_day.relative, "assets/20231115-1.png");
        assert_eq!(
            std::fs::read_dir(dir.path().join("assets"))
                .unwrap()
                .count(),
            3
        );
        let on_disk = image::open(&first.path).unwrap().to_rgba8();
        assert_eq!(on_disk.into_raw(), rgba(2, 2));
    }

    #[test]
    fn saving_fails_clearly_when_the_directory_is_not_writable() {
        let dir = TempDir::new().unwrap();
        let blocker = dir.path().join("assets");
        std::fs::write(&blocker, "soy un archivo, no una carpeta").unwrap();
        let err = save_clipboard_image(dir.path(), 1, 1, rgba(1, 1), at(0)).unwrap_err();
        assert!(matches!(
            err,
            CoreError::Io(_) | CoreError::AlreadyExists(_) | CoreError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn the_async_wrapper_saves_with_the_current_date() {
        let dir = TempDir::new().unwrap();
        let saved = save_clipboard_image_async(dir.path().to_path_buf(), 1, 1, rgba(1, 1))
            .await
            .unwrap();
        assert!(saved.relative.starts_with("assets/") && saved.relative.ends_with("-1.png"));
    }
}
