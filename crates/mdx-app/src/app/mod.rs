//! Estado, mensajes, `update` y `view` de la aplicación (arquitectura Elm).

mod disk;
mod documents;
mod editing;
mod explorer_ops;
mod preview;
mod search_ops;
mod session_ops;
mod sync;
#[cfg(test)]
mod tests;
mod zoom;

use documents::Parked;
use editing::{ask_unsaved, pick_file_to_open};
use explorer_ops::pick_folder;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::{SinkExt, Stream, StreamExt};
use iced::widget::{
    Column, Row, button, checkbox, container, markdown, pane_grid, scrollable, text, text_editor,
    tooltip,
};
use iced::{
    Element, Event, Font, Length, Subscription, Task, Theme, event, keyboard, time, window,
};
use mdx_core::config::{Config, LoadedConfig};
use mdx_core::document::count_words;
use mdx_core::fileops;
use mdx_core::history::History;
use mdx_core::io::SavedInfo;
use mdx_core::link::{self, LinkTarget};
use mdx_core::outline::{Block, Outline};
use mdx_core::paths::{self, AppFiles};
use mdx_core::search::WorkspaceSearch;
use mdx_core::session::{Boot, Session};
use mdx_core::watcher::{self, FsEvent};
use mdx_core::workspace::{self, FileNode, TreeFilters, Workspace};
use mdx_core::{CoreError, DiskState, Document, DocumentId};

use crate::editor;
use crate::explorer::{
    self, ContextAction, ContextTarget, ExplorerKey, ExplorerState, Nav, PROMPT_ID, Prompt,
    PromptKind,
};
use crate::layout::{PaneKind, initial_panes, panes_without_preview};
use crate::outline as outline_panel;
use crate::preview_sync::{self, Layout};
use crate::search::{self as search_ui, FindOption, FindState, WorkspaceSearchState};
use crate::shortcuts::{self, ShortcutAction};
use crate::state::tabs::Tabs;
use crate::statusbar::{Stats, display_col};
use crate::strings;
use crate::theme;
use zoom::ZoomChange;

/// Visor de Markdown por defecto; solo define cómo se convierte un enlace en mensaje.
struct PreviewViewer;

impl markdown::Viewer<'_, String> for PreviewViewer {
    fn on_link_click(url: markdown::Uri) -> String {
        url
    }
}

/// Qué pidió abrir la línea de comandos (`mdx <ruta>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliTarget {
    Folder(PathBuf),
    File(PathBuf),
}

/// Estado de la sesión persistente (US-3.5).
pub struct SessionState {
    /// Rutas de `config.toml` y `session.toml` (`None` si no hay directorio personal).
    files: Option<AppFiles>,
    /// Sesión leída al arrancar (aporta los «Recientes»); se completa al guardar.
    data: Session,
    /// La sesión anterior ya se leyó: antes de eso no se guarda (no pisarla a medio restaurar).
    ready: bool,
    /// Argumento de la línea de comandos pendiente de abrir tras restaurar la sesión.
    cli: Option<CliTarget>,
    /// Carpetas que estaban expandidas en la sesión, pendientes de abrir.
    pending_expand: Vec<PathBuf>,
}

pub struct App {
    explorer: ExplorerState,
    sidebar: Sidebar,
    outline: Outline,
    /// Bloques de nivel superior del documento (líneas de origen), para el scroll sincronizado.
    blocks: Vec<Block>,
    /// Primera fila visible del editor: (línea, fracción de esa línea).
    view_top: (usize, f32),
    /// Posición de los bloques ya renderizados en la vista previa del documento activo.
    preview_layout: Layout,
    /// Scroll sincronizado editor → vista previa (US-3.3 AC2).
    sync: SyncScroll,
    tabs: Tabs,
    /// Documentos abiertos que no son el activo (ver `documents`).
    parked: Parked,
    /// Versión del disco pendiente de resolver cuando hay conflicto (banner).
    external: Option<Document>,
    /// Ruta cuyo doble clic (o apertura explícita) llegó mientras se cargaba: queda fijada.
    pin_on_load: Option<PathBuf>,
    /// Tras abrir este archivo, posicionar el editor en (línea, columnas) del resultado.
    pending_goto: Option<(PathBuf, usize, std::ops::Range<usize>)>,
    find: FindState,
    ws_search: WorkspaceSearchState,
    config: Config,
    /// Modo claro/oscuro que informa el sistema operativo (para `theme = "system"`).
    system_mode: iced::theme::Mode,
    /// Zoom actual (factor del tamaño de fuente del editor y de la vista previa).
    zoom: f32,
    /// Fuente del editor resuelta desde `[editor] font_family`.
    editor_font: Font,
    sess: SessionState,
    /// Documentos con un autoguardado en curso.
    autosave_inflight: std::collections::HashSet<DocumentId>,
    /// Último cambio del documento activo (para el retraso del autoguardado).
    last_change: Instant,
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

/// Tamaño base del texto de la vista previa (iced usa 16 px por defecto).
const PREVIEW_BASE_TEXT_PX: f32 = 16.0;

/// Espera tras la última edición antes de re-parsear la vista previa (US-1.4 AC1).
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);
/// Frecuencia de comprobación del debounce mientras hay cambios pendientes.
const PREVIEW_TICK: Duration = Duration::from_millis(25);

