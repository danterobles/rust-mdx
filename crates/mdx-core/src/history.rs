//! Historial de deshacer/rehacer basado en diferencias (iced 0.14 no trae uno en `text_editor`).
//!
//! Cada entrada guarda solo el fragmento que cambió, no el documento completo. El historial es
//! lógica pura: no conoce el editor; la UI aplica el [`Patch`] devuelto.

use std::collections::VecDeque;

/// Posición `(línea, columna en bytes)`, ambas base 0, igual que el cursor del editor.
pub type Pos = (usize, usize);

/// Presupuesto de memoria del historial (texto retenido).
const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    offset: usize,
    removed: String,
    inserted: String,
    before: Pos,
    after: Pos,
}

impl Entry {
    fn size(&self) -> usize {
        self.removed.len() + self.inserted.len()
    }
}

/// Reemplazo a aplicar sobre el texto actual: `texto[start..end] = replacement`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    /// Dónde dejar el cursor tras aplicar el parche.
    pub cursor: Pos,
}

/// Historial de edición de un documento.
#[derive(Debug, Default)]
pub struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    bytes: usize,
}

/// Calcula `(offset, removed, inserted)` comparando prefijo y sufijo comunes.
fn diff<'a>(old: &'a str, new: &'a str) -> (usize, &'a str, &'a str) {
    let mut prefix = 0;
    for ((i, a), (_, b)) in old.char_indices().zip(new.char_indices()) {
        if a != b {
            break;
        }
        prefix = i + a.len_utf8();
    }
    let (o, n) = (&old[prefix..], &new[prefix..]);
    let mut suffix = 0;
    for (a, b) in o.chars().rev().zip(n.chars().rev()) {
        if a != b {
            break;
        }
        suffix += a.len_utf8();
    }
    (prefix, &o[..o.len() - suffix], &n[..n.len() - suffix])
}

/// Convierte un offset en bytes a `(línea, columna en bytes)`.
#[must_use]
pub fn offset_to_pos(text: &str, offset: usize) -> Pos {
    let head = &text[..offset.min(text.len())];
    let line = head.matches('\n').count();
    let col = head.rfind('\n').map_or(head.len(), |i| head.len() - i - 1);
    (line, col)
}

