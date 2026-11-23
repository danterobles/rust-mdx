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
    // Cmd/Ctrl+V: si el portapapeles trae una imagen se guarda junto al documento (US-4.4);
    // si no, la aplicación pega el texto.
    if focused
        && key_press.modifiers.command()
        && !key_press.modifiers.alt()
        && key_press.key.to_latin(key_press.physical_key) == Some('v')
    {
        return Some(Binding::Custom(Message::PasteRequested));
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

/// Fuente del editor según `[editor] font_family`: `monospace` (o una fuente que no está
/// instalada) usa la monoespaciada del sistema.
#[must_use]
pub fn resolve_font(family: &str) -> iced::Font {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    // `Font` pide un nombre `'static`: se conserva uno por familia distinta (pocas, en la vida
    // de la aplicación) en vez de filtrar una cadena en cada recarga de la configuración.
    static NAMES: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

    let family = family.trim();
    if family.is_empty() || family.eq_ignore_ascii_case("monospace") || !font_is_installed(family) {
        return iced::Font::MONOSPACE;
    }
    let mut names = NAMES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let name = *names
        .entry(family.to_owned())
        .or_insert_with(|| Box::leak(family.to_owned().into_boxed_str()));
    iced::Font::with_name(name)
}

fn font_is_installed(family: &str) -> bool {
    let Ok(mut system) = iced::advanced::graphics::text::font_system().write() else {
        return false;
    };
    system.raw().db().faces().any(|face| {
        face.families
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(family))
    })
}

/// Estrategia de ajuste de línea del editor según `[editor] line_wrap`.
#[must_use]
pub fn wrapping(line_wrap: bool) -> iced::widget::text::Wrapping {
    if line_wrap {
        iced::widget::text::Wrapping::Word
    } else {
        iced::widget::text::Wrapping::None
    }
}

/// Tamaño de fuente del editor en píxeles (la configuración ya lo acotó a 8..=48).
#[must_use]
pub fn font_px(size: u32) -> f32 {
    f32::from(u16::try_from(size).unwrap_or(14))
}

/// Tamaño de fuente efectivo: el de la configuración multiplicado por el zoom.
#[must_use]
pub fn zoomed_px(size: u32, zoom: f32) -> f32 {
    (font_px(size) * zoom).max(1.0)
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

    fn key_press(c: &str, modifiers: iced::keyboard::Modifiers, focused: bool) -> KeyPress {
        use iced::keyboard::key::{Code, Physical};
        KeyPress {
            key: Key::Character(c.into()),
            modified_key: Key::Character(c.into()),
            physical_key: Physical::Code(Code::KeyV),
            modifiers,
            text: Some(c.into()),
            status: if focused {
                Status::Focused { is_hovered: false }
            } else {
                Status::Active
            },
        }
    }

    #[test]
    fn command_v_in_the_focused_editor_requests_a_smart_paste() {
        // AC2
        let cmd = iced::keyboard::Modifiers::COMMAND;
        assert!(matches!(
            key_binding(key_press("v", cmd, true)),
            Some(Binding::Custom(Message::PasteRequested))
        ));
        // Sin enfocar el editor (p. ej. otro campo de texto) no se intercepta.
        assert!(!matches!(
            key_binding(key_press("v", cmd, false)),
            Some(Binding::Custom(Message::PasteRequested))
        ));
        // Una `v` normal se sigue escribiendo.
        assert!(matches!(
            key_binding(key_press("v", iced::keyboard::Modifiers::empty(), true)),
            Some(Binding::Insert('v'))
        ));
    }

    #[test]
    fn unknown_or_generic_font_families_fall_back_to_the_system_monospace() {
        assert_eq!(resolve_font("monospace"), iced::Font::MONOSPACE);
        assert_eq!(resolve_font("  MONOSPACE "), iced::Font::MONOSPACE);
        assert_eq!(resolve_font(""), iced::Font::MONOSPACE);
        assert_eq!(
            resolve_font("Fuente Que No Existe 12345"),
            iced::Font::MONOSPACE
        );
    }

    #[test]
    fn zoom_scales_the_configured_font_size() {
        assert!((zoomed_px(14, 1.0) - 14.0).abs() < f32::EPSILON);
        assert!((zoomed_px(14, 2.0) - 28.0).abs() < f32::EPSILON);
        assert!((zoomed_px(8, 0.5) - 4.0).abs() < f32::EPSILON);
        assert!(zoomed_px(8, 0.0) >= 1.0, "nunca un tamaño nulo");
    }

    #[test]
    fn wrap_and_size_mappings() {
        assert_eq!(wrapping(true), iced::widget::text::Wrapping::Word);
        assert_eq!(wrapping(false), iced::widget::text::Wrapping::None);
        assert!((font_px(18) - 18.0).abs() < f32::EPSILON);
        assert!(
            (font_px(u32::MAX) - 14.0).abs() < f32::EPSILON,
            "valor absurdo → 14"
        );
    }

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
