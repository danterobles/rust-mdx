//! Estado, mensajes, `update` y `view` de la aplicación (arquitectura Elm).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::{SinkExt, Stream, StreamExt};
use iced::widget::{
    Column, Row, button, container, markdown, pane_grid, scrollable, text, text_editor,
};
use iced::{
    Element, Event, Font, Length, Subscription, Task, Theme, event, highlighter, keyboard, time,
    window,
};
use mdx_core::document::count_words;
use mdx_core::fileops;
use mdx_core::history::History;
use mdx_core::io::SavedInfo;
use mdx_core::link::{self, LinkTarget};
use mdx_core::watcher::{self, FsEvent};
use mdx_core::workspace::{self, FileNode, TreeFilters, Workspace};
use mdx_core::{CoreError, DiskState, Document, DocumentId};

use crate::editor;
use crate::explorer::{
    self, ContextAction, ContextTarget, ExplorerKey, ExplorerState, Nav, PROMPT_ID, Prompt,
    PromptKind,
};
use crate::layout::{PaneKind, initial_panes, panes_without_preview};
use crate::shortcuts::{self, ShortcutAction};
use crate::state::tabs::Tabs;
use crate::statusbar::{Stats, display_col};
use crate::strings;

pub struct App {
    explorer: ExplorerState,
    tabs: Tabs,
    /// Versión del disco pendiente de resolver cuando hay conflicto (banner).
    external: Option<Document>,
    /// Ruta cuyo doble clic (o apertura explícita) llegó mientras se cargaba: queda fijada.
    pin_on_load: Option<PathBuf>,
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

/// Elección del usuario ante un conflicto con el disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    /// Descarta los cambios locales y carga la versión del disco.
    Reload,
    /// Conserva el texto local; el disco pasa a ser la referencia de «guardado».
    KeepMine,
}

/// Resultado de una operación de archivos.
#[derive(Debug, Clone)]
pub enum OpOutcome {
    Created(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
    Trashed(PathBuf),
    Revealed,
}

/// Filtros del explorador que el usuario puede alternar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Hidden,
    Gitignore,
    OnlyMarkdown,
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
    FolderPicked(Option<PathBuf>),
    WorkspaceOpened(Result<Workspace, CoreError>),
    ToggleDir(PathBuf),
    DirLoaded(PathBuf, Result<Vec<FileNode>, CoreError>),
    ToggleExplorerMenu,
    FileClicked(PathBuf),
    FilePinned(PathBuf),
    ExplorerKey(ExplorerKey),
    OpenContext(ContextTarget),
    CloseContext,
    ContextAction(ContextAction),
    PromptChanged(String),
    PromptSubmit,
    PromptCancel,
    DeleteAnswered(PathBuf, bool),
    OpDone(Result<OpOutcome, CoreError>),
    ToggleFilter(FilterKind),
    Fs(Vec<FsEvent>),
    WatchFailed(String),
    DiskInspected(DocumentId, PathBuf, Result<Document, CoreError>),
    ResolveConflict(ConflictChoice),
    PreviewParsed(u64, Vec<markdown::Item>),
    LinkClicked(String),
    LinkOpened(Result<(), String>),
}

