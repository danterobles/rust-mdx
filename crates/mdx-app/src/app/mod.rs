//! Estado, mensajes, `update` y `view` de la aplicación (arquitectura Elm).

mod disk;
mod documents;
mod editing;
mod explorer_ops;
mod preview;
#[cfg(test)]
mod tests;

use documents::Parked;
use editing::{ask_unsaved, pick_file_to_open};
use explorer_ops::pick_folder;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::{SinkExt, Stream, StreamExt};
use iced::widget::{
    Column, Row, button, container, markdown, pane_grid, scrollable, text, text_editor, tooltip,
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
use mdx_core::outline::Outline;
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
use crate::shortcuts::{self, ShortcutAction};
use crate::state::tabs::Tabs;
use crate::statusbar::{Stats, display_col};
use crate::strings;

pub struct App {
    explorer: ExplorerState,
    sidebar: Sidebar,
    outline: Outline,
    tabs: Tabs,
    /// Documentos abiertos que no son el activo (ver `documents`).
    parked: Parked,
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

/// Contenido del panel izquierdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sidebar {
    Files,
    Outline,
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
    PreviewParsed(DocumentId, u64, Vec<markdown::Item>, Outline),
    SetSidebar(Sidebar),
    GoToLine(usize),
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
                sidebar: Sidebar::Files,
                outline: Vec::new(),
                tabs,
                parked: Parked::new(),
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
            if self.preview_pending {
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
            Message::FilePicked(Some(path)) => self.open_path(path, true),
            Message::FilePicked(None) | Message::FolderPicked(None) => Task::none(),
            Message::DocumentLoaded(result) => self.handle_loaded(result),
            Message::SavePicked(Some(path)) => self.save_to(path),
            Message::SavePicked(None) => {
                self.after_save = None;
                Task::none()
            }
            Message::Saved(id, result) => self.handle_saved(id, result),
            Message::SelectTab(id) => {
                self.select_tab(id);
                Task::none()
            }
            Message::CloseTab(id) => self.request(PendingAction::CloseTab(id)),
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
            Message::DiskInspected(id, path, result) => self
                .with_doc(id, |app| app.handle_disk_inspected(id, &path, result))
                .unwrap_or_else(Task::none),
            Message::ResolveConflict(choice) => self.resolve_conflict(choice),
            Message::PreviewParsed(id, generation, items, outline) => {
                // Puede llegar para un documento que ya no es el activo.
                let _ = self.with_doc(id, |app| {
                    if generation == app.preview_gen {
                        // Con la vista previa oculta no se parsea (items vacíos): se conserva.
                        if app.preview_visible {
                            app.preview = items;
                        }
                        app.outline = outline;
                    }
                });
                Task::none()
            }
            Message::SetSidebar(sidebar) => {
                self.sidebar = sidebar;
                Task::none()
            }
            Message::GoToLine(line) => {
                self.go_to_line(line);
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
            Message::GuardAnswered(action, choice) => self.guard_answered(action, choice),
        }
    }

    fn handle_shortcut(&mut self, action: ShortcutAction) -> Task<Message> {
        match action {
            ShortcutAction::New => self.new_untitled(),
            ShortcutAction::Open => Task::perform(pick_file_to_open(), Message::FilePicked),
            ShortcutAction::CloseTab => self.request(PendingAction::CloseTab(self.doc.id)),
            ShortcutAction::NextTab => {
                self.cycle_tab(1);
                Task::none()
            }
            ShortcutAction::PrevTab => {
                self.cycle_tab(-1);
                Task::none()
            }
            ShortcutAction::GoToTab(n) => {
                self.goto_tab(n);
                Task::none()
            }
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
            PendingAction::Close => iced::exit(),
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
                self.select_tab(id);
                self.after_save = Some(action);
                self.save()
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

    pub fn view(&self) -> Element<'_, Message> {
        let theme = self.theme();
        let grid = pane_grid(&self.panes, |_, kind, _| {
            let body: Element<'_, Message> = match kind {
                PaneKind::Explorer => Column::new()
                    .spacing(6)
                    .push(self.sidebar_switch())
                    .push(match self.sidebar {
                        Sidebar::Files => explorer::view(&self.explorer, self.doc.path.as_deref()),
                        Sidebar::Outline => outline_panel::view(&self.outline),
                    })
                    .into(),
                PaneKind::Editor => Column::new()
                    .spacing(6)
                    .push(self.tab_bar())
                    .push(
                        text_editor(&self.content)
                            .on_action(Message::Edit)
                            .key_binding(editor::key_binding)
                            .font(Font::MONOSPACE)
                            .highlight("markdown", highlighter::Theme::InspiredGitHub)
                            .height(Length::Fill),
                    )
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
