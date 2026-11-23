//! Pegar desde el portapapeles (US-4.4): una imagen se guarda en `assets/` junto al documento
//! y se inserta su enlace Markdown; si no hay imagen, se pega el texto de siempre.

use mdx_core::assets::{self, SavedImage};
use mdx_core::error::CoreError;

use super::{App, DocumentId, Message, Task, strings};
use crate::clipboard::{self, ClipboardImage};

impl App {
    /// Mensajes del pegado desde el portapapeles.
    pub(super) fn update_paste(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PasteRequested => {
                Task::perform(clipboard::read_image_async(), Message::ClipboardImage)
            }
            Message::ClipboardImage(Some(image)) => self.save_pasted_image(image),
            Message::ClipboardImage(None) => iced::clipboard::read().map(Message::PasteText),
            Message::PasteText(Some(text)) => {
                // Los saltos de línea del documento siempre son `\n`.
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                if text.is_empty() {
                    Task::none()
                } else {
                    self.apply_text_edit(&text)
                }
            }
            Message::ImageSaved(id, result) => self.image_saved(id, result),
            _ => Task::none(),
        }
    }

    fn save_pasted_image(&mut self, image: ClipboardImage) -> Task<Message> {
        let Some(dir) = self.doc.path.as_deref().and_then(std::path::Path::parent) else {
            self.set_status(strings::STATUS_PASTE_NEEDS_SAVE.to_owned(), true);
            return Task::none();
        };
        let id = self.doc.id;
        Task::perform(
            assets::save_clipboard_image_async(
                dir.to_path_buf(),
                image.width,
                image.height,
                image.rgba,
            ),
            move |result| Message::ImageSaved(id, result),
        )
    }

    fn image_saved(
        &mut self,
        id: DocumentId,
        result: Result<SavedImage, CoreError>,
    ) -> Task<Message> {
        match result {
            Ok(saved) => {
                self.set_status(
                    format!("{}: {}", strings::STATUS_IMAGE_SAVED, saved.relative),
                    false,
                );
                let markdown = format!("![]({})", saved.relative);
                self.with_doc(id, |app| app.apply_text_edit(&markdown))
                    .unwrap_or_else(Task::none)
            }
            Err(err) => {
                self.set_status(err.to_string(), true);
                Task::none()
            }
        }
    }
}
