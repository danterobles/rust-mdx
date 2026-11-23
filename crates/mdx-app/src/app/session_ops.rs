//! Sesión (US-3.5): guardar y restaurar pestañas, carpeta y paneles; «Recientes»; autoguardado.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::Task;
use mdx_core::config::{Config, ConfigWarning, LoadedConfig};
use mdx_core::paths::AppFiles;
use mdx_core::session::{Boot, Layout, Session};
use mdx_core::workspace::{self, Children, FileNode, NodeKind};

use super::{App, DiskState, DocumentId, Message, strings};
use crate::layout;

/// Lo que, si cambia, obliga a guardar la sesión (no incluye los anchos de panel: se guardan con
/// el siguiente cambio y al cerrar, para no escribir mientras se arrastra un divisor).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionSignature {
    workspace: Option<PathBuf>,
    open_files: Vec<PathBuf>,
    active: Option<PathBuf>,
    recent_folders: Vec<PathBuf>,
    recent_files: Vec<PathBuf>,
    expanded: Vec<PathBuf>,
}

/// `true` si ya pasó el retraso de autoguardado desde el último cambio.
#[must_use]
pub fn autosave_due(last_change: Instant, now: Instant, delay: Duration) -> bool {
    now.saturating_duration_since(last_change) >= delay
}

fn expanded_dirs(node: &FileNode, out: &mut Vec<PathBuf>) {
    if let NodeKind::Dir {
        children: Children::Loaded(kids),
        expanded: true,
    } = &node.kind
    {
        for kid in kids {
            if matches!(kid.kind, NodeKind::Dir { expanded: true, .. }) {
                out.push(kid.path.clone());
            }
            expanded_dirs(kid, out);
        }
    }
}

impl App {
    /// Tareas de arranque: configuración y sesión (esta última, restaurada solo sin argumento).
    pub(super) fn boot_tasks(files: Option<AppFiles>, restore: bool) -> Task<Message> {
        let Some(files) = files else {
            return Task::none();
        };
        let config_file = files.config_file;
        let session_file = files.session_file;
        let config = Task::perform(
            async move { Config::load(&config_file).await },
            Message::ConfigLoaded,
        );
        let session = Task::perform(
            async move { Session::boot(&session_file, restore).await },
            |boot| Message::SessionBooted(Box::new(boot)),
        );
        Task::batch([config, session])
    }

    /// `true` si hay dónde guardar la sesión y la anterior ya se leyó.
    pub(super) fn can_save_session(&self) -> bool {
        self.sess.files.is_some() && self.sess.ready
    }

    pub(super) fn apply_config(&mut self, loaded: LoadedConfig) {
        self.config = loaded.config;
        if let Some(warning) = loaded.warnings.first() {
            let text = match warning {
                ConfigWarning::Invalid(detail) => {
                    strings::CONFIG_INVALID.replace("{detail}", detail)
                }
                ConfigWarning::NewerSchema(v) => {
                    strings::CONFIG_NEWER.replace("{version}", &v.to_string())
                }
                ConfigWarning::Clamped(field) => strings::CONFIG_CLAMPED.replace("{field}", field),
            };
            self.set_status(text, true);
        }
    }

    /// Aplica la sesión leída al arrancar: recientes y, si procede, carpeta, pestañas y paneles.
    pub(super) fn apply_boot(&mut self, boot: Boot) -> Task<Message> {
        let Boot {
            session,
            plan,
            documents,
            unusable,
        } = boot;
        self.sess.data = session;
        self.sess.ready = true;
        let mut tasks = Vec::new();
        if unusable {
            self.set_status(strings::SESSION_UNUSABLE.to_owned(), true);
        }
        if self.sess.restore {
            let layout = self.sess.data.layout;
            layout::apply_ratios(
                &mut self.panes,
                layout.explorer_ratio,
                layout.inner_editor_ratio(),
            );
            if let Some(root) = plan.workspace {
                self.sess.pending_expand = plan.expanded_dirs;
                tasks.push(Task::perform(
                    workspace::open_async(root, self.explorer.filters),
                    Message::WorkspaceOpened,
                ));
            }
            // Orden de pestañas = orden guardado; todas fijas (las abrió el usuario).
            for doc in documents {
                tasks.push(self.add_doc(doc, true));
            }
            if let Some(active) = plan.active
                && let Some(id) = self.find_open(&active)
            {
                tasks.push(self.select_tab(id));
            }
        }
        Task::batch(tasks)
    }

    /// Abre las carpetas guardadas como expandidas a medida que sus padres se van cargando.
    pub(super) fn expand_pending(&mut self) -> Task<Message> {
        if self.sess.pending_expand.is_empty() {
            return Task::none();
        }
        let filters = self.explorer.filters;
        let Some(ws) = &mut self.explorer.workspace else {
            return Task::none();
        };
        let mut tasks = Vec::new();
        let mut remaining = Vec::new();
        for dir in std::mem::take(&mut self.sess.pending_expand) {
            if ws.node(&dir).is_none() {
                // Su carpeta padre aún no está cargada: se reintenta en la próxima carga.
                remaining.push(dir);
            } else if ws.begin_expand(&dir) {
                let target = dir.clone();
                tasks.push(Task::perform(
                    workspace::load_children_async(dir, filters),
                    move |result| Message::DirLoaded(target.clone(), result),
                ));
            }
        }
        self.sess.pending_expand = remaining;
        Task::batch(tasks)
    }