impl App {
    /// Crea la app. `arg` es la ruta opcional recibida por CLI (`mdx <ruta>`): una carpeta se
    /// abre como workspace y un archivo se abre en el editor.
    pub fn new(arg: Option<PathBuf>) -> (Self, Task<Message>) {
        let filters = TreeFilters::default();
        let doc = Document::untitled();
        let mut tabs = Tabs::new();
        tabs.open_pinned(doc.id);
        let mut pin_on_load = None;
        let boot = match arg {
            Some(path) if path.is_dir() => Task::perform(
                workspace::open_async(path, filters),
                Message::WorkspaceOpened,
            ),
            Some(path) => {
                // Apertura explícita por CLI: la pestaña queda fijada.
                pin_on_load = Some(path.clone());
                Task::perform(
                    async move { mdx_core::io::read_document(&path).await },
                    Message::DocumentLoaded,
                )
            }
            None => Task::none(),
        };
        (
            Self {
                explorer: ExplorerState {
                    workspace: None,
                    filters,
                    menu_open: false,
                    selected: None,
                    context: None,
                    prompt: None,
                },
                tabs,
                pin_on_load,
                external: None,
                panes: initial_panes(),
                doc,
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
            boot,
        )
    }

    pub fn title(&self) -> String {
        let mark = if self.dirty { strings::DIRTY_MARK } else { "" };
        let deleted = if self.doc.disk_state == DiskState::Deleted {
            strings::DELETED_MARK
        } else {
            ""
        };
        format!(
            "{mark}{}{deleted} — {}",
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
            event::listen_with(|event, status, _window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    ..
                }) => shortcuts::map(&key, physical_key, modifiers)
                    .map(Message::Shortcut)
                    .or_else(|| {
                        // Solo teclas que ningún widget (p. ej. el editor enfocado) consumió.
                        if status == event::Status::Ignored {
                            explorer::map_key(&key, modifiers).map(Message::ExplorerKey)
                        } else {
                            None
                        }
                    }),
                _ => None,
            }),
            window::close_requests().map(|_| Message::CloseRequested),
            self.watch_subscriptions(),
            if self.preview_pending && self.preview_visible {
                time::every(PREVIEW_TICK).map(Message::PreviewTick)
            } else {
                Subscription::none()
            },
        ])
    }

    /// Vigila la carpeta abierta (recursivo) y, si el documento está fuera de ella, su directorio.
    fn watch_subscriptions(&self) -> Subscription<Message> {
        let mut subs = Vec::new();
        let root = self.explorer.workspace.as_ref().map(|ws| ws.root.clone());
        if let Some(root) = &root {
            subs.push(Subscription::run_with(root.clone(), |root| {
                watcher_stream(root.clone(), true)
            }));
        }
        if let Some(dir) = self.doc.path.as_deref().and_then(Path::parent) {
            let inside = root.as_deref().is_some_and(|r| dir.starts_with(r));
            if !inside {
                subs.push(Subscription::run_with(dir.to_path_buf(), |dir| {
                    watcher_stream(dir.clone(), false)
                }));
            }
        }
        Subscription::batch(subs)
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
            Message::FilePicked(Some(path)) => {
                self.pin_on_load = Some(path.clone());
                Task::perform(
                    async move { mdx_core::io::read_document(&path).await },
                    Message::DocumentLoaded,
                )
            }
            Message::FilePicked(None) | Message::FolderPicked(None) => Task::none(),
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
            Message::Fs(events) => self.handle_fs(&events),
            Message::WatchFailed(err) => {
                self.set_status(format!("{}: {err}", strings::STATUS_WATCH_FAILED), true);
                Task::none()
            }
            Message::DiskInspected(id, path, result) => {
                self.handle_disk_inspected(id, &path, result)
            }
            Message::ResolveConflict(choice) => self.resolve_conflict(choice),
            Message::PreviewParsed(generation, items) => {
                if generation == self.preview_gen {
                    self.preview = items;
                }
                Task::none()
            }
            m @ (Message::FolderPicked(Some(_))
            | Message::WorkspaceOpened(_)
            | Message::ToggleDir(_)
            | Message::DirLoaded(..)
            | Message::FileClicked(_)
            | Message::FilePinned(_)
            | Message::ExplorerKey(_)
            | Message::OpenContext(_)
            | Message::CloseContext
            | Message::ContextAction(_)
            | Message::PromptChanged(_)
            | Message::PromptSubmit
            | Message::PromptCancel
            | Message::DeleteAnswered(..)
            | Message::OpDone(_)
            | Message::ToggleExplorerMenu
            | Message::ToggleFilter(_)) => self.update_explorer(m),
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

    /// Mensajes del explorador (árbol, menú contextual, operaciones de archivo).
    fn update_explorer(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FolderPicked(Some(path)) => Task::perform(
                workspace::open_async(path, self.explorer.filters),
                Message::WorkspaceOpened,
            ),
            Message::WorkspaceOpened(result) => {
                match result {
                    Ok(ws) => {
                        let msg = format!("{}: {}", strings::STATUS_FOLDER_OPENED, ws.tree.name);
                        self.explorer.workspace = Some(ws);
                        self.explorer.selected = None;
                        self.set_status(msg, false);
                    }
                    Err(err) => self.set_status(err.to_string(), true),
                }
                Task::none()
            }
            Message::ToggleDir(path) => self.toggle_dir(&path),
            Message::DirLoaded(path, result) => {
                if let Some(ws) = &mut self.explorer.workspace {
                    ws.set_children(&path, result);
                }
                Task::none()
            }
            Message::FileClicked(path) => self.open_from_explorer(path, false),
            Message::FilePinned(path) => self.open_from_explorer(path, true),
            Message::ExplorerKey(key) => self.explorer_key(key),
            Message::OpenContext(target) => {
                self.explorer.context = Some(target);
                self.explorer.prompt = None;
                Task::none()
            }
            Message::CloseContext => {
                self.explorer.context = None;
                Task::none()
            }
            Message::ContextAction(action) => self.context_action(action),
            Message::PromptChanged(value) => {
                if let Some(p) = &mut self.explorer.prompt {
                    p.value = value;
                    p.error = None;
                }
                Task::none()
            }
            Message::PromptSubmit => self.submit_prompt(),
            Message::PromptCancel => {
                self.explorer.prompt = None;
                Task::none()
            }
            Message::DeleteAnswered(path, true) => Task::perform(
                async move {
                    fileops::move_to_trash_async(path.clone())
                        .await
                        .map(|()| OpOutcome::Trashed(path))
                },
                Message::OpDone,
            ),
            Message::OpDone(result) => self.handle_op_done(result),
            Message::ToggleExplorerMenu => {
                self.explorer.menu_open = !self.explorer.menu_open;
                Task::none()
            }
            Message::ToggleFilter(kind) => self.toggle_filter(kind),
            _ => Task::none(),
        }
    }

    /// Cambios externos: refresca el árbol (AC1) y revisa el documento abierto (AC2–AC5).
    fn handle_fs(&mut self, events: &[FsEvent]) -> Task<Message> {
        let mut tasks = Vec::new();
        let filters = self.explorer.filters;
        if let Some(ws) = &self.explorer.workspace {
            for dir in explorer::dirs_to_reload(ws, events) {
                tasks.push(Task::perform(
                    workspace::load_children_async(dir.clone(), filters),
                    move |result| Message::DirLoaded(dir.clone(), result),
                ));
            }
        }
        if let Some(path) = self.doc.path.clone()
            && events.iter().any(|e| e.paths().contains(&path.as_path()))
        {
            let id = self.doc.id;
            tasks.push(Task::perform(
                async move {
                    let result = mdx_core::io::read_document(&path).await;
                    (id, path, result)
                },
                |(id, path, result)| Message::DiskInspected(id, path, result),
            ));
        }
        Task::batch(tasks)
    }

    /// Decide qué hacer con el documento abierto según lo que hay ahora en disco (DATA §3.2).
    fn handle_disk_inspected(
        &mut self,
        id: DocumentId,
        path: &Path,
        result: Result<Document, CoreError>,
    ) -> Task<Message> {
        // Resultado obsoleto: el documento o su ruta cambiaron mientras se leía.
        if id != self.doc.id || self.doc.path.as_deref() != Some(path) {
            return Task::none();
        }
        let disk = match result {
            Ok(disk) => disk,
            Err(CoreError::NotFound(_)) => {
                // AC4: se conserva el contenido para poder volver a guardarlo.
                if self.doc.disk_state != DiskState::Deleted {
                    self.doc.disk_state = DiskState::Deleted;
                    self.external = None;
                    self.set_status(strings::STATUS_DELETED_ON_DISK.to_owned(), true);
                }
                return Task::none();
            }
            Err(err) => {
                self.set_status(err.to_string(), true);
                return Task::none();
            }
        };
        // INV-3 / AC5: si el disco coincide con lo último guardado (p. ej. nuestro propio
        // guardado), no hay nada que hacer.
        if disk.saved_hash == self.doc.saved_hash {
            if self.doc.disk_state != DiskState::InSync {
                self.doc.disk_state = DiskState::InSync;
                self.external = None;
            }
            return Task::none();
        }
        if self.dirty {
            // AC3: hay cambios locales y el disco cambió → banner.
            let already = self
                .external
                .as_ref()
                .is_some_and(|e| e.saved_hash == disk.saved_hash);
            if !already {
                self.doc.disk_state = DiskState::ChangedExternally;
                self.external = Some(disk);
            }
            return Task::none();
        }
        // AC2: sin cambios locales, recarga silenciosa.
        self.apply_disk_version(disk)
    }

    /// Sustituye el contenido por la versión del disco (conserva id y ruta).
    fn apply_disk_version(&mut self, disk: Document) -> Task<Message> {
        self.content = text_editor::Content::with_text(&disk.text);
        self.doc.text = disk.text;
        self.doc.saved_hash = disk.saved_hash;
        self.doc.disk_mtime = disk.disk_mtime;
        self.doc.line_ending = disk.line_ending;
        self.doc.has_bom = disk.has_bom;
        self.doc.disk_state = DiskState::InSync;
        self.external = None;
        self.reset_editing_state()
    }

    fn resolve_conflict(&mut self, choice: ConflictChoice) -> Task<Message> {
        let Some(disk) = self.external.take() else {
            return Task::none();
        };
        match choice {
            ConflictChoice::Reload => {
                self.set_status(strings::STATUS_RELOADED.to_owned(), false);
                self.apply_disk_version(disk)
            }
            ConflictChoice::KeepMine => {
                // El disco pasa a ser la referencia: el texto local sigue «sin guardar» y el
                // siguiente guardado sobrescribe conscientemente la versión externa.
                self.doc.saved_hash = disk.saved_hash;
                self.doc.disk_mtime = disk.disk_mtime;
                self.doc.disk_state = DiskState::InSync;
                self.dirty = self.doc.is_dirty();
                Task::none()
            }
        }
    }

    fn handle_shortcut(&mut self, action: ShortcutAction) -> Task<Message> {
        match action {
            ShortcutAction::New => self.request(PendingAction::New),
            ShortcutAction::Open => self.request(PendingAction::Open),
            ShortcutAction::OpenFolder => Task::perform(pick_folder(), Message::FolderPicked),
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
                self.tabs.close(self.doc.id);
                self.doc = Document::untitled();
                self.tabs.open_pinned(self.doc.id);
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

    fn context_action(&mut self, action: ContextAction) -> Task<Message> {
        let Some(target) = self.explorer.context.take() else {
            return Task::none();
        };
        let new_prompt = |kind, dir: PathBuf, value: String| Prompt {
            kind,
            target: dir,
            value,
            error: None,
        };
        let focus = || iced::widget::operation::focus(iced::widget::Id::new(PROMPT_ID));
        match action {
            ContextAction::NewFile | ContextAction::NewFolder => {
                let dir = if target.is_dir {
                    target.path
                } else {
                    target
                        .path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or(target.path)
                };
                let kind = if action == ContextAction::NewFile {
                    PromptKind::NewFile
                } else {
                    PromptKind::NewFolder
                };
                self.explorer.prompt = Some(new_prompt(kind, dir, String::new()));
                focus()
            }
            ContextAction::Rename => {
                let name = target
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.explorer.prompt = Some(new_prompt(PromptKind::Rename, target.path, name));
                focus()
            }
            ContextAction::Delete => {
                let name = target
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let path = target.path;
                Task::perform(ask_delete(name), move |yes| {
                    Message::DeleteAnswered(path.clone(), yes)
                })
            }
            ContextAction::Reveal => Task::perform(
                async move {
                    fileops::reveal_async(target.path)
                        .await
                        .map(|()| OpOutcome::Revealed)
                },
                Message::OpDone,
            ),
        }
    }

    fn submit_prompt(&mut self) -> Task<Message> {
        let Some(prompt) = &mut self.explorer.prompt else {
            return Task::none();
        };
        // Validación en línea, sin tocar el disco (AC4).
        if let Err(err) = fileops::validate_name(&prompt.value) {
            prompt.error = Some(err.to_string());
            return Task::none();
        }
        let (target, value) = (prompt.target.clone(), prompt.value.clone());
        match prompt.kind {
            PromptKind::NewFile => Task::perform(
                async move {
                    fileops::create_file_async(target, value)
                        .await
                        .map(OpOutcome::Created)
                },
                Message::OpDone,
            ),
            PromptKind::NewFolder => Task::perform(
                async move {
                    fileops::create_dir_async(target, value)
                        .await
                        .map(OpOutcome::Created)
                },
                Message::OpDone,
            ),
            PromptKind::Rename => Task::perform(
                async move {
                    fileops::rename_async(target.clone(), value)
                        .await
                        .map(|to| OpOutcome::Renamed { from: target, to })
                },
                Message::OpDone,
            ),
        }
    }

    fn handle_op_done(&mut self, result: Result<OpOutcome, CoreError>) -> Task<Message> {
        match result {
            Ok(OpOutcome::Created(path)) => {
                self.explorer.prompt = None;
                self.explorer.selected = Some(path.clone());
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                self.set_status(
                    format!("{}: {}", strings::STATUS_CREATED, name.unwrap_or_default()),
                    false,
                );
                self.reload_parent(&path, true)
            }
            Ok(OpOutcome::Renamed { from, to }) => {
                self.explorer.prompt = None;
                // AC3: el documento abierto (o uno dentro de la carpeta renombrada) sigue su ruta.
                if let Some(new_path) = self
                    .doc
                    .path
                    .as_deref()
                    .and_then(|p| fileops::rebase_path(p, &from, &to))
                {
                    self.doc.path = Some(new_path);
                }
                self.explorer.selected = Some(to.clone());
                let name = to.file_name().map(|n| n.to_string_lossy().into_owned());
                self.set_status(
                    format!("{}: {}", strings::STATUS_RENAMED, name.unwrap_or_default()),
                    false,
                );
                self.reload_parent(&from, false)
            }
            Ok(OpOutcome::Trashed(path)) => {
                if self
                    .doc
                    .path
                    .as_deref()
                    .is_some_and(|p| p.starts_with(&path))
                {
                    // El contenido se conserva en el editor para poder volver a guardarlo.
                    self.doc.disk_state = mdx_core::DiskState::Deleted;
                }
                if self.explorer.selected.as_deref() == Some(path.as_path()) {
                    self.explorer.selected = None;
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                self.set_status(
                    format!("{}: {}", strings::STATUS_TRASHED, name.unwrap_or_default()),
                    false,
                );
                self.reload_parent(&path, false)
            }
            Ok(OpOutcome::Revealed) => Task::none(),
            Err(err) => {
                // Con la entrada de nombre abierta el error se muestra en línea (AC4).
                if let Some(prompt) = &mut self.explorer.prompt {
                    prompt.error = Some(err.to_string());
                } else {
                    self.set_status(err.to_string(), true);
                }
                Task::none()
            }
        }
    }

    /// Recarga los hijos del directorio que contiene `path` (y lo expande si `expand`).
    fn reload_parent(&mut self, path: &Path, expand: bool) -> Task<Message> {
        let filters = self.explorer.filters;
        let Some(parent) = path.parent().map(Path::to_path_buf) else {
            return Task::none();
        };
        if expand && let Some(ws) = &mut self.explorer.workspace {
            let _ = ws.begin_expand(&parent);
        }
        Task::perform(
            workspace::load_children_async(parent.clone(), filters),
            move |result| Message::DirLoaded(parent.clone(), result),
        )
    }

    /// Clic simple (preview) o doble clic (fija) sobre un archivo del árbol.
    fn open_from_explorer(&mut self, path: PathBuf, pin: bool) -> Task<Message> {
        self.explorer.selected = Some(path.clone());
        if self.doc.path.as_deref() == Some(path.as_path()) {
            // Ya está abierto: solo se fija si fue doble clic.
            if pin {
                self.tabs.pin(self.doc.id);
            }
            return Task::none();
        }
        if pin {
            self.pin_on_load = Some(path.clone());
        }
        self.request(PendingAction::OpenPath(path))
    }

    fn explorer_key(&mut self, key: ExplorerKey) -> Task<Message> {
        let Some(ws) = &self.explorer.workspace else {
            return Task::none();
        };
        let nav = explorer::navigate(&ws.visible_nodes(), self.explorer.selected.as_deref(), key);
        match nav {
            Nav::None => Task::none(),
            Nav::Select(path) => {
                self.explorer.selected = Some(path);
                Task::none()
            }
            Nav::Toggle(path) => self.toggle_dir(&path),
            Nav::Open(path) => self.open_from_explorer(path, false),
        }
    }

    /// Registra el documento recién abierto en el modelo de pestañas (INV-7).
    fn track_tab(&mut self, doc: &Document) {
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

    fn toggle_dir(&mut self, path: &Path) -> Task<Message> {
        let filters = self.explorer.filters;
        let Some(ws) = &mut self.explorer.workspace else {
            return Task::none();
        };
        let expanded = matches!(
            ws.node(path).map(|n| &n.kind),
            Some(mdx_core::workspace::NodeKind::Dir { expanded: true, .. })
        );
        if expanded {
            ws.collapse(path);
            return Task::none();
        }
        if ws.begin_expand(path) {
            let dir = path.to_path_buf();
            return Task::perform(
                workspace::load_children_async(dir.clone(), filters),
                move |result| Message::DirLoaded(dir.clone(), result),
            );
        }
        Task::none()
    }

    /// Cambia un filtro y recarga la raíz (el árbol vuelve a colapsarse).
    fn toggle_filter(&mut self, kind: FilterKind) -> Task<Message> {
        let filters = &mut self.explorer.filters;
        match kind {
            FilterKind::Hidden => filters.show_hidden = !filters.show_hidden,
            FilterKind::Gitignore => filters.respect_gitignore = !filters.respect_gitignore,
            FilterKind::OnlyMarkdown => filters.only_markdown = !filters.only_markdown,
        }
        let Some(ws) = &self.explorer.workspace else {
            return Task::none();
        };
        Task::perform(
            workspace::open_async(ws.root.clone(), self.explorer.filters),
            Message::WorkspaceOpened,
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

    fn handle_saved(&mut self, result: Result<SavedInfo, CoreError>) -> Task<Message> {
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

    fn conflict_banner() -> Element<'static, Message> {
        container(
            Row::new()
                .spacing(12)
                .align_y(iced::Alignment::Center)
                .push(text(strings::BANNER_CHANGED))
                .push(
                    button(text(strings::BANNER_RELOAD))
                        .on_press(Message::ResolveConflict(ConflictChoice::Reload)),
                )
                .push(
                    button(text(strings::BANNER_KEEP))
                        .style(button::secondary)
                        .on_press(Message::ResolveConflict(ConflictChoice::KeepMine)),
                ),
        )
        .padding([6, 10])
        .width(Length::Fill)
        .style(container::bordered_box)
        .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let theme = self.theme();
        let grid = pane_grid(&self.panes, |_, kind, _| {
            let body: Element<'_, Message> = match kind {
                PaneKind::Explorer => explorer::view(&self.explorer, self.doc.path.as_deref()),
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

        let mut layout = Column::new();
        if self.doc.disk_state == DiskState::ChangedExternally {
            layout = layout.push(Self::conflict_banner());
        }
        layout.push(grid).push(status_bar).into()
    }
}

/// Flujo de eventos del sistema de archivos para una subscripción de iced.
fn watcher_stream(path: PathBuf, recursive: bool) -> impl Stream<Item = Message> {
    iced::stream::channel(32, async move |mut output| {
        match watcher::watch(&path, recursive) {
            Ok(mut watcher) => {
                while let Some(batch) = watcher.next().await {
                    if output.send(Message::Fs(batch)).await.is_err() {
                        break;
                    }
                }
            }
            Err(err) => {
                let _ = output.send(Message::WatchFailed(err.to_string())).await;
            }
        }
        // El observador solo debe terminar al cancelarse la subscripción.
        std::future::pending::<()>().await;
    })
}

async fn ask_delete(name: String) -> bool {
    let result = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(strings::DELETE_TITLE)
        .set_description(strings::DELETE_BODY.replace("{name}", &name))
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            strings::DELETE_CONFIRM.to_owned(),
            strings::PROMPT_CANCEL.to_owned(),
        ))
        .show()
        .await;
    match result {
        rfd::MessageDialogResult::Ok => true,
        rfd::MessageDialogResult::Custom(label) => label == strings::DELETE_CONFIRM,
        _ => false,
    }
}

async fn pick_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_FOLDER_TITLE)
        .pick_folder()
        .await
        .map(|h| h.path().to_path_buf())
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
        let (app, _) = App::new(None);
        assert_eq!(app.title(), "Sin título — Mdx");
    }

    #[test]
    fn editing_marks_dirty_in_title() {
        // AC1
        let (mut app, _) = App::new(None);
        type_char(&mut app, 'a');
        assert_eq!(app.title(), "● Sin título — Mdx");
    }

    #[test]
    fn returning_to_saved_state_clears_indicator() {
        // AC3: borrar lo escrito devuelve el hash al del estado guardado.
        let (mut app, _) = App::new(None);
        type_char(&mut app, 'a');
        let _ = app.update(Message::Edit(Action::Edit(Edit::Backspace)));
        assert_eq!(app.title(), "Sin título — Mdx");
    }

    #[test]
    fn saved_content_is_clean_and_new_edit_is_dirty_again() {
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
        let (mut app, _) = App::new(None);
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
            let (mut app, _) = App::new(None);
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

    fn temp_workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/a.md"), "x").unwrap();
        std::fs::write(dir.path().join("r.md"), "x").unwrap();
        let ws = Workspace::open(dir.path(), TreeFilters::default()).unwrap();
        (dir, ws)
    }

    #[test]
    fn workspace_opened_is_stored() {
        // AC1 (lógica): al abrir una carpeta queda el árbol cargado.
        let (_dir, ws) = temp_workspace();
        let (mut app, _) = App::new(None);
        let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
        assert!(app.explorer.workspace.is_some());
        assert!(app.status.as_ref().is_some_and(|s| !s.is_error));
    }

    #[test]
    fn toggle_dir_loads_lazily_then_collapses() {
        // AC2
        let (_dir, ws) = temp_workspace();
        let sub = ws.root.join("sub");
        let (mut app, _) = App::new(None);
        let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
        let state = |app: &App, p: &Path| app.explorer.workspace.as_ref().unwrap().node(p).cloned();

        let _ = app.update(Message::ToggleDir(sub.clone()));
        let node = state(&app, &sub).unwrap();
        assert!(matches!(
            node.kind,
            mdx_core::workspace::NodeKind::Dir {
                children: mdx_core::workspace::Children::Loading,
                expanded: true
            }
        ));

        let kids = workspace::load_children(&sub, &TreeFilters::default());
        let _ = app.update(Message::DirLoaded(sub.clone(), kids));
        assert!(state(&app, &sub.join("a.md")).is_some());

        let _ = app.update(Message::ToggleDir(sub.clone()));
        assert!(matches!(
            state(&app, &sub).unwrap().kind,
            mdx_core::workspace::NodeKind::Dir {
                expanded: false,
                ..
            }
        ));
    }

    #[test]
    fn toggling_filters_updates_flags() {
        // AC3 / AC4
        let (mut app, _) = App::new(None);
        assert!(app.explorer.filters.only_markdown && !app.explorer.filters.show_hidden);
        let _ = app.update(Message::ToggleFilter(FilterKind::OnlyMarkdown));
        let _ = app.update(Message::ToggleFilter(FilterKind::Hidden));
        let _ = app.update(Message::ToggleFilter(FilterKind::Gitignore));
        let f = app.explorer.filters;
        assert!(!f.only_markdown && f.show_hidden && !f.respect_gitignore);
        let _ = app.update(Message::ToggleExplorerMenu);
        assert!(app.explorer.menu_open);
    }

    #[test]
    fn failed_workspace_open_reports_error() {
        let (mut app, _) = App::new(None);
        let _ = app.update(Message::WorkspaceOpened(Err(CoreError::NotFound(
            PathBuf::from("/x"),
        ))));
        assert!(app.explorer.workspace.is_none());
        assert!(app.status.as_ref().is_some_and(|s| s.is_error));
    }

    fn load(app: &mut App, name: &str) -> PathBuf {
        let path = PathBuf::from(format!("/proyecto/{name}"));
        let doc = Document::from_bytes(&path, b"# x", None).unwrap();
        let _ = app.update(Message::DocumentLoaded(Ok(doc)));
        path
    }

    fn unpinned_tabs(app: &App) -> usize {
        app.tabs.tabs().iter().filter(|t| !t.pinned).count()
    }

    #[test]
    fn single_clicks_reuse_one_preview_tab() {
        // AC1 (INV-7): clic simple sobre archivos distintos nunca acumula pestañas preview.
        let (mut app, _) = App::new(None);
        for name in ["a.md", "b.md", "c.md"] {
            let path = PathBuf::from(format!("/proyecto/{name}"));
            let _ = app.update(Message::FileClicked(path));
            load(&mut app, name);
        }
        assert_eq!(unpinned_tabs(&app), 1);
        assert_eq!(app.tabs.tabs().len(), 1);
        assert_eq!(app.doc.path.as_deref(), Some(Path::new("/proyecto/c.md")));
        assert_eq!(
            app.explorer.selected.as_deref(),
            Some(Path::new("/proyecto/c.md"))
        );
    }

    #[test]
    fn double_click_pins_the_opened_file() {
        // AC1: el doble clic llega con el archivo aún cargándose.
        let (mut app, _) = App::new(None);
        let path = PathBuf::from("/proyecto/a.md");
        let _ = app.update(Message::FileClicked(path.clone()));
        let _ = app.update(Message::FilePinned(path));
        load(&mut app, "a.md");
        assert_eq!(unpinned_tabs(&app), 0);
        assert!(app.tabs.is_pinned(app.doc.id));
    }

    #[test]
    fn double_click_on_open_preview_pins_it_without_reloading() {
        let (mut app, _) = App::new(None);
        let path = load(&mut app, "a.md");
        assert_eq!(unpinned_tabs(&app), 1);
        let id = app.doc.id;
        let _ = app.update(Message::FilePinned(path));
        assert!(app.tabs.is_pinned(id));
        assert_eq!(app.doc.id, id, "no se recarga el documento");
    }

    #[test]
    fn clicking_a_file_with_unsaved_changes_does_not_discard_them() {
        let (mut app, _) = App::new(None);
        type_char(&mut app, 'a');
        let _ = app.update(Message::FileClicked(PathBuf::from("/proyecto/b.md")));
        assert_eq!(
            app.content.text(),
            "a",
            "debe preguntar antes de reemplazar"
        );
    }

    #[test]
    fn keyboard_navigation_moves_selection_and_toggles() {
        // AC2
        let (_dir, ws) = temp_workspace();
        let sub = ws.root.join("sub");
        let (mut app, _) = App::new(None);
        let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
        let _ = app.update(Message::ExplorerKey(ExplorerKey::Down));
        assert_eq!(app.explorer.selected.as_deref(), Some(sub.as_path()));
        let _ = app.update(Message::ExplorerKey(ExplorerKey::Right));
        let expanded = |app: &App| {
            matches!(
                app.explorer
                    .workspace
                    .as_ref()
                    .unwrap()
                    .node(&sub)
                    .map(|n| &n.kind),
                Some(mdx_core::workspace::NodeKind::Dir { expanded: true, .. })
            )
        };
        assert!(expanded(&app));
        let _ = app.update(Message::ExplorerKey(ExplorerKey::Left));
        assert!(!expanded(&app));
    }

    fn open_prompt(app: &mut App, kind: PromptKind, target: &str, value: &str) {
        app.explorer.prompt = Some(Prompt {
            kind,
            target: PathBuf::from(target),
            value: value.to_owned(),
            error: None,
        });
    }

    #[test]
    fn context_menu_opens_for_the_clicked_row_and_closes() {
        // AC1
        let (mut app, _) = App::new(None);
        let target = ContextTarget {
            path: PathBuf::from("/p/a.md"),
            is_dir: false,
            is_root: false,
        };
        let _ = app.update(Message::OpenContext(target.clone()));
        assert_eq!(app.explorer.context, Some(target));
        let _ = app.update(Message::CloseContext);
        assert!(app.explorer.context.is_none());
    }

    #[test]
    fn context_actions_open_the_inline_name_prompt() {
        // AC1: Nuevo archivo sobre un archivo crea en su carpeta; Renombrar precarga el nombre.
        let (mut app, _) = App::new(None);
        let target = ContextTarget {
            path: PathBuf::from("/p/sub/a.md"),
            is_dir: false,
            is_root: false,
        };
        let _ = app.update(Message::OpenContext(target.clone()));
        let _ = app.update(Message::ContextAction(ContextAction::NewFile));
        let prompt = app.explorer.prompt.clone().unwrap();
        assert_eq!(
            (prompt.kind, prompt.target.as_path()),
            (PromptKind::NewFile, Path::new("/p/sub"))
        );

        let _ = app.update(Message::OpenContext(target));
        let _ = app.update(Message::ContextAction(ContextAction::Rename));
        let prompt = app.explorer.prompt.clone().unwrap();
        assert_eq!(
            (prompt.kind, prompt.value.as_str()),
            (PromptKind::Rename, "a.md")
        );
        assert!(
            app.explorer.context.is_none(),
            "el menú se cierra al elegir"
        );
    }

    #[test]
    fn invalid_name_shows_inline_error_without_running_any_operation() {
        // AC4
        let (mut app, _) = App::new(None);
        open_prompt(&mut app, PromptKind::NewFile, "/p", "a/b");
        let _ = app.update(Message::PromptSubmit);
        let prompt = app.explorer.prompt.clone().unwrap();
        assert!(
            prompt
                .error
                .as_deref()
                .is_some_and(|e| e.contains("no está permitido"))
        );
        // Escribir de nuevo limpia el error.
        let _ = app.update(Message::PromptChanged("ok".into()));
        assert!(app.explorer.prompt.as_ref().unwrap().error.is_none());
    }

    #[test]
    fn duplicate_name_error_from_disk_is_shown_inline_and_keeps_the_prompt() {
        // AC4
        let (mut app, _) = App::new(None);
        open_prompt(&mut app, PromptKind::Rename, "/p/a.md", "b.md");
        let _ = app.update(Message::OpDone(Err(CoreError::AlreadyExists(
            PathBuf::from("/p/b.md"),
        ))));
        let prompt = app.explorer.prompt.clone().unwrap();
        assert!(
            prompt
                .error
                .as_deref()
                .is_some_and(|e| e.contains("Ya existe"))
        );
        assert!(
            app.status.is_none(),
            "no es un error global de la barra de estado"
        );
    }

    #[test]
    fn renaming_the_open_file_updates_its_path() {
        // AC3
        let (mut app, _) = App::new(None);
        let from = load(&mut app, "a.md");
        type_char(&mut app, 'x');
        open_prompt(&mut app, PromptKind::Rename, "/proyecto/a.md", "b.md");
        let to = PathBuf::from("/proyecto/b.md");
        let _ = app.update(Message::OpDone(Ok(OpOutcome::Renamed {
            from,
            to: to.clone(),
        })));
        assert_eq!(app.doc.path.as_deref(), Some(to.as_path()));
        assert!(app.explorer.prompt.is_none());
        assert_eq!(
            app.title(),
            "● b.md — Mdx",
            "conserva el estado sin guardar"
        );
    }

    #[test]
    fn renaming_a_parent_folder_rebases_the_open_file() {
        // AC3
        let (mut app, _) = App::new(None);
        load(&mut app, "dir/a.md");
        let _ = app.update(Message::OpDone(Ok(OpOutcome::Renamed {
            from: PathBuf::from("/proyecto/dir"),
            to: PathBuf::from("/proyecto/nuevo"),
        })));
        assert_eq!(
            app.doc.path.as_deref(),
            Some(Path::new("/proyecto/nuevo/a.md"))
        );
    }

    #[test]
    fn trashing_the_open_file_keeps_its_content_marked_deleted() {
        // AC2
        let (mut app, _) = App::new(None);
        let path = load(&mut app, "a.md");
        let _ = app.update(Message::OpDone(Ok(OpOutcome::Trashed(path))));
        assert_eq!(app.doc.disk_state, mdx_core::DiskState::Deleted);
        assert_eq!(app.content.text(), "# x");
    }

    #[test]
    fn declining_the_trash_confirmation_does_nothing() {
        // AC2
        let (mut app, _) = App::new(None);
        let _ = app.update(Message::DeleteAnswered(PathBuf::from("/p/a.md"), false));
        assert!(app.status.is_none());
    }

    /// Abre `/proyecto/a.md` con `text` en disco y devuelve su ruta.
    fn open_with(app: &mut App, text: &str) -> PathBuf {
        let path = PathBuf::from("/proyecto/a.md");
        let doc = Document::from_bytes(&path, text.as_bytes(), None).unwrap();
        let _ = app.update(Message::DocumentLoaded(Ok(doc)));
        path
    }

    fn inspect(app: &mut App, path: &Path, result: Result<Document, CoreError>) {
        let _ = app.update(Message::DiskInspected(
            app.doc.id,
            path.to_path_buf(),
            result,
        ));
    }

    fn disk_version(path: &Path, text: &str) -> Result<Document, CoreError> {
        Document::from_bytes(path, text.as_bytes(), None)
    }

    #[test]
    fn external_change_on_clean_document_reloads_silently() {
        // AC2
        let (mut app, _) = App::new(None);
        let path = open_with(&mut app, "viejo");
        inspect(
            &mut app,
            &path,
            disk_version(&path, "nuevo desde otra herramienta"),
        );
        assert_eq!(app.content.text(), "nuevo desde otra herramienta");
        assert_eq!(app.doc.disk_state, DiskState::InSync);
        assert_eq!(app.title(), "a.md — Mdx");
        assert!(app.external.is_none());
    }

    #[test]
    fn external_change_on_dirty_document_shows_conflict_banner() {
        // AC3
        let (mut app, _) = App::new(None);
        let path = open_with(&mut app, "base");
        type_char(&mut app, 'X');
        inspect(&mut app, &path, disk_version(&path, "otra versión"));
        assert_eq!(app.doc.disk_state, DiskState::ChangedExternally);
        assert!(app.external.is_some());
        assert_eq!(app.content.text(), "Xbase", "el texto local no se toca");
    }

    #[test]
    fn conflict_reload_takes_the_disk_version() {
        let (mut app, _) = App::new(None);
        let path = open_with(&mut app, "base");
        type_char(&mut app, 'X');
        inspect(&mut app, &path, disk_version(&path, "disco"));
        let _ = app.update(Message::ResolveConflict(ConflictChoice::Reload));
        assert_eq!(app.content.text(), "disco");
        assert_eq!(app.doc.disk_state, DiskState::InSync);
        assert_eq!(app.title(), "a.md — Mdx");
    }

    #[test]
    fn conflict_keep_mine_keeps_text_and_stays_dirty() {
        let (mut app, _) = App::new(None);
        let path = open_with(&mut app, "base");
        type_char(&mut app, 'X');
        inspect(&mut app, &path, disk_version(&path, "disco"));
        let _ = app.update(Message::ResolveConflict(ConflictChoice::KeepMine));
        assert_eq!(app.content.text(), "Xbase");
        assert_eq!(app.doc.disk_state, DiskState::InSync);
        assert_eq!(app.title(), "● a.md — Mdx");
        // Un nuevo aviso con el mismo contenido de disco ya no molesta.
        inspect(&mut app, &path, disk_version(&path, "disco"));
        assert_eq!(app.doc.disk_state, DiskState::InSync);
    }

    #[test]
    fn own_save_never_triggers_the_banner() {
        // AC5 / INV-3: el disco coincide con `saved_hash`, aunque se haya seguido escribiendo.
        let (mut app, _) = App::new(None);
        let path = open_with(&mut app, "base");
        type_char(&mut app, 'X');
        let saved = mdx_core::document::hash_text("Xbase");
        let info = SavedInfo {
            path: path.clone(),
            saved_hash: saved,
            mtime: None,
        };
        let _ = app.update(Message::Saved(Ok(info)));
        type_char(&mut app, 'Y'); // sigue editando mientras llega el evento del guardado
        inspect(&mut app, &path, disk_version(&path, "Xbase"));
        assert_eq!(app.doc.disk_state, DiskState::InSync);
        assert!(app.external.is_none());
        assert_eq!(app.content.text(), "XYbase");
    }

    #[test]
    fn deleted_on_disk_is_marked_and_keeps_content_and_can_be_resaved() {
        // AC4
        let (mut app, _) = App::new(None);
        let path = open_with(&mut app, "contenido");
        inspect(&mut app, &path, Err(CoreError::NotFound(path.clone())));
        assert_eq!(app.doc.disk_state, DiskState::Deleted);
        assert_eq!(app.title(), "a.md (eliminado) — Mdx");
        assert_eq!(app.content.text(), "contenido");
        // Volver a guardar restituye el archivo y quita la marca.
        let info = SavedInfo {
            path: path.clone(),
            saved_hash: mdx_core::document::hash_text("contenido"),
            mtime: None,
        };
        let _ = app.update(Message::Saved(Ok(info)));
        assert_eq!(app.title(), "a.md — Mdx");
    }

    #[test]
    fn stale_inspection_results_are_ignored() {
        let (mut app, _) = App::new(None);
        let path = open_with(&mut app, "base");
        let old_id = app.doc.id;
        // Se abre otro documento mientras se leía el disco.
        let _ = load(&mut app, "otro.md");
        let _ = app.update(Message::DiskInspected(
            old_id,
            path.clone(),
            disk_version(&path, "x"),
        ));
        assert_eq!(app.content.text(), "# x");
        assert_eq!(app.doc.disk_state, DiskState::InSync);
    }

    #[test]
    fn fs_events_for_unrelated_files_do_nothing_to_the_document() {
        let (mut app, _) = App::new(None);
        let _ = open_with(&mut app, "base");
        let _ = app.update(Message::Fs(vec![FsEvent::Modified(PathBuf::from(
            "/otro/b.md",
        ))]));
        assert_eq!(app.content.text(), "base");
    }

    #[test]
    fn cancelling_save_dialog_clears_pending_action() {
        let (mut app, _) = App::new(None);
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