/// Tiempo máximo entre pulsaciones para que cuenten como un solo paso de deshacer.
const MERGE_WINDOW: Duration = Duration::from_millis(1000);

/// Acciones que descartarían cambios sin guardar; se protegen con el diálogo de confirmación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAction {
    /// Cerrar la aplicación (revisa todos los documentos abiertos).
    Close,
    CloseTab(DocumentId),
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

/// Estado del scroll sincronizado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncScroll {
    On,
    Off,
}

/// Contenido del panel izquierdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sidebar {
    Files,
    Outline,
    Search,
}

/// Resultado del parseo en segundo plano de un documento.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub items: Vec<markdown::Item>,
    pub outline: Outline,
    pub blocks: Vec<Block>,
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
    Saved(DocumentId, Result<SavedInfo, CoreError>),
    SelectTab(DocumentId),
    CloseTab(DocumentId),
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
    PreviewParsed(DocumentId, u64, Parsed),
    PreviewMeasured(DocumentId, Layout),
    SetSyncScroll(bool),
    ExportHtml,
    HtmlExportPicked(Option<(PathBuf, String, String)>),
    HtmlExported(Result<PathBuf, CoreError>),
    LayoutChanged,
    SetSidebar(Sidebar),
    GoToLine(usize),
    SystemTheme(iced::theme::Mode),
    ConfigLoaded(LoadedConfig),
    ConfigReloaded(LoadedConfig),
    ConfigFileReady(Result<PathBuf, CoreError>),
    SessionBooted(Box<Boot>),
    SessionSaved(Result<(), CoreError>),
    AutosaveTick(Instant),
    OpenRecentFolder(PathBuf),
    OpenRecentFile(PathBuf),
    FindQueryChanged(String),
    ReplaceChanged(String),
    FindOptionToggled(FindOption),
    FindNext,
    FindPrev,
    ReplaceOne,
    ReplaceAll,
    FindClose,
    EscapePressed,
    WsQueryChanged(String),
    WsOptionToggled(FindOption),
    WsSearch,
    WsCancel,
    WsDone(u64, Result<WorkspaceSearch, CoreError>),
    OpenResult(PathBuf, usize, std::ops::Range<usize>),
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
        let app_files = paths::app_files();
        let cli = arg.map(|path| {
            if path.is_dir() {
                CliTarget::Folder(path)
            } else {
                CliTarget::File(path)
            }
        });
        // Con la sesión disponible se restaura primero y el argumento se abre después, encima.
        // Sin directorio de estado no hay sesión: el argumento se abre de inmediato.
        let session_boot = Self::boot_tasks(app_files.clone());
        let (cli_boot, cli_pending) = match (app_files.is_some(), cli) {
            (true, cli) => (Task::none(), cli),
            (false, Some(CliTarget::Folder(path))) => (
                Task::perform(
                    workspace::open_async(path, filters),
                    Message::WorkspaceOpened,
                ),
                None,
            ),
            (false, Some(CliTarget::File(path))) => {
                // Apertura explícita por CLI: la pestaña queda fijada.
                pin_on_load = Some(path.clone());
                (
                    Task::perform(
                        async move { mdx_core::io::read_document(&path).await },
                        Message::DocumentLoaded,
                    ),
                    None,
                )
            }
            (false, None) => (Task::none(), None),
        };
        let system_theme = iced::system::theme().map(Message::SystemTheme);
        let boot = Task::batch([cli_boot, session_boot, system_theme]);
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
                sidebar: Sidebar::Files,
                outline: Vec::new(),
                blocks: Vec::new(),
                view_top: (0, 0.0),
                preview_layout: Layout::default(),
                sync: SyncScroll::On,
                tabs,
                parked: Parked::new(),
                pin_on_load,
                pending_goto: None,
                config: Config::default(),
                system_mode: iced::theme::Mode::None,
                zoom: 1.0,
                editor_font: Font::MONOSPACE,
                sess: SessionState {
                    files: app_files,
                    data: Session::default(),
                    ready: false,
                    cli: cli_pending,
                    pending_expand: Vec::new(),
                },
                autosave_inflight: std::collections::HashSet::new(),
                last_change: Instant::now(),
                find: FindState::default(),
                ws_search: WorkspaceSearchState::default(),
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

    /// Tema activo: el del ajuste `appearance.theme` o, en modo «sistema», el del SO.
    pub fn theme(&self) -> Theme {
        theme::resolve(self.config.appearance.theme, self.system_mode)
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
            event::listen_with(|event, _status, _window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::EscapePressed),
                _ => None,
            }),
            window::close_requests().map(|_| Message::CloseRequested),
            if self.autosave_armed() {
                time::every(Duration::from_millis(250)).map(Message::AutosaveTick)
            } else {
                Subscription::none()
            },
            self.watch_subscriptions(),
            window::resize_events().map(|_| Message::LayoutChanged),
            iced::system::theme_changes().map(Message::SystemTheme),
            if self.preview_pending {
                time::every(PREVIEW_TICK).map(Message::PreviewTick)
            } else {
                Subscription::none()
            },
        ])
    }

    /// Procesa un mensaje y, si cambió algo que forma parte de la sesión (pestañas abiertas,
    /// carpeta, recientes…), guarda `session.toml` en segundo plano.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let before = self.session_signature();
        let task = self.update_inner(message);
        if self.can_save_session() && self.session_signature() != before {
            Task::batch([task, self.save_session()])
        } else {
            task
        }
    }

    // iced exige `Message` por valor; crecerá con las próximas historias.
    #[allow(clippy::needless_pass_by_value)]
    fn update_inner(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
                // El ancho de la vista previa cambió: los bloques miden otra altura.
                self.measure_preview()
            }
            Message::LayoutChanged => self.measure_preview(),
            Message::Edit(action) => self.handle_edit(action),
            Message::Shortcut(action) => self.handle_shortcut(action),
            Message::FilePicked(Some(path)) | Message::OpenRecentFile(path) => {
                self.open_path(path, true)
            }
            Message::FilePicked(None) | Message::FolderPicked(None) => Task::none(),
            Message::DocumentLoaded(result) => self.handle_loaded(result),
            Message::SavePicked(Some(path)) => self.save_to(path),
            Message::SavePicked(None) => {
                self.after_save = None;
                Task::none()
            }
            Message::Saved(id, result) => self.handle_saved(id, result),
            Message::SelectTab(id) => self.select_tab(id),
            Message::CloseTab(id) => self.request(PendingAction::CloseTab(id)),
            Message::CloseRequested => self.request(PendingAction::Close),
            Message::Fs(events) => self.handle_fs(&events),
            Message::WatchFailed(err) => {
                self.set_status(format!("{}: {err}", strings::STATUS_WATCH_FAILED), true);
                Task::none()
            }
            Message::DiskInspected(id, path, result) => self
                .with_doc(id, |app| app.handle_disk_inspected(id, &path, result))
                .unwrap_or_else(Task::none),
            Message::ResolveConflict(choice) => self.resolve_conflict(choice),
            Message::SetSidebar(sidebar) => {
                self.sidebar = sidebar;
                Task::none()
            }
            Message::GoToLine(line) => self.go_to_line(line),
            m @ (Message::SystemTheme(_)
            | Message::ConfigLoaded(_)
            | Message::ConfigReloaded(_)
            | Message::ConfigFileReady(_)
            | Message::SessionBooted(_)
            | Message::SessionSaved(_)
            | Message::AutosaveTick(_)
            | Message::OpenRecentFolder(_)) => self.update_session(m),
            m @ (Message::PreviewTick(_)
            | Message::PreviewParsed(..)
            | Message::PreviewMeasured(..)
            | Message::SetSyncScroll(_)
            | Message::ExportHtml
            | Message::HtmlExportPicked(_)
            | Message::HtmlExported(_)) => self.update_preview(m),
            Message::EscapePressed => {
                if self.find.open {
                    self.close_find();
                }
                Task::none()
            }
            m @ (Message::FindQueryChanged(_)
            | Message::ReplaceChanged(_)
            | Message::FindOptionToggled(_)
            | Message::FindNext
            | Message::FindPrev
            | Message::ReplaceOne
            | Message::ReplaceAll
            | Message::FindClose
            | Message::WsQueryChanged(_)
            | Message::WsOptionToggled(_)
            | Message::WsSearch
            | Message::WsCancel
            | Message::WsDone(..)
            | Message::OpenResult(..)) => self.update_search(m),
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
            Message::GuardAnswered(action, choice) => self.guard_answered(action, choice),
        }
    }

    fn handle_shortcut(&mut self, action: ShortcutAction) -> Task<Message> {
        match action {
            ShortcutAction::New => self.new_untitled(),
            ShortcutAction::Open => Task::perform(pick_file_to_open(), Message::FilePicked),
            ShortcutAction::CloseTab => self.request(PendingAction::CloseTab(self.doc.id)),
            ShortcutAction::NextTab => self.cycle_tab(1),
            ShortcutAction::PrevTab => self.cycle_tab(-1),
            ShortcutAction::GoToTab(n) => self.goto_tab(n),
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
            ShortcutAction::OpenConfig => self.open_config(),
            ShortcutAction::ExportHtml => self.export_html(),
            ShortcutAction::ZoomIn => self.change_zoom(ZoomChange::Step(1)),
            ShortcutAction::ZoomOut => self.change_zoom(ZoomChange::Step(-1)),
            ShortcutAction::ZoomReset => self.change_zoom(ZoomChange::Reset),
            ShortcutAction::Find => self.open_find(false),
            ShortcutAction::Replace => self.open_find(true),
            ShortcutAction::FindInWorkspace => self.open_workspace_search(),
        }
    }

    /// Ejecuta `action`, o pregunta primero si el documento afectado tiene cambios sin guardar.
    fn request(&mut self, action: PendingAction) -> Task<Message> {
        match self.guard_target(&action).filter(|id| self.is_dirty(*id)) {
            None => self.run(action),
            Some(id) => {
                let name = self.doc_name(id);
                Task::perform(ask_unsaved(name), move |choice| {
                    Message::GuardAnswered(action, choice)
                })
            }
        }
    }

    fn run(&mut self, action: PendingAction) -> Task<Message> {
        match action {
            PendingAction::Close => {
                // La sesión debe estar escrita antes de salir.
                if self.can_save_session() {
                    self.save_session().then(|_| iced::exit())
                } else {
                    iced::exit()
                }
            }
            PendingAction::CloseTab(id) => self.close_tab(id),
        }
    }

    fn guard_answered(&mut self, action: PendingAction, choice: GuardChoice) -> Task<Message> {
        let Some(id) = self.guard_target(&action) else {
            return Task::none();
        };
        match choice {
            GuardChoice::Save => {
                // Se muestra la pestaña que se está guardando (puede pedir «Guardar como»).
                let select = self.select_tab(id);
                self.after_save = Some(action);
                Task::batch([select, self.save()])
            }
            GuardChoice::Discard => {
                let task = self.close_tab(id);
                match action {
                    // Al cerrar la app puede quedar otro documento con cambios.
                    PendingAction::Close => Task::batch([task, self.request(PendingAction::Close)]),
                    PendingAction::CloseTab(_) => task,
                }
            }
            GuardChoice::Cancel => Task::none(),
        }
    }

    fn set_status(&mut self, text: String, is_error: bool) {
        self.status = Some(Status { text, is_error });
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

    /// Conmutador «Archivos | Esquema» del panel izquierdo.
    fn sidebar_switch(&self) -> Element<'_, Message> {
        let tab = |label: &'static str, target: Sidebar| {
            button(text(label).size(13))
                .padding([2, 8])
                .style(if self.sidebar == target {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::SetSidebar(target))
        };
        Row::new()
            .spacing(4)
            .push(tab(strings::SIDEBAR_FILES, Sidebar::Files))
            .push(tab(strings::SIDEBAR_OUTLINE, Sidebar::Outline))
            .push(tab(strings::SIDEBAR_SEARCH, Sidebar::Search))
            .into()
    }

    /// Barra de pestañas: `●` si está sucia, cursiva si es «preview», tooltip con la ruta.
    fn tab_bar(&self) -> Element<'_, Message> {
        let mut bar = Row::new().spacing(4);
        for tab in self.tabs.tabs() {
            let Some(label) = self.tab_label(tab.doc) else {
                continue;
            };
            let active = label.id == self.doc.id;
            let mut title = String::new();
            if label.dirty {
                title.push_str(strings::DIRTY_MARK);
            }
            title.push_str(&label.name);
            if label.deleted {
                title.push_str(strings::DELETED_MARK);
            }
            let font = if label.pinned {
                Font::DEFAULT
            } else {
                Font {
                    style: iced::font::Style::Italic,
                    ..Font::DEFAULT
                }
            };
            let select = button(text(title).font(font))
                .padding([3, 8])
                .style(if active {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::SelectTab(label.id));
            let close = button(text(strings::TAB_CLOSE).size(14))
                .padding([3, 6])
                .style(button::text)
                .on_press(Message::CloseTab(label.id));
            bar = bar.push(tooltip(
                Row::new()
                    .align_y(iced::Alignment::Center)
                    .push(select)
                    .push(close),
                container(text(label.full_path).size(12))
                    .padding(4)
                    .style(container::bordered_box),
                tooltip::Position::Bottom,
            ));
        }
        scrollable(bar)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::default(),
            ))
            .width(Length::Fill)
            .into()
    }

    fn editor_pane(&self, theme: &Theme) -> Element<'_, Message> {
        Column::new()
            .spacing(6)
            .push(self.tab_bar())
            .extend(self.find.open.then(|| search_ui::find_bar(&self.find)))
            .push(
                text_editor(&self.content)
                    .on_action(Message::Edit)
                    .key_binding(editor::key_binding)
                    .font(self.editor_font)
                    .size(editor::zoomed_px(self.config.editor.font_size, self.zoom))
                    .wrapping(editor::wrapping(self.config.editor.line_wrap))
                    .highlight("markdown", theme::highlight_theme(theme))
                    .height(Length::Fill),
            )
            .into()
    }

    fn preview_pane(&self, theme: &Theme) -> Element<'_, Message> {
        let settings = markdown::Settings::with_text_size(
            PREVIEW_BASE_TEXT_PX * self.zoom,
            markdown::Style::from(theme),
        );
        let blocks = self.preview.iter().enumerate().map(|(i, item)| {
            container(markdown::item(&PreviewViewer, settings, item, i))
                .id(preview_sync::block_id(i))
                .into()
        });
        let column: Element<'_, String> = Column::with_children(blocks)
            .spacing(settings.spacing)
            .into();
        Column::new()
            .spacing(6)
            .push(
                Row::new()
                    .spacing(12)
                    .align_y(iced::Alignment::Center)
                    .push(
                        checkbox(self.sync == SyncScroll::On)
                            .label(strings::SYNC_SCROLL)
                            .size(16)
                            .text_size(13)
                            .on_toggle(Message::SetSyncScroll),
                    )
                    .push(
                        button(text(strings::EXPORT_HTML).size(12))
                            .padding([2, 8])
                            .style(button::secondary)
                            .on_press(Message::ExportHtml),
                    ),
            )
            .push(
                scrollable(column.map(Message::LinkClicked))
                    .id(iced::widget::Id::new(preview_sync::SCROLL_ID))
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let theme = self.theme();
        let grid = pane_grid(&self.panes, |_, kind, _| {
            let body: Element<'_, Message> = match kind {
                PaneKind::Explorer => Column::new()
                    .spacing(6)
                    .push(self.sidebar_switch())
                    .push(match self.sidebar {
                        Sidebar::Files => explorer::view(
                            &self.explorer,
                            self.doc.path.as_deref(),
                            &self.sess.data.recent,
                        ),
                        Sidebar::Outline => outline_panel::view(&self.outline),
                        Sidebar::Search => search_ui::workspace_panel(
                            &self.ws_search,
                            self.explorer.workspace.as_ref().map(|ws| ws.root.as_path()),
                        ),
                    })
                    .into(),
                PaneKind::Editor => self.editor_pane(&theme),
                PaneKind::Preview => self.preview_pane(&theme),
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
