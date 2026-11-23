//! Modelo de pestañas: una pestaña «preview» reutilizable (INV-7) y las demás fijadas.
//!
//! En US-2.2 solo se usa el modelo; la barra de pestañas visible llega en US-3.1.

use mdx_core::DocumentId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tab {
    pub doc: DocumentId,
    /// `false` = pestaña «preview», que el siguiente clic simple reemplaza.
    pub pinned: bool,
}

#[derive(Debug, Default)]
pub struct Tabs {
    tabs: Vec<Tab>,
    active: Option<DocumentId>,
}

// `tabs`, `active` e `is_pinned` los consumirá la barra de pestañas de US-3.1.
#[allow(dead_code)]
impl Tabs {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    #[must_use]
    pub fn active(&self) -> Option<DocumentId> {
        self.active
    }

    #[must_use]
    pub fn is_pinned(&self, doc: DocumentId) -> bool {
        self.tabs.iter().any(|t| t.doc == doc && t.pinned)
    }

    /// Abre `doc` como pestaña «preview»: reemplaza la preview existente (si la hay) o añade una.
    /// Devuelve el documento de la preview reemplazada, que debe cerrarse.
    pub fn open_preview(&mut self, doc: DocumentId) -> Option<DocumentId> {
        self.active = Some(doc);
        if let Some(existing) = self.tabs.iter_mut().find(|t| !t.pinned) {
            let replaced = existing.doc;
            existing.doc = doc;
            return (replaced != doc).then_some(replaced);
        }
        self.tabs.push(Tab { doc, pinned: false });
        None
    }

    /// Abre `doc` como pestaña fija (documento nuevo o abierto con doble clic).
    pub fn open_pinned(&mut self, doc: DocumentId) {
        self.active = Some(doc);
        match self.tabs.iter_mut().find(|t| t.doc == doc) {
            Some(tab) => tab.pinned = true,
            None => self.tabs.push(Tab { doc, pinned: true }),
        }
    }

    /// Fija una pestaña existente (doble clic sobre una preview).
    pub fn pin(&mut self, doc: DocumentId) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.doc == doc) {
            tab.pinned = true;
        }
    }

    /// Cierra la pestaña de `doc`; si era la activa, activa la vecina anterior.
    pub fn close(&mut self, doc: DocumentId) {
        let Some(pos) = self.tabs.iter().position(|t| t.doc == doc) else {
            return;
        };
        self.tabs.remove(pos);
        if self.active == Some(doc) {
            self.active = self.tabs.get(pos.saturating_sub(1)).map(|t| t.doc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<DocumentId> {
        (0..n).map(|_| DocumentId::next()).collect()
    }

    fn unpinned(tabs: &Tabs) -> usize {
        tabs.tabs().iter().filter(|t| !t.pinned).count()
    }

    #[test]
    fn single_click_reuses_the_preview_tab() {
        // INV-7: a lo sumo una pestaña sin fijar.
        let d = ids(3);
        let mut tabs = Tabs::new();
        assert_eq!(tabs.open_preview(d[0]), None);
        assert_eq!(tabs.open_preview(d[1]), Some(d[0]));
        assert_eq!(tabs.open_preview(d[2]), Some(d[1]));
        assert_eq!(tabs.tabs().len(), 1);
        assert_eq!(tabs.active(), Some(d[2]));
        assert_eq!(unpinned(&tabs), 1);
    }

    #[test]
    fn double_click_pins_and_next_click_opens_a_new_preview() {
        let d = ids(3);
        let mut tabs = Tabs::new();
        tabs.open_preview(d[0]);
        tabs.pin(d[0]);
        assert!(tabs.is_pinned(d[0]));
        assert_eq!(tabs.open_preview(d[1]), None, "la fijada no se reemplaza");
        assert_eq!(tabs.tabs().len(), 2);
        assert_eq!(tabs.open_preview(d[2]), Some(d[1]));
        assert_eq!(unpinned(&tabs), 1);
    }

    #[test]
    fn open_pinned_never_creates_a_second_preview() {
        let d = ids(3);
        let mut tabs = Tabs::new();
        tabs.open_preview(d[0]);
        tabs.open_pinned(d[1]);
        tabs.open_pinned(d[0]); // fija la preview existente
        assert_eq!(unpinned(&tabs), 0);
        assert_eq!(tabs.tabs().len(), 2);
    }

    #[test]
    fn close_activates_previous_neighbour() {
        let d = ids(3);
        let mut tabs = Tabs::new();
        for id in &d {
            tabs.open_pinned(*id);
        }
        tabs.close(d[2]);
        assert_eq!(tabs.active(), Some(d[1]));
        tabs.close(d[0]);
        tabs.close(d[1]);
        assert_eq!(tabs.active(), None);
    }
}