/// Convierte `(línea, columna en bytes)` en un offset en bytes dentro de `text`.
/// Si la posición excede el texto, devuelve el final.
#[must_use]
pub fn pos_to_offset(text: &str, (line, col): Pos) -> usize {
    let mut offset = 0;
    for (i, l) in text.split('\n').enumerate() {
        if i == line {
            return offset + col.min(l.len());
        }
        offset += l.len() + 1;
    }
    text.len()
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Vacía el historial (al abrir o crear un documento).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.bytes = 0;
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Registra el cambio `old → new`. Con `merge` se intenta fusionar con la entrada anterior
    /// (tecleo seguido, borrados seguidos) para que un deshacer revierta una ráfaga completa.
    pub fn record(&mut self, old: &str, new: &str, before: Pos, after: Pos, merge: bool) {
        let (offset, removed, inserted) = diff(old, new);
        if removed.is_empty() && inserted.is_empty() {
            return;
        }
        self.redo.clear();
        let entry = Entry {
            offset,
            removed: removed.to_owned(),
            inserted: inserted.to_owned(),
            before,
            after,
        };
        if merge && self.try_merge(&entry) {
            return;
        }
        self.bytes += entry.size();
        self.undo.push_back(entry);
        // Se descarta lo más antiguo, pero nunca la última edición: un «reemplazar todo» en un
        // documento grande puede superar por sí solo el presupuesto y debe poder deshacerse.
        while self.bytes > MAX_BYTES && self.undo.len() > 1 {
            if let Some(old) = self.undo.pop_front() {
                self.bytes -= old.size();
            }
        }
    }

    fn try_merge(&mut self, e: &Entry) -> bool {
        let Some(last) = self.undo.back_mut() else {
            return false;
        };
        let before = last.size();
        let merged = if last.removed.is_empty()
            && e.removed.is_empty()
            && e.offset == last.offset + last.inserted.len()
        {
            // Tecleo contiguo.
            last.inserted.push_str(&e.inserted);
            true
        } else if last.inserted.is_empty()
            && e.inserted.is_empty()
            && e.offset + e.removed.len() == last.offset
        {
            // Retroceso (Backspace) contiguo.
            last.offset = e.offset;
            last.removed.insert_str(0, &e.removed);
            true
        } else if last.inserted.is_empty() && e.inserted.is_empty() && e.offset == last.offset {
            // Suprimir (Delete) contiguo.
            last.removed.push_str(&e.removed);
            true
        } else {
            false
        };
        if merged {
            last.after = e.after;
            self.bytes = self.bytes + last.size() - before;
        }
        merged
    }

    /// Devuelve el parche que revierte la última edición, validándolo contra `text`.
    pub fn undo(&mut self, text: &str) -> Option<Patch> {
        let entry = self.undo.pop_back()?;
        self.bytes -= entry.size();
        let end = entry.offset + entry.inserted.len();
        if !Self::matches(text, entry.offset, end, &entry.inserted) {
            self.clear();
            return None;
        }
        let patch = Patch {
            start: entry.offset,
            end,
            replacement: entry.removed.clone(),
            cursor: entry.before,
        };
        self.redo.push(entry);
        Some(patch)
    }

    /// Devuelve el parche que reaplica la última edición deshecha.
    pub fn redo(&mut self, text: &str) -> Option<Patch> {
        let entry = self.redo.pop()?;
        let end = entry.offset + entry.removed.len();
        if !Self::matches(text, entry.offset, end, &entry.removed) {
            self.clear();
            return None;
        }
        let patch = Patch {
            start: entry.offset,
            end,
            replacement: entry.inserted.clone(),
            cursor: entry.after,
        };
        self.bytes += entry.size();
        self.undo.push_back(entry);
        Some(patch)
    }

    fn matches(text: &str, start: usize, end: usize, expected: &str) -> bool {
        text.get(start..end) == Some(expected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn apply(text: &str, p: &Patch) -> String {
        format!("{}{}{}", &text[..p.start], p.replacement, &text[p.end..])
    }

    #[test]
    fn undo_and_redo_roundtrip() {
        let mut h = History::new();
        h.record("hola", "hola mundo", (0, 4), (0, 10), false);
        let p = h.undo("hola mundo").unwrap();
        assert_eq!(apply("hola mundo", &p), "hola");
        assert_eq!(p.cursor, (0, 4));
        let r = h.redo("hola").unwrap();
        assert_eq!(apply("hola", &r), "hola mundo");
        assert_eq!(r.cursor, (0, 10));
    }

    #[test]
    fn typing_burst_is_one_undo_step() {
        let mut h = History::new();
        h.record("", "a", (0, 0), (0, 1), true);
        h.record("a", "ab", (0, 1), (0, 2), true);
        h.record("ab", "abc", (0, 2), (0, 3), true);
        let p = h.undo("abc").unwrap();
        assert_eq!(apply("abc", &p), "");
        assert!(!h.can_undo());
    }

    #[test]
    fn non_merged_edits_are_separate_steps() {
        let mut h = History::new();
        h.record("", "a", (0, 0), (0, 1), false);
        h.record("a", "ab", (0, 1), (0, 2), false);
        let p = h.undo("ab").unwrap();
        assert_eq!(apply("ab", &p), "a");
    }

    #[test]
    fn backspace_burst_merges() {
        let mut h = History::new();
        h.record("abc", "ab", (0, 3), (0, 2), true);
        h.record("ab", "a", (0, 2), (0, 1), true);
        let p = h.undo("a").unwrap();
        assert_eq!(apply("a", &p), "abc");
        assert_eq!(p.cursor, (0, 3));
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut h = History::new();
        h.record("", "a", (0, 0), (0, 1), false);
        let _ = h.undo("a");
        assert!(h.can_redo());
        h.record("", "z", (0, 0), (0, 1), false);
        assert!(!h.can_redo());
    }

    #[test]
    fn replacement_and_multibyte() {
        let mut h = History::new();
        h.record("añb", "aéb", (0, 1), (0, 3), false);
        let p = h.undo("aéb").unwrap();
        assert_eq!(apply("aéb", &p), "añb");
    }

    #[test]
    fn diverged_text_clears_history_instead_of_corrupting() {
        let mut h = History::new();
        h.record("", "abc", (0, 0), (0, 3), false);
        assert_eq!(h.undo("otra cosa"), None);
        assert!(!h.can_undo() && !h.can_redo());
    }

    #[test]
    fn no_change_records_nothing() {
        let mut h = History::new();
        h.record("igual", "igual", (0, 0), (0, 0), false);
        assert!(!h.can_undo());
    }

    #[test]
    fn offset_to_pos_counts_lines_and_byte_columns() {
        assert_eq!(offset_to_pos("ab\ncdé\nf", 0), (0, 0));
        assert_eq!(offset_to_pos("ab\ncdé\nf", 2), (0, 2));
        assert_eq!(offset_to_pos("ab\ncdé\nf", 3), (1, 0));
        assert_eq!(offset_to_pos("ab\ncdé\nf", 7), (1, 4));
        assert_eq!(offset_to_pos("ab\ncdé\nf", 8), (2, 0));
    }

    #[test]
    fn pos_to_offset_is_the_inverse_of_offset_to_pos() {
        let text = "ab\ncdé\nf";
        for offset in [0, 1, 2, 3, 5, 7, 8, 9] {
            assert_eq!(pos_to_offset(text, offset_to_pos(text, offset)), offset);
        }
        assert_eq!(pos_to_offset(text, (99, 0)), text.len());
        assert_eq!(
            pos_to_offset(text, (0, 99)),
            2,
            "la columna se acota a la línea"
        );
    }

    #[test]
    fn a_single_edit_larger_than_the_budget_is_still_undoable() {
        let mut h = History::new();
        let old = "a".repeat(MAX_BYTES);
        let new = "b".repeat(MAX_BYTES);
        h.record(&old, &new, (0, 0), (0, 0), false);
        let patch = h.undo(&new).expect("la última edición se conserva");
        assert_eq!(patch.replacement.len(), MAX_BYTES);
        // Una edición posterior sí desaloja a la anterior si se pasa del presupuesto.
        let mut h = History::new();
        h.record(&old, &new, (0, 0), (0, 0), false);
        h.record(&new, "c", (0, 0), (0, 0), false);
        assert!(h.can_undo());
        assert!(h.bytes <= MAX_BYTES + 1);
    }

    #[test]
    fn history_is_bounded() {
        let mut h = History::new();
        let big = "x".repeat(2 * 1024 * 1024);
        let mut text = String::new();
        for _ in 0..5 {
            let new = format!("{text}{big}");
            h.record(&text, &new, (0, 0), (0, 0), false);
            text = new;
        }
        assert!(h.bytes <= MAX_BYTES);
        assert!(h.can_undo());
    }
}
