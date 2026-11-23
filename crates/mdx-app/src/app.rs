//! Estado, mensajes, `update` y `view` de la aplicación (arquitectura Elm).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::widget::{Column, Row, container, markdown, pane_grid, scrollable, text, text_editor};
use iced::{
    Element, Event, Font, Length, Subscription, Task, Theme, event, highlighter, keyboard, time,
    window,
};
use mdx_core::document::count_words;
use mdx_core::history::History;
use mdx_core::io::SavedInfo;
use mdx_core::link::{self, LinkTarget};
use mdx_core::{CoreError, Document};

use crate::editor;
use crate::layout::{PaneKind, initial_panes, panes_without_preview};
use crate::shortcuts::{self, ShortcutAction};
use crate::statusbar::{Stats, display_col};
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
    history: History,
    /// Instante de la última edición fusionable; `None` corta la ráfaga actual.
    last_edit_at: Option<Instant>,
    words: usize,
    preview: Vec<markdown::Item>,
    /// Generación del último parseo lanzado; descarta resultados obsoletos.
    preview_gen: u64,
    preview_visible: bool,
    /// Hay cambios sin reflejar en la vista previa (activa el temporizador de debounce).
    preview_pending: bool,
    preview_edit_at: Instant,
}

/// Espera tras la última edición antes de re-parsear la vista previa (US-1.4 AC1).
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);
/// Frecuencia de comprobación del debounce mientras hay cambios pendientes.
const PREVIEW_TICK: Duration = Duration::from_millis(25);

/// Tiempo máximo entre pulsaciones para que cuenten como un solo paso de deshacer.
const MERGE_WINDOW: Duration = Duration::from_millis(1000);

