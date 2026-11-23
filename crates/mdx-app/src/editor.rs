//! Utilidades del editor: atajos propios del `text_editor` y aplicación de parches de historial.

use std::sync::Arc;

use iced::keyboard::{Key, key::Named};
use iced::widget::text_editor::{
    self, Action, Binding, Content, Cursor, Edit, KeyPress, Position, Status,
};
use mdx_core::history::{Patch, Pos, offset_to_pos};

use crate::app::Message;

/// `Tab` / `Shift+Tab` indentan y desindentan; el resto usa el comportamiento por defecto.
pub fn key_binding(key_press: KeyPress) -> Option<Binding<Message>> {
    let focused = matches!(key_press.status, Status::Focused { .. });
    if focused
        && key_press.key == Key::Named(Named::Tab)
        && !key_press.modifiers.command()
        && !key_press.modifiers.control()
    {
        let edit = if key_press.modifiers.shift() {
            Edit::Unindent
        } else {
            Edit::Indent
        };
        return Some(Binding::Custom(Message::Edit(Action::Edit(edit))));
    }
    Binding::from_key_press(key_press)
}

/// Posición del cursor `(línea, columna en bytes)`.
#[must_use]
pub fn cursor_pos(content: &Content) -> Pos {
    let p = content.cursor().position;
    (p.line, p.column)
}

fn position((line, column): Pos) -> Position {
    Position { line, column }
}

/// Aplica `patch` sobre `content` (cuyo texto actual es `text`) sin reconstruir el editor,
/// para conservar el scroll.
pub fn apply_patch(content: &mut Content, text: &str, patch: &Patch) {
    let start = offset_to_pos(text, patch.start);
    let end = offset_to_pos(text, patch.end);
    let selection = (patch.start != patch.end).then(|| position(start));
    content.move_to(Cursor {
        position: position(end),
        selection,
    });
    if patch.replacement.is_empty() {
        content.perform(Action::Edit(Edit::Delete));
    } else {
        content.perform(Action::Edit(Edit::Paste(Arc::new(
            patch.replacement.clone(),
        ))));
    }
    content.move_to(Cursor {
        position: position(patch.cursor),
        selection: None,
    });
}

/// Primera fila visible del editor como `(línea, fracción recorrida de esa línea)`.
///
/// `text_editor` no expone su scroll, así que se «sondea»: se hace clic virtual en la esquina
/// superior izquierda del contenido, se lee dónde cae el cursor y se restaura el cursor y la
/// selección originales. Es barato y no modifica el texto.
pub fn top_visible_line(content: &mut Content) -> (usize, f32) {
    let saved = content.cursor();
    content.perform(Action::Click(iced::Point::ORIGIN));
    let top = content.cursor().position;
    content.move_to(saved);
    let len = content.line(top.line).map_or(0, |l| l.text.len());
    #[allow(clippy::cast_precision_loss)]
    let fraction = if len == 0 {
        0.0
    } else {
        top.column as f32 / len as f32
    };
    (top.line, fraction)
}

/// `true` si la acción continúa una ráfaga de tecleo/borrado (se fusiona en el historial).
#[must_use]
pub fn is_mergeable(action: &text_editor::Action) -> bool {
    matches!(
        action,
        Action::Edit(Edit::Insert(c)) if !c.is_whitespace() || *c == ' '
    ) || matches!(action, Action::Edit(Edit::Backspace | Edit::Delete))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reads_the_first_visible_line_without_moving_the_cursor() {
        let text = "línea\n".repeat(100);
        let mut content = Content::with_text(&text);
        content.move_to(Cursor {
            position: Position {
                line: 50,
                column: 3,
            },
            selection: None,
        });
        assert_eq!(top_visible_line(&mut content).0, 0);
        content.perform(Action::Scroll { lines: 10 });
        assert_eq!(top_visible_line(&mut content).0, 10);
        content.perform(Action::Scroll { lines: 25 });
        assert_eq!(top_visible_line(&mut content).0, 35);
        // El cursor del usuario sigue donde estaba.
        assert_eq!(cursor_pos(&content), (50, 3));
    }
}
