//! Buscar y reemplazar en el documento y buscar en la carpeta (US-3.4).

use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::widget::text_editor::{Action, Cursor, Edit, Position};
use mdx_core::history::{offset_to_pos, pos_to_offset};
use mdx_core::search::{self, find_all, replace_all, replacement_for};

use super::{App, Message, Sidebar, Task, strings};
use crate::search::{FIND_INPUT_ID, WS_INPUT_ID};

fn position((line, column): (usize, usize)) -> Position {
    Position { line, column }
}

impl App {
    /// Mensajes de buscar/reemplazar y de la búsqueda en carpeta.
    pub(super) fn update_search(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FindQueryChanged(pattern) => {
                self.find.query.pattern = pattern;
                self.find.notice = None;
                self.refresh_find(true)
            }
            Message::ReplaceChanged(text) => {
                self.find.replacement = text;
                Task::none()
            }
            Message::FindOptionToggled(option) => {
                option.toggle(&mut self.find.query);
                self.refresh_find(true)
            }
            Message::FindNext => self.step_match(1),
            Message::FindPrev => self.step_match(-1),
            Message::ReplaceOne => self.replace_one(),
            Message::ReplaceAll => self.replace_every_match(),
            Message::FindClose => {
                self.close_find();
                Task::none()
            }
            Message::WsQueryChanged(pattern) => {
                self.ws_search.query.pattern = pattern;
                Task::none()
            }
            Message::WsOptionToggled(option) => {
                option.toggle(&mut self.ws_search.query);
                Task::none()
            }
            Message::WsSearch => self.start_workspace_search(),
            Message::WsCancel => {
                if let Some(flag) = &self.ws_search.cancel {
                    flag.store(true, Ordering::Relaxed);
                }
                Task::none()
            }
            Message::WsDone(generation, result) => {
                if generation == self.ws_search.generation {
                    self.ws_search.running = false;
                    self.ws_search.cancel = None;
                    match result {
                        Ok(found) => self.ws_search.result = Some(found),
                        Err(err) => self.ws_search.error = Some(err.to_string()),
                    }
                }
                Task::none()
            }
            Message::OpenResult(path, line, cols) => {
                let already_open = self.find_open(&path).is_some();
                let open = self.open_path(path.clone(), false);
                if already_open {
                    let goto = self.goto_match(line, &cols);
                    Task::batch([open, goto])
                } else {
                    self.pending_goto = Some((path, line, cols));
                    open
                }
            }
            _ => Task::none(),
        }
    }

    /// Ctrl/Cmd+F (buscar) o Ctrl/Cmd+H (reemplazar): abre la barra y enfoca el campo.
    pub(super) fn open_find(&mut self, replace: bool) -> Task<Message> {
        let was_open = self.find.open;
        self.find.open = true;
        self.find.replace_mode = replace || self.find.replace_mode;
        self.find.notice = None;
        // Solo al abrir la barra: con ella ya abierta, la selección es la coincidencia actual.
        if !was_open && let Some(selected) = self.single_line_selection() {
            self.find.query.pattern = selected;
        }
        self.refresh_find_matches();
        iced::widget::operation::focus(iced::widget::Id::new(FIND_INPUT_ID))
    }

    /// Ctrl/Cmd+Shift+F: muestra el panel de búsqueda en carpeta y enfoca su campo.
    pub(super) fn open_workspace_search(&mut self) -> Task<Message> {
        self.sidebar = Sidebar::Search;
        if let Some(selected) = self.single_line_selection() {
            self.ws_search.query.pattern = selected;
        }
        iced::widget::operation::focus(iced::widget::Id::new(WS_INPUT_ID))
    }

    /// Texto seleccionado en el editor si cabe en una línea (sirve de patrón inicial).
    fn single_line_selection(&self) -> Option<String> {
        self.content
            .selection()
            .filter(|s| !s.is_empty() && !s.contains('\n'))
    }

    pub(super) fn close_find(&mut self) {
        self.find.open = false;
        self.find.matches.clear();
        self.find.current = None;
        self.find.notice = None;
    }

    /// Recalcula las coincidencias en el texto actual sin tocar la selección del editor. La
    /// «actual» es la primera que empieza en o después del cursor (con vuelta al principio).
    pub(super) fn refresh_find_matches(&mut self) {
        if !self.find.open {
            return;
        }
        match find_all(&self.doc.text, &self.find.query) {
            Ok(matches) => {
                self.find.matches = matches;
                self.find.error = None;
            }
            Err(err) => {
                self.find.matches.clear();
                self.find.error = Some(err.to_string());
            }
        }
        let cursor = self.content.cursor();
        let anchor = [Some(cursor.position), cursor.selection]
            .into_iter()
            .flatten()
            .map(|p| pos_to_offset(&self.doc.text, (p.line, p.column)))
            .min()
            .unwrap_or(0);
        self.find.current = if self.find.matches.is_empty() {
            None
        } else {
            Some(
                self.find
                    .matches
                    .iter()
                    .position(|m| m.start >= anchor)
                    .unwrap_or(0),
            )
        };
    }

    fn refresh_find(&mut self, select: bool) -> Task<Message> {
        self.refresh_find_matches();
        match self.find.current {
            Some(index) if select => self.select_match(index),
            _ => Task::none(),
        }
    }

    /// Selecciona la coincidencia `index` en el editor y la deja visible.
    fn select_match(&mut self, index: usize) -> Task<Message> {
        let Some(range) = self.find.matches.get(index).cloned() else {
            return Task::none();
        };
        self.find.current = Some(index);
        let start = offset_to_pos(&self.doc.text, range.start);
        let end = offset_to_pos(&self.doc.text, range.end);
        let scroll = self.go_to_line(start.0);
        self.content.move_to(Cursor {
            position: position(end),
            selection: Some(position(start)),
        });
        scroll
    }

    fn step_match(&mut self, delta: isize) -> Task<Message> {
        let total = self.find.matches.len();
        if total == 0 {
            return Task::none();
        }
        let next = match self.find.current {
            Some(i) => {
                let step = i64::try_from(i).unwrap_or(0) + i64::try_from(delta).unwrap_or(0);
                usize::try_from(step.rem_euclid(i64::try_from(total).unwrap_or(1))).unwrap_or(0)
            }
            None if delta >= 0 => 0,
            None => total - 1,
        };
        self.select_match(next)
    }

    /// Reemplaza la coincidencia actual y salta a la siguiente.
    fn replace_one(&mut self) -> Task<Message> {
        let Some(index) = self.find.current else {
            return Task::none();
        };
        let Some(range) = self.find.matches.get(index).cloned() else {
            return Task::none();
        };
        let replacement = match replacement_for(
            &self.doc.text,
            &self.find.query,
            &range,
            &self.find.replacement,
        ) {
            Ok(r) => r,
            Err(err) => {
                self.find.error = Some(err.to_string());
                return Task::none();
            }
        };
        self.content.move_to(Cursor {
            position: position(offset_to_pos(&self.doc.text, range.end)),
            selection: Some(position(offset_to_pos(&self.doc.text, range.start))),
        });
        let edit = self.apply_text_edit(&replacement);
        self.find.notice = Some(strings::FIND_REPLACED_ONE.to_owned());
        // `handle_edit` ya recalculó las coincidencias; se selecciona la siguiente.
        let next = match self.find.current {
            Some(i) => self.select_match(i),
            None => Task::none(),
        };
        Task::batch([edit, next])
    }

    /// Reemplaza todas las coincidencias en un solo paso de deshacer.
    fn replace_every_match(&mut self) -> Task<Message> {
        let (replaced, count) =
            match replace_all(&self.doc.text, &self.find.query, &self.find.replacement) {
                Ok(r) => r,
                Err(err) => {
                    self.find.error = Some(err.to_string());
                    return Task::none();
                }
            };
        if count == 0 {
            self.find.notice = Some(strings::FIND_NO_RESULTS.to_owned());
            return Task::none();
        }
        let first = self.find.matches.first().map_or(0, |m| m.start);
        let cursor = offset_to_pos(&self.doc.text, first);
        let edit = self.apply_bulk_edit(&replaced, cursor);
        self.find.notice = Some(if count == 1 {
            strings::FIND_REPLACED_ONE.to_owned()
        } else {
            strings::FIND_REPLACED.replace("{n}", &count.to_string())
        });
        edit
    }

    /// Sustituye la selección actual del editor por `text` como una edición normal
    /// (historial, «sin guardar», vista previa).
    pub(super) fn apply_text_edit(&mut self, text: &str) -> Task<Message> {
        let action = if text.is_empty() {
            Action::Edit(Edit::Delete)
        } else {
            Action::Edit(Edit::Paste(Arc::new(text.to_owned())))
        };
        self.handle_edit(action)
    }

    /// Lanza la búsqueda en toda la carpeta abierta (cancela la anterior).
    fn start_workspace_search(&mut self) -> Task<Message> {
        let Some(root) = self.explorer.workspace.as_ref().map(|ws| ws.root.clone()) else {
            return Task::none();
        };
        if let Some(previous) = self.ws_search.cancel.take() {
            previous.store(true, Ordering::Relaxed);
        }
        self.ws_search.result = None;
        self.ws_search.error = None;
        if self.ws_search.query.pattern.is_empty() {
            self.ws_search.running = false;
            return Task::none();
        }
        self.ws_search.generation += 1;
        self.ws_search.running = true;
        let generation = self.ws_search.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.ws_search.cancel = Some(cancel.clone());
        Task::perform(
            search::search_workspace_async(root, self.ws_search.query.clone(), cancel),
            move |result| Message::WsDone(generation, result),
        )
    }

    /// Posiciona el editor en una coincidencia de la búsqueda en carpeta (línea y columnas).
    pub(super) fn goto_match(&mut self, line: usize, cols: &Range<usize>) -> Task<Message> {
        let line = line.min(self.content.line_count().saturating_sub(1));
        let len = self.content.line(line).map_or(0, |l| l.text.len());
        let (start, end) = (cols.start.min(len), cols.end.min(len));
        let scroll = self.go_to_line(line);
        self.content.move_to(Cursor {
            position: position((line, end)),
            selection: Some(position((line, start))),
        });
        scroll
    }
}
