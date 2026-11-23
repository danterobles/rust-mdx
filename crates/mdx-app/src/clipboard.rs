//! Portapapeles del sistema para imágenes (US-4.4). El texto se pega con el portapapeles de iced.

/// Imagen RGBA leída del portapapeles.
#[derive(Clone)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for ClipboardImage {
    // No se vuelcan megabytes de píxeles en los registros ni en los mensajes de depuración.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ClipboardImage({}x{}, {} bytes)",
            self.width,
            self.height,
            self.rgba.len()
        )
    }
}

/// Lee una imagen del portapapeles, si hay una. Bloqueante: usar fuera del hilo de UI.
#[must_use]
pub fn read_image() -> Option<ClipboardImage> {
    let image = arboard::Clipboard::new().ok()?.get_image().ok()?;
    Some(ClipboardImage {
        width: u32::try_from(image.width).ok()?,
        height: u32::try_from(image.height).ok()?,
        rgba: image.bytes.into_owned(),
    })
}

/// [`read_image`] en un hilo aparte.
pub async fn read_image_async() -> Option<ClipboardImage> {
    tokio::task::spawn_blocking(read_image).await.ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_does_not_dump_pixels() {
        let image = ClipboardImage {
            width: 2,
            height: 1,
            rgba: vec![0; 8],
        };
        assert_eq!(format!("{image:?}"), "ClipboardImage(2x1, 8 bytes)");
    }
}
