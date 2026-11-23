//! Cambios externos en disco (US-2.4): vigilancia, recarga silenciosa y conflictos.

use super::{
    App, ConflictChoice, CoreError, DiskState, Document, DocumentId, FsEvent, Message, Path,
    PathBuf, SinkExt, Stream, StreamExt, Subscription, Task, explorer, strings, text_editor,
    watcher, workspace,
};

impl App {
    /// Vigila la carpeta abierta (recursivo) y, si el documento está fuera de ella, su directorio.
    pub(super) fn watch_subscriptions(&self) -> Subscription<Message> {
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

    /// Cambios externos: refresca el árbol (AC1) y revisa el documento abierto (AC2–AC5).
    pub(super) fn handle_fs(&mut self, events: &[FsEvent]) -> Task<Message> {
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
    pub(super) fn handle_disk_inspected(
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
    pub(super) fn apply_disk_version(&mut self, disk: Document) -> Task<Message> {
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

    pub(super) fn resolve_conflict(&mut self, choice: ConflictChoice) -> Task<Message> {
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
}

/// Flujo de eventos del sistema de archivos para una subscripción de iced.
pub(super) fn watcher_stream(path: PathBuf, recursive: bool) -> impl Stream<Item = Message> {
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
