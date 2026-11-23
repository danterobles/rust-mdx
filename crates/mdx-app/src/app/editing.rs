//! Edición y guardado del documento activo.

use super::{
    App, CoreError, DiskState, Document, DocumentId, GuardChoice, Instant, MERGE_WINDOW, Message,
    Path, PathBuf, SavedInfo, Task, count_words, editor, strings, text_editor,
};

/// Parches de deshacer/rehacer mayores que esto se aplican reconstruyendo el editor.
const BULK_PATCH_BYTES: usize = 64 * 1024;

impl App {
    pub(super) fn handle_edit(&mut self, action: text_editor::Action) -> Task<Message> {
        if !action.is_edit() {
            // Mover el cursor o seleccionar corta la ráfaga de tecleo (el scroll no).
            let scrolled = matches!(action, text_editor::Action::Scroll { .. });
            let moved = matches!(action, text_editor::Action::Move(_));
            if !scrolled {
                self.last_edit_at = None;
            }
            self.content.perform(action);
            // La rueda y el movimiento con teclado pueden desplazar la vista del editor.
            return if scrolled || moved {
                self.update_view_top()
            } else {
                Task::none()
            };
        }
        // Editar fija la pestaña «preview» (así nunca se reemplaza una con cambios).
        self.tabs.pin(self.doc.id);
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
        // Con la barra de búsqueda abierta, las coincidencias siguen al texto.
        self.refresh_find_matches();
        // Escribir puede desplazar el editor hasta el cursor.
        self.update_view_top()
    }

    /// Mueve el cursor al inicio de `line` (base 0) y lo deja visible (clic en el esquema).
    pub(super) fn go_to_line(&mut self, line: usize) -> Task<Message> {
        let line = line.min(self.content.line_count().saturating_sub(1));
        self.last_edit_at = None;
        self.content.move_to(text_editor::Cursor {
            position: text_editor::Position { line, column: 0 },
            selection: None,
        });
        // Una acción de movimiento hace que el editor desplace la vista hasta el cursor.
        self.content
            .perform(text_editor::Action::Move(text_editor::Motion::Home));
        self.update_view_top()
    }

    pub(super) fn undo_redo(&mut self, undo: bool) {
        self.last_edit_at = None;
        let patch = if undo {
            self.history.undo(&self.doc.text)
        } else {
            self.history.redo(&self.doc.text)
        };
        if let Some(patch) = patch {
            if patch.replacement.len().max(patch.end - patch.start) > BULK_PATCH_BYTES {
                let (start, end) = (
                    patch.start.min(self.doc.text.len()),
                    patch.end.min(self.doc.text.len()),
                );
                let rebuilt = format!(
                    "{}{}{}",
                    &self.doc.text[..start],
                    patch.replacement,
                    &self.doc.text[end..]
                );
                self.rebuild_editor(&rebuilt, patch.cursor);
            } else {
                editor::apply_patch(&mut self.content, &self.doc.text, &patch);
            }
            self.refresh_dirty();
        }
    }

    /// Reemplaza todo el contenido del editor y coloca el cursor en `cursor`.
    fn rebuild_editor(&mut self, text: &str, cursor: (usize, usize)) {
        self.content = text_editor::Content::with_text(text);
        self.content.move_to(text_editor::Cursor {
            position: text_editor::Position {
                line: cursor.0,
                column: cursor.1,
            },
            selection: None,
        });
    }

    /// Sustituye el texto completo como **una** edición (historial, «sin guardar», vista
    /// previa) reconstruyendo el editor. Pegar megabytes de golpe tarda segundos; reconstruir,
    /// una fracción (medido: 1 MB ≈ 11 s pegando frente a ≈ 0,7 s reconstruyendo).
    pub(super) fn apply_bulk_edit(
        &mut self,
        new_text: &str,
        cursor: (usize, usize),
    ) -> Task<Message> {
        self.tabs.pin(self.doc.id);
        let before = editor::cursor_pos(&self.content);
        let old = std::mem::take(&mut self.doc.text);
        self.rebuild_editor(new_text, cursor);
        self.doc.text = self.content.text();
        self.history
            .record(&old, &self.doc.text, before, cursor, false);
        self.last_edit_at = None;
        self.refresh_stats();
        self.refresh_find_matches();
        self.update_view_top()
    }

