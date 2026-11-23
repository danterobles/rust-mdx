//! Edición y guardado del documento activo.

use super::{
    App, CoreError, DiskState, Document, GuardChoice, Instant, MERGE_WINDOW, Message, Path,
    PathBuf, SavedInfo, Task, count_words, editor, strings, text_editor,
};

impl App {
    pub(super) fn handle_edit(&mut self, action: text_editor::Action) {
        if !action.is_edit() {
            // Mover el cursor o seleccionar corta la ráfaga de tecleo (el scroll no).
            if !matches!(action, text_editor::Action::Scroll { .. }) {
                self.last_edit_at = None;
            }
            self.content.perform(action);
            return;
        }
        let before = editor::cursor_pos(&self.content);
        let mergeable = editor::is_mergeable(&action)
            && self
                .last_edit_at
                .is_some_and(|t| t.elapsed() < MERGE_WINDOW);
        self.content.perform(action.clone());
        let old = std::mem::take(&mut self.doc.text);
        let after = editor::cursor_pos(&self.content);
        self.doc.text = self.content.text();
        self.history
            .record(&old, &self.doc.text, before, after, mergeable);
        self.last_edit_at = editor::is_mergeable(&action).then(Instant::now);
        self.refresh_stats();
    }

    pub(super) fn undo_redo(&mut self, undo: bool) {
        self.last_edit_at = None;
        let patch = if undo {
            self.history.undo(&self.doc.text)
        } else {
            self.history.redo(&self.doc.text)
        };
        if let Some(patch) = patch {
            editor::apply_patch(&mut self.content, &self.doc.text, &patch);
            self.refresh_dirty();
        }
    }

    /// Sincroniza `doc.text` con el editor y recalcula dirty y palabras.
    /// Tras abrir o crear un documento: sin historial ni ráfaga activa.
    pub(super) fn reset_editing_state(&mut self) -> Task<Message> {
        self.history.clear();
        self.last_edit_at = None;
        self.dirty = false;
        self.words = count_words(&self.doc.text);
        self.preview_pending = false;
        if self.preview_visible {
            self.start_preview_parse()
        } else {
            Task::none()
        }
    }

    pub(super) fn refresh_dirty(&mut self) {
        self.doc.text = self.content.text();
        self.refresh_stats();
    }

    /// Recalcula dirty y palabras a partir de `doc.text` (ya sincronizado con el editor).
    pub(super) fn refresh_stats(&mut self) {
        self.preview_pending = true;
        self.preview_edit_at = Instant::now();
        self.dirty = self.doc.is_dirty();
        self.words = count_words(&self.doc.text);
    }

    pub(super) fn save(&self) -> Task<Message> {
        match self.doc.path.clone() {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    pub(super) fn save_as(&self) -> Task<Message> {
        let suggested = self.doc.display_name(&format!("{}.md", strings::UNTITLED));
        let start_dir = self
            .doc
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        Task::perform(pick_file_to_save(suggested, start_dir), Message::SavePicked)
    }

    pub(super) fn save_to(&self, path: PathBuf) -> Task<Message> {
        let text = self.content.text();
        let (line_ending, has_bom) = (self.doc.line_ending, self.doc.has_bom);
        Task::perform(
            async move { mdx_core::io::save_text(&path, &text, line_ending, has_bom).await },
            Message::Saved,
        )
    }

    pub(super) fn handle_loaded(&mut self, result: Result<Document, CoreError>) -> Task<Message> {
        match result {
            Ok(doc) => {
                self.content = text_editor::Content::with_text(&doc.text);
                let name = doc.display_name(strings::UNTITLED);
                self.track_tab(&doc);
                self.doc = doc;
                let task = self.reset_editing_state();
                self.set_status(format!("{}: {name}", strings::STATUS_OPENED), false);
                task
            }
            // AC7 (US-1.1): el archivo no se abre y el documento actual queda intacto.
            Err(err) => {
                self.set_status(err.to_string(), true);
                Task::none()
            }
        }
    }

    pub(super) fn handle_saved(&mut self, result: Result<SavedInfo, CoreError>) -> Task<Message> {
        match result {
            Ok(info) => {
                self.doc.path = Some(info.path);
                self.doc.saved_hash = Some(info.saved_hash);
                self.doc.disk_mtime = info.mtime;
                self.doc.disk_state = DiskState::InSync;
                self.external = None;
                // Se compara con el contenido actual: si se editó durante el guardado, sigue sucio.
                self.refresh_dirty();
                let name = self.doc.display_name(strings::UNTITLED);
                self.set_status(format!("{}: {name}", strings::STATUS_SAVED), false);
                match self.after_save.take() {
                    Some(action) => self.run(action),
                    None => Task::none(),
                }
            }
            Err(err) => {
                self.after_save = None;
                self.set_status(err.to_string(), true);
                Task::none()
            }
        }
    }

    /// Registra el documento recién abierto en el modelo de pestañas (INV-7).
    pub(super) fn track_tab(&mut self, doc: &Document) {
        // Con un solo documento en memoria, abrir otro siempre reemplaza al actual; las pestañas
        // visibles y los varios documentos abiertos llegan con US-3.1.
        self.tabs.close(self.doc.id);
        let pin = self
            .pin_on_load
            .take()
            .is_some_and(|p| doc.path.as_deref() == Some(p.as_path()));
        if pin {
            self.tabs.open_pinned(doc.id);
        } else {
            self.tabs.open_preview(doc.id);
        }
    }
}

pub(super) async fn pick_file_to_open() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_OPEN_TITLE)
        .add_filter(strings::FILTER_MARKDOWN, &["md", "markdown", "txt"])
        .add_filter(strings::FILTER_ALL, &["*"])
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}

pub(super) async fn pick_file_to_save(suggested: String, dir: Option<PathBuf>) -> Option<PathBuf> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_SAVE_TITLE)
        .set_file_name(suggested)
        .add_filter(strings::FILTER_MARKDOWN, &["md", "markdown", "txt"]);
    if let Some(dir) = dir {
        dialog = dialog.set_directory(dir);
    }
    dialog.save_file().await.map(|h| h.path().to_path_buf())
}

pub(super) async fn ask_unsaved(name: String) -> GuardChoice {
    let result = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(strings::UNSAVED_TITLE)
        .set_description(strings::UNSAVED_BODY.replace("{name}", &name))
        .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
            strings::UNSAVED_SAVE.to_owned(),
            strings::UNSAVED_DISCARD.to_owned(),
            strings::UNSAVED_CANCEL.to_owned(),
        ))
        .show()
        .await;
    match result {
        rfd::MessageDialogResult::Yes => GuardChoice::Save,
        rfd::MessageDialogResult::No => GuardChoice::Discard,
        rfd::MessageDialogResult::Custom(label) if label == strings::UNSAVED_SAVE => {
            GuardChoice::Save
        }
        rfd::MessageDialogResult::Custom(label) if label == strings::UNSAVED_DISCARD => {
            GuardChoice::Discard
        }
        _ => GuardChoice::Cancel,
    }
}