/// Acciones que descartarían el documento actual; se protegen si hay cambios sin guardar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAction {
    Close,
    New,
    Open,
    OpenPath(PathBuf),
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
    PreviewTick(Instant),
    PreviewParsed(u64, Vec<markdown::Item>),
    LinkClicked(String),
    LinkOpened(Result<(), String>),
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
                history: History::new(),
                last_edit_at: None,
                words: 0,
                preview: Vec::new(),
                preview_gen: 0,
                preview_visible: true,
                preview_pending: false,
                preview_edit_at: Instant::now(),
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
            if self.preview_pending && self.preview_visible {
                time::every(PREVIEW_TICK).map(Message::PreviewTick)
            } else {
                Subscription::none()
            },
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
                self.handle_edit(action);
                Task::none()
            }
            Message::Shortcut(action) => self.handle_shortcut(action),
            Message::FilePicked(Some(path)) => Task::perform(
                async move { mdx_core::io::read_document(&path).await },
                Message::DocumentLoaded,
            ),
            Message::FilePicked(None) => Task::none(),
            Message::DocumentLoaded(result) => self.handle_loaded(result),
            Message::SavePicked(Some(path)) => self.save_to(path),
            Message::SavePicked(None) => {
                self.after_save = None;
                Task::none()
            }
            Message::Saved(result) => self.handle_saved(result),
            Message::CloseRequested => self.request(PendingAction::Close),
            Message::PreviewTick(now) => {
                if self.preview_pending
                    && now.saturating_duration_since(self.preview_edit_at) >= PREVIEW_DEBOUNCE
                {
                    return self.start_preview_parse();
                }
                Task::none()
            }
            Message::PreviewParsed(generation, items) => {
                if generation == self.preview_gen {
                    self.preview = items;
                }
                Task::none()
            }
            Message::LinkClicked(url) => self.handle_link(&url),
            Message::LinkOpened(result) => {
                if let Err(err) = result {
                    self.set_status(err, true);
                }
                Task::none()
            }
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
            ShortcutAction::Undo => {
                self.undo_redo(true);
                Task::none()
            }
            ShortcutAction::Redo => {
                self.undo_redo(false);
                Task::none()
            }
            ShortcutAction::TogglePreview => self.toggle_preview(),
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
            Message::GuardAnswered(action.clone(), choice)
        })
    }

    fn run(&mut self, action: PendingAction) -> Task<Message> {
        match action {
            PendingAction::Close => iced::exit(),
            PendingAction::New => {
                self.doc = Document::untitled();
                self.content = text_editor::Content::new();
                let task = self.reset_editing_state();
                self.set_status(strings::STATUS_NEW.to_owned(), false);
                task
            }
            PendingAction::Open => Task::perform(pick_file_to_open(), Message::FilePicked),
            PendingAction::OpenPath(path) => Task::perform(
                async move { mdx_core::io::read_document(&path).await },
                Message::DocumentLoaded,
            ),
        }
    }

    fn handle_edit(&mut self, action: text_editor::Action) {
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

    fn undo_redo(&mut self, undo: bool) {
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
    fn reset_editing_state(&mut self) -> Task<Message> {
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

    /// Parsea el Markdown en un hilo aparte para no bloquear la UI con documentos grandes
    /// (NFR-4). El resultado llega como `PreviewParsed` y se ignora si ya quedó obsoleto.
    fn start_preview_parse(&mut self) -> Task<Message> {
        self.preview_gen += 1;
        self.preview_pending = false;
        let generation = self.preview_gen;
        let text = self.doc.text.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || markdown::parse(&text).collect::<Vec<_>>())
                    .await
                    .unwrap_or_default()
            },
            move |items| Message::PreviewParsed(generation, items),
        )
    }

    fn toggle_preview(&mut self) -> Task<Message> {
        self.preview_visible = !self.preview_visible;
        if self.preview_visible {
            self.panes = initial_panes();
            self.start_preview_parse()
        } else {
            self.panes = panes_without_preview();
            Task::none()
        }
    }

    fn handle_link(&mut self, url: &str) -> Task<Message> {
        let base = self.doc.path.as_deref().and_then(Path::parent);
        match link::classify(url, base) {
            LinkTarget::External(url) => {
                Task::perform(async move { open::that_detached(url) }, |result| {
                    Message::LinkOpened(result.map_err(|e| e.to_string()))
                })
            }
            LinkTarget::Markdown(path) => self.request(PendingAction::OpenPath(path)),
            LinkTarget::Unsupported => {
                self.set_status(strings::STATUS_LINK_UNSUPPORTED.replace("{url}", url), true);
                Task::none()
            }
        }
    }

    fn refresh_dirty(&mut self) {
        self.doc.text = self.content.text();
        self.refresh_stats();
    }

    /// Recalcula dirty y palabras a partir de `doc.text` (ya sincronizado con el editor).
    fn refresh_stats(&mut self) {
        self.preview_pending = true;
        self.preview_edit_at = Instant::now();
        self.dirty = self.doc.is_dirty();
        self.words = count_words(&self.doc.text);
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

    fn handle_loaded(&mut self, result: Result<Document, CoreError>) -> Task<Message> {
        match result {
            Ok(doc) => {
                self.content = text_editor::Content::with_text(&doc.text);
                let name = doc.display_name(strings::UNTITLED);
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

    fn stats(&self) -> Stats {
        let (line, byte_col) = editor::cursor_pos(&self.content);
        let col = self
            .content
            .line(line)
            .map_or(1, |l| display_col(&l.text, byte_col));
        Stats {
            line: line + 1,
            col,
            words: self.words,
            line_ending: self.doc.line_ending,
            has_bom: self.doc.has_bom,
        }
    }

    fn set_status(&mut self, text: String, is_error: bool) {
        self.status = Some(Status { text, is_error });
    }

    pub fn view(&self) -> Element<'_, Message> {
        let theme = self.theme();
        let grid = pane_grid(&self.panes, |_, kind, _| {
            let body: Element<'_, Message> = match kind {
                PaneKind::Explorer => text(strings::PANE_EXPLORER).into(),
                PaneKind::Editor => text_editor(&self.content)
                    .on_action(Message::Edit)
                    .key_binding(editor::key_binding)
                    .font(Font::MONOSPACE)
                    .highlight("markdown", highlighter::Theme::InspiredGitHub)
                    .height(Length::Fill)
                    .into(),
                PaneKind::Preview => {
                    scrollable(markdown::view(&self.preview, &theme).map(Message::LinkClicked))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                }
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
        let status_bar = container(
            Row::new().push(status).push(
                container(text(self.stats().render()))
                    .width(Length::Fill)
                    .align_right(Length::Fill),
            ),
        )
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
    fn undo_and_redo_typing_burst() {
        // AC3: una ráfaga de tecleo es un solo paso.
        let (mut app, _) = App::new();
        for c in "hola".chars() {
            type_char(&mut app, c);
        }
        let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
        assert_eq!(app.content.text(), "");
        assert_eq!(app.title(), "Sin título — Mdx");
        let _ = app.update(Message::Shortcut(ShortcutAction::Redo));
        assert_eq!(app.content.text(), "hola");
        assert_eq!(app.title(), "● Sin título — Mdx");
    }

    #[test]
    fn undo_restores_cursor_and_edit_after_undo_drops_redo() {
        let (mut app, _) = App::new();
        for c in "ab".chars() {
            type_char(&mut app, c);
        }
        let _ = app.update(Message::Edit(Action::Edit(Edit::Enter)));
        type_char(&mut app, 'c');
        let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
        assert_eq!(app.content.text(), "ab\n");
        let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
        assert_eq!(app.content.text(), "ab");
        assert_eq!(editor::cursor_pos(&app.content), (0, 2));
        type_char(&mut app, 'z');
        let _ = app.update(Message::Shortcut(ShortcutAction::Redo));
        assert_eq!(app.content.text(), "abz");
    }

    #[test]
    fn indent_and_unindent_selection() {
        // AC4: con la línea seleccionada, Tab la indenta y Shift+Tab la desindenta.
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        let _ = app.update(Message::Edit(Action::SelectAll));
        let _ = app.update(Message::Edit(Action::Edit(Edit::Indent)));
        let indented = app.content.text();
        assert!(indented.ends_with('a') && indented.len() > 1 && indented.trim() == "a");
        let _ = app.update(Message::Edit(Action::Edit(Edit::Unindent)));
        assert_eq!(app.content.text(), "a");
    }

    #[test]
    fn stats_follow_cursor_and_words() {
        // AC5
        let (mut app, _) = App::new();
        for c in "uno dos".chars() {
            type_char(&mut app, c);
        }
        let _ = app.update(Message::Edit(Action::Edit(Edit::Enter)));
        type_char(&mut app, 'ñ');
        let stats = app.stats();
        assert_eq!((stats.line, stats.col, stats.words), (2, 2, 3));
    }

    /// Simula la llegada del resultado del parseo en segundo plano.
    fn deliver_preview(app: &mut App) {
        let items = markdown::parse(&app.doc.text).collect();
        let _ = app.update(Message::PreviewParsed(app.preview_gen, items));
    }

    #[test]
    fn preview_updates_only_after_debounce() {
        // AC1: el re-parseo se lanza ≥150 ms después de la última edición.
        let (mut app, _) = App::new();
        for c in "# T".chars() {
            type_char(&mut app, c);
        }
        assert!(app.preview_pending);
        let launched = app.preview_gen;
        let edited = app.preview_edit_at;
        let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(100)));
        assert!(app.preview_pending, "aún dentro del debounce");
        assert_eq!(app.preview_gen, launched);
        let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(150)));
        assert!(!app.preview_pending);
        assert_eq!(app.preview_gen, launched + 1);
        assert!(app.preview.is_empty());
        deliver_preview(&mut app);
        assert!(!app.preview.is_empty());
    }

    #[test]
    fn stale_preview_results_are_discarded() {
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        let edited = app.preview_edit_at;
        let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(150)));
        let stale = app.preview_gen - 1;
        let items = markdown::parse("# viejo").collect();
        let _ = app.update(Message::PreviewParsed(stale, items));
        assert!(app.preview.is_empty());
    }

    #[test]
    fn loaded_document_starts_preview_parse() {
        let doc = Document::from_bytes(Path::new("a.md"), b"# Hola\n\ntexto", None).unwrap();
        let (mut app, _) = App::new();
        let before = app.preview_gen;
        let _ = app.update(Message::DocumentLoaded(Ok(doc)));
        assert_eq!(app.preview_gen, before + 1);
        assert!(!app.preview_pending);
        deliver_preview(&mut app);
        assert!(!app.preview.is_empty());
    }

    #[test]
    fn toggle_preview_hides_and_restores_pane() {
        // AC4
        let (mut app, _) = App::new();
        assert_eq!(app.panes.len(), 3);
        let _ = app.update(Message::Shortcut(ShortcutAction::TogglePreview));
        assert!(!app.preview_visible);
        assert_eq!(app.panes.len(), 2);
        let _ = app.update(Message::Shortcut(ShortcutAction::TogglePreview));
        assert_eq!(app.panes.len(), 3);
    }

    #[test]
    fn unsupported_link_reports_error_and_keeps_document() {
        // AC3
        let (mut app, _) = App::new();
        type_char(&mut app, 'a');
        let _ = app.update(Message::LinkClicked("mailto:a@b.c".into()));
        assert!(app.status.as_ref().is_some_and(|s| s.is_error));
        assert_eq!(app.content.text(), "a");
    }

    /// NFR-4: coste por edición con documentos grandes (`cargo test --workspace -- --ignored`).
    /// Ejecutar con `--release` para cifras representativas; el umbral es holgado para debug.
    #[test]
    #[ignore = "rendimiento: requiere fixtures/large_*.md (cargo xtask gen-fixtures)"]
    fn perf_edit_cost_on_large_documents() {
        const N: u128 = 20;
        for (name, budget_ms) in [("large_1mb.md", 50u128), ("large_5mb.md", 250u128)] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name);
            let bytes = std::fs::read(&path).expect("ejecuta `cargo xtask gen-fixtures`");
            let doc = Document::from_bytes(&path, &bytes, None).unwrap();
            let (mut app, _) = App::new();
            let t = Instant::now();
            let _ = app.update(Message::DocumentLoaded(Ok(doc)));
            let load_ms = t.elapsed().as_millis();

            // Medir el coste de una pulsación (editar + diff + hash + palabras).
            let mut worst = 0u128;
            let mut total = 0u128;
            for _ in 0..N {
                let t = Instant::now();
                type_char(&mut app, 'x');
                let ms = t.elapsed().as_millis();
                worst = worst.max(ms);
                total += ms;
            }
            // Coste del parseo de la vista previa (corre fuera del hilo de UI en la app real).
            let t = Instant::now();
            let items: Vec<_> = markdown::parse(&app.doc.text).collect();
            let preview_ms = t.elapsed().as_millis();
            drop(items);
            // Deshacer.
            let t = Instant::now();
            let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
            let undo_ms = t.elapsed().as_millis();
            eprintln!(
                "PERF {name}: carga={load_ms} ms, edición media={} ms (peor {worst} ms), \
                 parseo preview (hilo aparte)={preview_ms} ms, deshacer={undo_ms} ms",
                total / N
            );
            assert!(
                total / N <= budget_ms,
                "{name}: edición media {} ms > {budget_ms} ms",
                total / N
            );
        }
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