    /// Sincroniza `doc.text` con el editor y recalcula dirty y palabras.
    /// Tras abrir o crear un documento: sin historial ni ráfaga activa.
    pub(super) fn reset_editing_state(&mut self) -> Task<Message> {
        self.history.clear();
        self.last_edit_at = None;
        self.dirty = false;
        self.words = count_words(&self.doc.text);
        self.preview_pending = false;
        self.refresh_find_matches();
        // Se parsea siempre: el esquema lo necesita aunque la vista previa esté oculta.
        self.start_preview_parse()
    }

    pub(super) fn refresh_dirty(&mut self) {
        self.doc.text = self.content.text();
        self.refresh_stats();
    }

    /// Recalcula dirty y palabras a partir de `doc.text` (ya sincronizado con el editor).
    pub(super) fn refresh_stats(&mut self) {
        self.preview_pending = true;
        self.preview_edit_at = Instant::now();
        self.last_change = Instant::now();
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
        let id = self.doc.id;
        Task::perform(
            async move { mdx_core::io::save_text(&path, &text, line_ending, has_bom).await },
            move |result| Message::Saved(id, result),
        )
    }

    pub(super) fn handle_loaded(&mut self, result: Result<Document, CoreError>) -> Task<Message> {
        match result {
            Ok(doc) => {
                let pinned = self
                    .pin_on_load
                    .take()
                    .is_some_and(|p| doc.path.as_deref() == Some(p.as_path()));
                let name = doc.display_name(strings::UNTITLED);
                let opened_path = doc.path.clone();
                if let Some(path) = &opened_path {
                    self.note_recent_file(path);
                }
                let task = self.add_doc(doc, pinned);
                self.set_status(format!("{}: {name}", strings::STATUS_OPENED), false);
                // Resultado de la búsqueda en carpeta: posicionar el editor en la coincidencia.
                match self.pending_goto.take() {
                    Some((path, line, cols)) if opened_path.as_deref() == Some(path.as_path()) => {
                        Task::batch([task, self.goto_match(line, &cols)])
                    }
                    _ => task,
                }
            }
            // AC7 (US-1.1): el archivo no se abre y los documentos abiertos quedan intactos.
            Err(err) => {
                self.pending_goto = None;
                self.set_status(err.to_string(), true);
                Task::none()
            }
        }
    }

    /// Aplica el resultado de un guardado al documento `id` (no tiene por qué ser el activo)
    /// y, si salió bien, continúa con la acción pendiente (cerrar pestaña o aplicación).
    pub(super) fn handle_saved(
        &mut self,
        id: DocumentId,
        result: Result<SavedInfo, CoreError>,
    ) -> Task<Message> {
        self.autosave_inflight.remove(&id);
        let saved_path = result.as_ref().ok().map(|info| info.path.clone());
        let saved = self
            .with_doc(id, |app| app.apply_saved(result))
            .unwrap_or(false);
        if !saved {
            self.after_save = None;
            return Task::none();
        }
        // Guardar `config.toml` aplica la configuración al instante.
        let reload = saved_path.map_or_else(Task::none, |p| self.reload_config_if_saved(&p));
        match self.after_save.take() {
            Some(action) => Task::batch([reload, self.request(action)]),
            None => reload,
        }
    }

    /// Actualiza el documento activo con el resultado de guardar. Devuelve si salió bien.
    fn apply_saved(&mut self, result: Result<SavedInfo, CoreError>) -> bool {
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
                true
            }
            Err(err) => {
                // Un fallo (p. ej. autoguardado sin permisos) no debe reintentarse en bucle.
                self.last_change = Instant::now();
                self.set_status(err.to_string(), true);
                false
            }
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