    /// Estado de la sesión tal como está ahora (conserva los recientes ya acumulados).
    pub(super) fn session_snapshot(&self) -> Session {
        let mut session = self.sess.data.clone();
        session.workspace = self.explorer.workspace.as_ref().map(|ws| ws.root.clone());
        session.open_files = self
            .tabs
            .tabs()
            .iter()
            .filter_map(|t| self.doc_path(t.doc))
            .collect();
        session.active_tab = self
            .doc_path(self.doc.id)
            .and_then(|active| session.open_files.iter().position(|p| *p == active))
            .unwrap_or(0);
        let mut expanded = Vec::new();
        if let Some(ws) = &self.explorer.workspace {
            expanded_dirs(&ws.tree, &mut expanded);
        }
        expanded.truncate(200);
        session.expanded_dirs = expanded;
        if let Some((explorer, inner)) = layout::current_ratios(&self.panes) {
            let inner = inner.unwrap_or_else(|| session.layout.inner_editor_ratio());
            session.layout = Layout::from_pane_ratios(explorer, inner);
        }
        session
    }

    fn doc_path(&self, id: DocumentId) -> Option<PathBuf> {
        self.open_docs()
            .find(|(d, _)| d.id == id)
            .and_then(|(d, _)| d.path.clone())
    }

    pub(super) fn session_signature(&self) -> SessionSignature {
        let mut expanded = Vec::new();
        if let Some(ws) = &self.explorer.workspace {
            expanded_dirs(&ws.tree, &mut expanded);
        }
        SessionSignature {
            workspace: self.explorer.workspace.as_ref().map(|ws| ws.root.clone()),
            open_files: self
                .tabs
                .tabs()
                .iter()
                .filter_map(|t| self.doc_path(t.doc))
                .collect(),
            active: self.doc_path(self.doc.id),
            recent_folders: self.sess.data.recent.folders.clone(),
            recent_files: self.sess.data.recent.files.clone(),
            expanded,
        }
    }

    /// Escribe `session.toml` (en segundo plano). No hace nada hasta que la sesión anterior se
    /// haya leído, para no pisarla con un estado a medio restaurar.
    pub(super) fn save_session(&self) -> Task<Message> {
        let Some(files) = self.sess.files.clone() else {
            return Task::none();
        };
        if !self.can_save_session() {
            return Task::none();
        }
        let session = self.session_snapshot();
        Task::perform(
            async move { session.save(&files.session_file).await },
            Message::SessionSaved,
        )
    }

    /// Registra en «Recientes» lo que se acaba de abrir.
    pub(super) fn note_recent_folder(&mut self, root: &Path) {
        self.sess.data.push_recent_folder(root);
    }

    pub(super) fn note_recent_file(&mut self, file: &Path) {
        self.sess.data.push_recent_file(file);
    }

    /// Documentos que el autoguardado puede guardar ahora: con ruta, con cambios, sin conflicto
    /// con el disco, sin un guardado en curso y con el retraso cumplido.
    pub(super) fn autosave_candidates(&self, now: Instant) -> Vec<DocumentId> {
        if !self.config.files.autosave {
            return Vec::new();
        }
        let delay = Duration::from_millis(self.config.files.autosave_delay_ms);
        self.doc_summaries()
            .into_iter()
            .filter(|d| {
                d.path_is_set
                    && d.dirty
                    && d.disk_state == DiskState::InSync
                    && !self.autosave_inflight.contains(&d.id)
                    && autosave_due(d.last_change, now, delay)
            })
            .map(|d| d.id)
            .collect()
    }

    /// `true` si hay algo que el autoguardado pueda querer guardar (activa el temporizador).
    pub(super) fn autosave_armed(&self) -> bool {
        self.config.files.autosave
            && self
                .doc_summaries()
                .iter()
                .any(|d| d.path_is_set && d.dirty && d.disk_state == DiskState::InSync)
    }

    pub(super) fn autosave_tick(&mut self, now: Instant) -> Task<Message> {
        let mut tasks = Vec::new();
        for id in self.autosave_candidates(now) {
            self.autosave_inflight.insert(id);
            if let Some(task) = self.with_doc(id, |app| app.save()) {
                tasks.push(task);
            }
        }
        Task::batch(tasks)
    }

    pub(super) fn open_recent_folder(&mut self, folder: PathBuf) -> Task<Message> {
        Task::perform(
            workspace::open_async(folder, self.explorer.filters),
            Message::WorkspaceOpened,
        )
    }
}

/// Documento (sin el contenido) con lo que necesita el autoguardado.
pub(super) struct DocSummary {
    pub id: DocumentId,
    pub path_is_set: bool,
    pub dirty: bool,
    pub disk_state: DiskState,
    pub last_change: Instant,
}
