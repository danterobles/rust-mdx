//! Modelo de pestañas: una pestaña «preview» reutilizable (INV-7) y las demás fijadas.
//!
//! El orden del `Vec` es el orden visible de la barra de pestañas.

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

    /// Activa la pestaña de `doc`. Devuelve `false` si no existe.
    pub fn set_active(&mut self, doc: DocumentId) -> bool {
        let exists = self.tabs.iter().any(|t| t.doc == doc);
        if exists {
            self.active = Some(doc);
        }
        exists
    }

    /// Pestaña vecina a la activa, circular: `+1` siguiente, `-1` anterior.
    #[must_use]
    pub fn neighbour(&self, delta: isize) -> Option<DocumentId> {
        let len = isize::try_from(self.tabs.len()).ok()?;
        if len == 0 {
            return None;
        }
        let current = self
            .active
            .and_then(|a| self.tabs.iter().position(|t| t.doc == a))
            .and_then(|i| isize::try_from(i).ok())?;
        let index = usize::try_from((current + delta).rem_euclid(len)).ok()?;
        self.tabs.get(index).map(|t| t.doc)
    }

    /// Pestaña en la posición `n` (base 1). `Ctrl/Cmd+9` salta siempre a la última, como en
    /// los navegadores.
    #[must_use]
    pub fn nth(&self, n: usize) -> Option<DocumentId> {
        match n {
            0 => None,
            9 => self.tabs.last().map(|t| t.doc),
            _ => self.tabs.get(n - 1).map(|t| t.doc),
        }
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
    fn cycles_circularly_and_jumps_by_position() {
        // AC2
        let d = ids(3);
        let mut tabs = Tabs::new();
        for id in &d {
            tabs.open_pinned(*id);
        }
        assert_eq!(tabs.active(), Some(d[2]));
        assert_eq!(
            tabs.neighbour(1),
            Some(d[0]),
            "después de la última, la primera"
        );
        assert_eq!(tabs.neighbour(-1), Some(d[1]));
        assert!(tabs.set_active(d[0]));
        assert_eq!(
            tabs.neighbour(-1),
            Some(d[2]),
            "antes de la primera, la última"
        );
        assert_eq!(tabs.nth(2), Some(d[1]));
        assert_eq!(tabs.nth(1), Some(d[0]));
        assert_eq!(tabs.nth(5), None);
        assert_eq!(tabs.nth(9), Some(d[2]), "9 = última");
        assert_eq!(tabs.nth(0), None);
        assert!(!tabs.set_active(DocumentId::next()));
        assert_eq!(Tabs::new().neighbour(1), None);
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
