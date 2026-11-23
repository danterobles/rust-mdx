//! Estado, mensajes, `update` y `view` de la aplicación (arquitectura Elm).

use std::path::{Path, PathBuf};

use iced::widget::{Column, container, pane_grid, text, text_editor};
use iced::{Element, Event, Length, Subscription, Task, Theme, event, keyboard, window};
use mdx_core::io::SavedInfo;
use mdx_core::{CoreError, Document};

use crate::layout::{PaneKind, initial_panes};
use crate::shortcuts::{self, ShortcutAction};
use crate::strings;

pub struct App {
    panes: pane_grid::State<PaneKind>,
    doc: Document,
    content: text_editor::Content,
    status: Option<Status>,
    /// Caché de `doc.is_dirty()`: se recalcula al editar, guardar o cambiar de documento.
    dirty: bool,
    /// Acción a ejecutar cuando termine el guardado en curso (cierre, nuevo, abrir).
    after_save: Option<PendingAction>,
}

/// Acciones que descartarían el documento actual; se protegen si hay cambios sin guardar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAction {
    Close,
    New,
    Open,
}

/// Respuesta del usuario al diálogo de cambios sin guardar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardChoice {
    Save,
    Discard,
    Cancel,
}

struct Status {
    text: String,
    is_error: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    PaneResized(pane_grid::ResizeEvent),
    Edit(text_editor::Action),
    Shortcut(ShortcutAction),
    FilePicked(Option<PathBuf>),
    DocumentLoaded(Result<Document, CoreError>),
    SavePicked(Option<PathBuf>),
    Saved(Result<SavedInfo, CoreError>),
    CloseRequested,
    GuardAnswered(PendingAction, GuardChoice),
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                panes: initial_panes(),
                doc: Document::untitled(),
                content: text_editor::Content::new(),
                status: None,
                dirty: false,
                after_save: None,
            },
            Task::none(),
        )
    }

    pub fn title(&self) -> String {
        let mark = if self.dirty { strings::DIRTY_MARK } else { "" };
        format!(
            "{mark}{} — {}",
            self.doc.display_name(strings::UNTITLED),
            strings::APP_TITLE
        )
    }

    #[allow(clippy::unused_self)]
    pub fn theme(&self) -> Theme {
        Theme::Light
    }

    #[allow(clippy::unused_self)]
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            event::listen_with(|event, _status, _window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    ..
                }) => shortcuts::map(&key, physical_key, modifiers).map(Message::Shortcut),
                _ => None,
            }),
            window::close_requests().map(|_| Message::CloseRequested),
        ])
    }

    // iced exige `Message` por valor; crecerá con las próximas historias.
    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
                Task::none()
            }
            Message::Edit(action) => {
                let is_edit = action.is_edit();
                self.content.perform(action);
                if is_edit {
                    self.refresh_dirty();
                }
                Task::none()
            }
            Message::Shortcut(action) => self.handle_shortcut(action),
            Message::FilePicked(Some(path)) => Task::perform(
                async move { mdx_core::io::read_document(&path).await },
                Message::DocumentLoaded,
            ),
            Message::FilePicked(None) => Task::none(),
            Message::DocumentLoaded(result) => {
                self.handle_loaded(result);
                Task::none()
            }
            Message::SavePicked(Some(path)) => self.save_to(path),
            Message::SavePicked(None) => {
                self.after_save = None;
                Task::none()
            }
            Message::Saved(result) => self.handle_saved(result),
            Message::CloseRequested => self.request(PendingAction::Close),
            Message::GuardAnswered(action, choice) => match choice {
                GuardChoice::Save => {
                    self.after_save = Some(action);
                    self.save()
                }
                GuardChoice::Discard => self.run(action),
                GuardChoice::Cancel => Task::none(),
            },
        }
    }

    fn handle_shortcut(&mut self, action: ShortcutAction) -> Task<Message> {
        match action {
            ShortcutAction::New => self.request(PendingAction::New),
            ShortcutAction::Open => self.request(PendingAction::Open),
            ShortcutAction::Save => self.save(),
            ShortcutAction::SaveAs => self.save_as(),
        }
    }

    fn save(&self) -> Task<Message> {
        match self.doc.path.clone() {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    /// Ejecuta `action`, o pregunta primero si hay cambios sin guardar.
    fn request(&mut self, action: PendingAction) -> Task<Message> {
        if !self.dirty {
            return self.run(action);
        }
        let name = self.doc.display_name(strings::UNTITLED);
        Task::perform(ask_unsaved(name), move |choice| {
            Message::GuardAnswered(action, choice)
        })
    }

    fn run(&mut self, action: PendingAction) -> Task<Message> {
        match action {
            PendingAction::Close => iced::exit(),
            PendingAction::New => {
                self.doc = Document::untitled();
                self.content = text_editor::Content::new();
                self.dirty = false;
                self.set_status(strings::STATUS_NEW.to_owned(), false);
                Task::none()
            }
            PendingAction::Open => Task::perform(pick_file_to_open(), Message::FilePicked),
        }
    }

    fn refresh_dirty(&mut self) {
        self.doc.text = self.content.text();
        self.dirty = self.doc.is_dirty();
    }

    fn save_as(&self) -> Task<Message> {
        let suggested = self.doc.display_name(&format!("{}.md", strings::UNTITLED));
        let start_dir = self
            .doc
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        Task::perform(pick_file_to_save(suggested, start_dir), Message::SavePicked)
    }

    fn save_to(&self, path: PathBuf) -> Task<Message> {
        let text = self.content.text();
        let (line_ending, has_bom) = (self.doc.line_ending, self.doc.has_bom);
        Task::perform(
            async move { mdx_core::io::save_text(&path, &text, line_ending, has_bom).await },
            Message::Saved,
        )
    }

    fn handle_loaded(&mut self, result: Result<Document, CoreError>) {
        match result {
            Ok(doc) => {
                self.content = text_editor::Content::with_text(&doc.text);
                let name = doc.display_name(strings::UNTITLED);
                self.doc = doc;
                self.dirty = false;
                self.set_status(format!("{}: {name}", strings::STATUS_OPENED), false);
            }
            // AC7 (US-1.1): el archivo no se abre y el documento actual queda intacto.
            Err(err) => self.set_status(err.to_string(), true),
        }
    }

    fn handle_saved(&mut self, result: Result<SavedInfo, CoreError>) -> Task<Message> {
        match result {
            Ok(info) => {
                self.doc.path = Some(info.path);
                self.doc.saved_hash = Some(info.saved_hash);
                self.doc.disk_mtime = info.mtime;
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

    fn set_status(&mut self, text: String, is_error: bool) {
        self.status = Some(Status { text, is_error });
    }

    pub fn view(&self) -> Element<'_, Message> {
        let grid = pane_grid(&self.panes, |_, kind, _| {
            let body: Element<'_, Message> = match kind {
                PaneKind::Explorer => text(strings::PANE_EXPLORER).into(),
                PaneKind::Editor => text_editor(&self.content)
                    .on_action(Message::Edit)
                    .height(Length::Fill)
                    .into(),
                PaneKind::Preview => text(strings::PANE_PREVIEW).into(),
            };
            pane_grid::Content::new(
                container(body)
                    .padding(8)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
        })
        .spacing(4)
        .on_resize(6, Message::PaneResized);

        let status: Element<'_, Message> = match &self.status {
            Some(s) if s.is_error => text(&s.text).style(text::danger).into(),
            Some(s) => text(&s.text).into(),
            None => text("").into(),
        };
        let status_bar = container(status)
            .padding([4, 8])
            .width(Length::Fill)
            .style(container::bordered_box);

        Column::new().push(grid).push(status_bar).into()
    }
}

async fn pick_file_to_open() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_OPEN_TITLE)
        .add_filter(strings::FILTER_MARKDOWN, &["md", "markdown", "txt"])
        .add_filter(strings::FILTER_ALL, &["*"])
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn pick_file_to_save(suggested: String, dir: Option<PathBuf>) -> Option<PathBuf> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_SAVE_TITLE)
        .set_file_name(suggested)
        .add_filter(strings::FILTER_MARKDOWN, &["md", "markdown", "txt"]);
    if let Some(dir) = dir {
        dialog = dialog.set_directory(dir);
    }
    dialog.save_file().await.map(|h| h.path().to_path_buf())
}

