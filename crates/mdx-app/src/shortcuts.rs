//! Mapeo tecla → acción. `Ctrl` en Windows/Linux, `Cmd` en macOS (vía `Modifiers::command`).

use iced::keyboard::key::Physical;
use iced::keyboard::{Key, Modifiers};

/// Acciones disparadas por atajos de teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    New,
    Open,
    Save,
    SaveAs,
}

/// Traduce una pulsación a una acción, si corresponde.
#[must_use]
pub fn map(key: &Key, physical: Physical, modifiers: Modifiers) -> Option<ShortcutAction> {
    if !modifiers.command() || modifiers.alt() {
        return None;
    }
    let letter = key.to_latin(physical)?;
    match (letter.to_ascii_lowercase(), modifiers.shift()) {
        ('n', false) => Some(ShortcutAction::New),
        ('o', false) => Some(ShortcutAction::Open),
        ('s', false) => Some(ShortcutAction::Save),
        ('s', true) => Some(ShortcutAction::SaveAs),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::key::Code;

    fn press(c: &str, code: Code, mods: Modifiers) -> Option<ShortcutAction> {
        map(&Key::Character(c.into()), Physical::Code(code), mods)
    }

    #[test]
    fn maps_file_shortcuts() {
        let cmd = Modifiers::COMMAND;
        assert_eq!(press("n", Code::KeyN, cmd), Some(ShortcutAction::New));
        assert_eq!(press("o", Code::KeyO, cmd), Some(ShortcutAction::Open));
        assert_eq!(press("s", Code::KeyS, cmd), Some(ShortcutAction::Save));
        assert_eq!(
            press("S", Code::KeyS, cmd | Modifiers::SHIFT),
            Some(ShortcutAction::SaveAs)
        );
    }

    #[test]
    fn ignores_plain_keys_and_other_combos() {
        assert_eq!(press("s", Code::KeyS, Modifiers::empty()), None);
        assert_eq!(press("s", Code::KeyS, Modifiers::SHIFT), None);
        assert_eq!(press("x", Code::KeyX, Modifiers::COMMAND), None);
        assert_eq!(
            press("s", Code::KeyS, Modifiers::COMMAND | Modifiers::ALT),
            None
        );
    }
}
