//! Varios documentos abiertos (pestañas): un documento **activo** y el resto **aparcados**.
//!
//! El estado del documento activo vive en los campos de [`App`] (`doc`, `content`, `history`, …)
//! y el de los demás en `App::parked`. Cambiar de pestaña intercambia ambos (ADR 0006). Los
//! eventos asíncronos dirigidos a un documento concreto usan [`App::with_doc`], que lo activa un
//! instante y restaura el activo.

use std::collections::HashMap;
use std::mem;

use super::session_ops::DocSummary;
use super::{
    App, Block, DiskState, Document, DocumentId, History, Instant, Layout, Message, Outline, Path,
    PendingAction, Task, markdown, strings, text_editor,
};

/// Estado completo de un documento abierto que no es el activo.
pub(super) struct OpenDoc {
    pub(super) doc: Document,
    pub(super) content: text_editor::Content,
    pub(super) dirty: bool,
    pub(super) history: History,
    pub(super) last_edit_at: Option<Instant>,
    pub(super) words: usize,
    pub(super) preview: Vec<markdown::Item>,
    pub(super) preview_gen: u64,
    pub(super) preview_pending: bool,
    pub(super) preview_edit_at: Instant,
    pub(super) external: Option<Document>,
    pub(super) outline: Outline,
    pub(super) blocks: Vec<Block>,
    pub(super) view_top: (usize, f32),
    pub(super) last_change: Instant,
}

pub(super) type Parked = HashMap<DocumentId, OpenDoc>;

impl App {
    /// Saca el estado del documento activo dejando valores vacíos en su lugar.
    fn take_active(&mut self) -> OpenDoc {
        OpenDoc {
            doc: mem::replace(&mut self.doc, Document::untitled()),
            content: mem::replace(&mut self.content, text_editor::Content::new()),
            dirty: mem::take(&mut self.dirty),
            history: mem::take(&mut self.history),
            last_edit_at: self.last_edit_at.take(),
            words: mem::take(&mut self.words),
            preview: mem::take(&mut self.preview),
            preview_gen: self.preview_gen,
            preview_pending: mem::take(&mut self.preview_pending),
            preview_edit_at: self.preview_edit_at,
            external: self.external.take(),
            outline: mem::take(&mut self.outline),
            blocks: mem::take(&mut self.blocks),
            view_top: mem::take(&mut self.view_top),
            last_change: self.last_change,
        }
    }

    fn put_active(&mut self, d: OpenDoc) {
        self.doc = d.doc;
        self.content = d.content;
        self.dirty = d.dirty;
        self.history = d.history;
        self.last_edit_at = d.last_edit_at;
        self.words = d.words;
        self.preview = d.preview;
        self.preview_gen = d.preview_gen;
        self.preview_pending = d.preview_pending;
        self.preview_edit_at = d.preview_edit_at;
        self.external = d.external;
        self.outline = d.outline;
        self.blocks = d.blocks;
        self.view_top = d.view_top;
        self.last_change = d.last_change;
    }

    /// Intercambia el documento activo con el aparcado `id`. No toca el modelo de pestañas.
    fn swap_active(&mut self, id: DocumentId) -> bool {
        if id == self.doc.id {
            return true;
        }
        let Some(next) = self.parked.remove(&id) else {
            return false;
        };
        let current = self.take_active();
        self.parked.insert(current.doc.id, current);
        self.put_active(next);
        true
    }

    /// Ejecuta `f` con `id` como documento activo y luego restaura el que lo era.
    /// Devuelve `None` si el documento ya no está abierto.
    pub(super) fn with_doc<R>(
        &mut self,
        id: DocumentId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        let previous = self.doc.id;
        if !self.swap_active(id) {
            return None;
        }
        let result = f(self);
        if self.doc.id != previous {
            self.swap_active(previous);
        }
        Some(result)
    }

    /// Todos los documentos abiertos (activo y aparcados).
    pub(super) fn open_docs_mut(&mut self, mut f: impl FnMut(&mut Document, bool)) {
        f(&mut self.doc, self.dirty);
        for parked in self.parked.values_mut() {
            f(&mut parked.doc, parked.dirty);
        }
    }

    pub(super) fn open_docs(&self) -> impl Iterator<Item = (&Document, bool)> {
        std::iter::once((&self.doc, self.dirty))
            .chain(self.parked.values().map(|p| (&p.doc, p.dirty)))
    }

    /// Resumen de todos los documentos abiertos (para el autoguardado).
    pub(super) fn doc_summaries(&self) -> Vec<DocSummary> {
        let active = DocSummary {
            id: self.doc.id,
            path_is_set: self.doc.path.is_some(),
            dirty: self.dirty,
            disk_state: self.doc.disk_state,
            last_change: self.last_change,
        };
        std::iter::once(active)
            .chain(self.parked.values().map(|p| DocSummary {
                id: p.doc.id,
                path_is_set: p.doc.path.is_some(),
                dirty: p.dirty,
                disk_state: p.doc.disk_state,
                last_change: p.last_change,
            }))
            .collect()
    }

    pub(super) fn find_open(&self, path: &Path) -> Option<DocumentId> {
        self.open_docs()
            .find(|(d, _)| d.path.as_deref() == Some(path))
            .map(|(d, _)| d.id)
    }

    /// Primer documento con cambios sin guardar (el activo primero).
    pub(super) fn first_dirty(&self) -> Option<DocumentId> {
        self.open_docs()
            .find(|(_, dirty)| *dirty)
            .map(|(d, _)| d.id)
    }

    pub(super) fn is_dirty(&self, id: DocumentId) -> bool {
        self.open_docs().any(|(d, dirty)| d.id == id && dirty)
    }