async fn ask_unsaved(name: String) -> GuardChoice {
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

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::text_editor::{Action, Edit};

    fn type_char(app: &mut App, c: char) {
        let _ = app.update(Message::Edit(Action::Edit(Edit::Insert(c))));
    }

    #[test]
    fn starts_clean_and_untitled() {
        let (app, _) = App::new();
        assert_eq!(app.title(), "Sin título — Mdx");
    }

    #[test]
    fn editing_marks_dirty_in_title() {
        // AC1
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        assert_eq!(app.title(), "● Sin título — Mdx");
    }

    #[test]
    fn returning_to_saved_state_clears_indicator() {
        // AC3: borrar lo escrito devuelve el hash al del estado guardado.
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        let _ = app.update(Message::Edit(Action::Edit(Edit::Backspace)));
        assert_eq!(app.title(), "Sin título — Mdx");
    }

    #[test]
    fn saved_content_is_clean_and_new_edit_is_dirty_again() {
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        let info = SavedInfo {
            path: PathBuf::from("/tmp/x.md"),
            saved_hash: mdx_core::document::hash_text("a"),
            mtime: None,
        };
        let _ = app.update(Message::Saved(Ok(info)));
        assert_eq!(app.title(), "x.md — Mdx");
        type_char(&mut app, 'b');
        assert_eq!(app.title(), "● x.md — Mdx");
    }

    #[test]
    fn discard_runs_new_and_cancel_keeps_document() {
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        let _ = app.update(Message::GuardAnswered(
            PendingAction::New,
            GuardChoice::Cancel,
        ));
        assert_eq!(app.content.text(), "a");
        let _ = app.update(Message::GuardAnswered(
            PendingAction::New,
            GuardChoice::Discard,
        ));
        assert_eq!(app.content.text(), "");
        assert_eq!(app.title(), "Sin título — Mdx");
    }

    #[test]
    fn cancelling_save_dialog_clears_pending_action() {
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        let _ = app.update(Message::GuardAnswered(
            PendingAction::Close,
            GuardChoice::Save,
        ));
        assert_eq!(app.after_save, Some(PendingAction::Close));
        let _ = app.update(Message::SavePicked(None));
        assert_eq!(app.after_save, None);
    }
}