    pub(super) fn doc_name(&self, id: DocumentId) -> String {
        self.open_docs()
            .find(|(d, _)| d.id == id)
            .map_or_else(String::new, |(d, _)| d.display_name(strings::UNTITLED))
    }

    /// Activa la pestaña `id` (modelo y estado).
    pub(super) fn select_tab(&mut self, id: DocumentId) -> Task<Message> {
        if !self.swap_active(id) {
            return Task::none();
        }
        self.tabs.set_active(id);
        // Las medidas de la vista previa eran del documento anterior: se vuelve a medir.
        self.preview_layout = Layout::default();
        self.refresh_find_matches();
        self.measure_preview()
    }

    pub(super) fn cycle_tab(&mut self, delta: isize) -> Task<Message> {
        match self.tabs.neighbour(delta) {
            Some(id) => self.select_tab(id),
            None => Task::none(),
        }
    }

    pub(super) fn goto_tab(&mut self, n: usize) -> Task<Message> {
        match self.tabs.nth(n) {
            Some(id) => self.select_tab(id),
            None => Task::none(),
        }
    }

    /// Añade `doc` como nueva pestaña activa. Si ya estaba abierto solo activa su pestaña.
    pub(super) fn add_doc(&mut self, doc: Document, pinned: bool) -> Task<Message> {
        if let Some(id) = doc.path.as_deref().and_then(|p| self.find_open(p)) {
            if pinned {
                self.tabs.pin(id);
            }
            return self.select_tab(id);
        }
        // Un «Sin título» vacío y sin tocar se reemplaza en vez de acumularse.
        let pristine = self.doc.path.is_none() && !self.dirty && self.doc.text.is_empty();
        let current = self.take_active();
        if pristine {
            self.tabs.close(current.doc.id);
        } else {
            self.parked.insert(current.doc.id, current);
        }
        let replaced = if pinned {
            self.tabs.open_pinned(doc.id);
            None
        } else {
            self.tabs.open_preview(doc.id)
        };
        if let Some(id) = replaced {
            self.parked.remove(&id);
        }
        self.content = text_editor::Content::with_text(&doc.text);
        self.doc = doc;
        self.reset_editing_state()
    }

    /// Ctrl/Cmd+N: nueva pestaña «Sin título» (si la actual ya es una vacía, no se duplica).
    pub(super) fn new_untitled(&mut self) -> Task<Message> {
        if self.doc.path.is_none() && !self.dirty && self.doc.text.is_empty() {
            self.tabs.pin(self.doc.id);
            self.set_status(strings::STATUS_NEW.to_owned(), false);
            return Task::none();
        }
        let current = self.take_active();
        self.parked.insert(current.doc.id, current);
        let doc = Document::untitled();
        self.tabs.open_pinned(doc.id);
        self.doc = doc;
        self.content = text_editor::Content::new();
        let task = self.reset_editing_state();
        self.set_status(strings::STATUS_NEW.to_owned(), false);
        task
    }

    /// Cierra la pestaña `id` sin preguntar (el llamador ya resolvió los cambios sin guardar).
    /// Si era la última, queda un «Sin título» vacío.
    pub(super) fn close_tab(&mut self, id: DocumentId) -> Task<Message> {
        let was_active = id == self.doc.id;
        self.tabs.close(id);
        if !was_active {
            self.parked.remove(&id);
            return Task::none();
        }
        let next = self.tabs.active().and_then(|id| self.parked.remove(&id));
        if let Some(next) = next {
            self.put_active(next);
            self.preview_layout = Layout::default();
            self.refresh_find_matches();
            return self.measure_preview();
        }
        let doc = Document::untitled();
        self.tabs.open_pinned(doc.id);
        self.doc = doc;
        self.content = text_editor::Content::new();
        self.reset_editing_state()
    }

    /// Abre `path` en una pestaña (la reutiliza si ya está abierta). Con `pin` queda fijada.
    pub(super) fn open_path(&mut self, path: std::path::PathBuf, pin: bool) -> Task<Message> {
        if let Some(id) = self.find_open(&path) {
            if pin {
                self.tabs.pin(id);
            }
            return self.select_tab(id);
        }
        if pin {
            self.pin_on_load = Some(path.clone());
        }
        Task::perform(
            async move { mdx_core::io::read_document(&path).await },
            Message::DocumentLoaded,
        )
    }

    /// Datos de la pestaña `id` para la barra.
    pub(super) fn tab_label(&self, id: DocumentId) -> Option<TabLabel> {
        let pinned = self.tabs.is_pinned(id);
        self.open_docs()
            .find(|(d, _)| d.id == id)
            .map(|(d, dirty)| TabLabel {
                id,
                name: d.display_name(strings::UNTITLED),
                full_path: d
                    .path
                    .as_ref()
                    .map_or_else(|| strings::UNTITLED.to_owned(), |p| p.display().to_string()),
                dirty,
                deleted: d.disk_state == DiskState::Deleted,
                pinned,
            })
    }

    /// Documento al que se refiere el diálogo de cambios sin guardar de `action`.
    pub(super) fn guard_target(&self, action: &PendingAction) -> Option<DocumentId> {
        match action {
            PendingAction::Close => self.first_dirty(),
            PendingAction::CloseTab(id) => Some(*id),
        }
    }
}

/// Datos de una pestaña para dibujar la barra.
pub(super) struct TabLabel {
    pub(super) id: DocumentId,
    pub(super) name: String,
    pub(super) full_path: String,
    pub(super) dirty: bool,
    pub(super) deleted: bool,
    pub(super) pinned: bool,
}
