//! Mapeo tecla → acción. `Ctrl` en Windows/Linux, `Cmd` en macOS (vía `Modifiers::command`).

use iced::keyboard::key::{Code, Named, Physical};
use iced::keyboard::{Key, Modifiers};

/// Acciones disparadas por atajos de teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    New,
    Open,
    OpenFolder,
    Save,
    SaveAs,
    Undo,
    Redo,
    TogglePreview,
    CloseTab,
    /// Exportar el documento a un archivo HTML.
    ExportHtml,
    /// Aumentar, reducir o restablecer el zoom del editor y la vista previa.
    ZoomIn,
    ZoomOut,
    ZoomReset,
    /// Abrir `config.toml` en una pestaña.
    OpenConfig,
    /// Buscar en el documento.
    Find,
    /// Buscar y reemplazar en el documento.
    Replace,
    /// Buscar en toda la carpeta.
    FindInWorkspace,
    NextTab,
    PrevTab,
    /// Salta a la pestaña `n` (1..=9).
    GoToTab(usize),
}

/// Dígito 1..=9 de la tecla (fila numérica o teclado numérico).
fn digit(key: &Key, physical: Physical) -> Option<usize> {
    if let Key::Character(c) = key
        && let Some(d) = c.chars().next().and_then(|c| c.to_digit(10))
        && c.chars().count() == 1
        && (1..=9).contains(&d)
    {
        return usize::try_from(d).ok();
    }
    let Physical::Code(code) = physical else {
        return None;
    };
    let n = match code {
        Code::Digit1 | Code::Numpad1 => 1,
        Code::Digit2 | Code::Numpad2 => 2,
        Code::Digit3 | Code::Numpad3 => 3,
        Code::Digit4 | Code::Numpad4 => 4,
        Code::Digit5 | Code::Numpad5 => 5,
        Code::Digit6 | Code::Numpad6 => 6,
        Code::Digit7 | Code::Numpad7 => 7,
        Code::Digit8 | Code::Numpad8 => 8,
        Code::Digit9 | Code::Numpad9 => 9,
        _ => return None,
    };
    Some(n)
}

/// Traduce una pulsación a una acción, si corresponde.
#[must_use]
pub fn map(key: &Key, physical: Physical, modifiers: Modifiers) -> Option<ShortcutAction> {
    // Ctrl/Cmd+\ se detecta por tecla física: en muchos teclados la barra invertida requiere Alt.
    if modifiers.command()
        && !modifiers.shift()
        && matches!(
            physical,
            Physical::Code(iced::keyboard::key::Code::Backslash)
        )
    {
        return Some(ShortcutAction::TogglePreview);
    }
    // Ctrl+Tab / Ctrl+Shift+Tab (también en macOS, donde es Control y no Cmd).
    if modifiers.control() && !modifiers.alt() && matches!(key, Key::Named(Named::Tab)) {
        return Some(if modifiers.shift() {
            ShortcutAction::PrevTab
        } else {
            ShortcutAction::NextTab
        });
    }
    // En macOS Cmd+H es «ocultar la aplicación»: reemplazar es Cmd+Opt+F (PRD §10).
    if cfg!(target_os = "macos")
        && modifiers.command()
        && modifiers.alt()
        && !modifiers.shift()
        // Con Opt, el carácter producido cambia (ƒ): se usa la tecla física.
        && matches!(physical, Physical::Code(Code::KeyF))
    {
        return Some(ShortcutAction::Replace);
    }
    if !modifiers.command() || modifiers.alt() {
        return None;
    }
    if !modifiers.shift()
        && let Some(n) = digit(key, physical)
    {
        return Some(ShortcutAction::GoToTab(n));
    }
    let letter = key.to_latin(physical)?;
    match (letter.to_ascii_lowercase(), modifiers.shift()) {
        ('n', false) => Some(ShortcutAction::New),
        ('o', false) => Some(ShortcutAction::Open),
        ('o', true) => Some(ShortcutAction::OpenFolder),
        ('s', false) => Some(ShortcutAction::Save),
        ('s', true) => Some(ShortcutAction::SaveAs),
        ('w', false) => Some(ShortcutAction::CloseTab),
        (',', false) => Some(ShortcutAction::OpenConfig),
        ('e', true) => Some(ShortcutAction::ExportHtml),
        // `+` suele exigir Shift; `=` es la misma tecla sin él.
        ('+' | '=', _) => Some(ShortcutAction::ZoomIn),
        ('-' | '_', _) => Some(ShortcutAction::ZoomOut),
        ('0', false) => Some(ShortcutAction::ZoomReset),
        ('f', false) => Some(ShortcutAction::Find),
        ('f', true) => Some(ShortcutAction::FindInWorkspace),
        ('h', false) if !cfg!(target_os = "macos") => Some(ShortcutAction::Replace),
        ('z', false) => Some(ShortcutAction::Undo),
        ('z', true) => Some(ShortcutAction::Redo),
        // Ctrl+Y rehace en Windows/Linux (en macOS es Cmd+Shift+Z).
        ('y', false) if !cfg!(target_os = "macos") => Some(ShortcutAction::Redo),
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
        assert_eq!(
            press("O", Code::KeyO, cmd | Modifiers::SHIFT),
            Some(ShortcutAction::OpenFolder)
        );
        assert_eq!(press("s", Code::KeyS, cmd), Some(ShortcutAction::Save));
        assert_eq!(
            press("S", Code::KeyS, cmd | Modifiers::SHIFT),
            Some(ShortcutAction::SaveAs)
        );
    }

    #[test]
    fn maps_undo_redo() {
        let cmd = Modifiers::COMMAND;
        assert_eq!(press("z", Code::KeyZ, cmd), Some(ShortcutAction::Undo));
        assert_eq!(
            press("Z", Code::KeyZ, cmd | Modifiers::SHIFT),
            Some(ShortcutAction::Redo)
        );
        let ctrl_y = press("y", Code::KeyY, cmd);
        assert_eq!(ctrl_y.is_some(), !cfg!(target_os = "macos"));
    }

    #[test]
    fn maps_toggle_preview_by_physical_key() {
        let cmd = Modifiers::COMMAND;
        assert_eq!(
            press("\\", Code::Backslash, cmd),
            Some(ShortcutAction::TogglePreview)
        );
        // En teclados donde `\` exige Alt, la tecla física sigue funcionando.
        assert_eq!(
            press("º", Code::Backslash, cmd | Modifiers::ALT),
            Some(ShortcutAction::TogglePreview)
        );
        assert_eq!(press("\\", Code::Backslash, Modifiers::empty()), None);
    }

    #[test]
    fn maps_tab_shortcuts() {
        let cmd = Modifiers::COMMAND;
        assert_eq!(press("w", Code::KeyW, cmd), Some(ShortcutAction::CloseTab));
        assert_eq!(
            press("1", Code::Digit1, cmd),
            Some(ShortcutAction::GoToTab(1))
        );
        assert_eq!(
            press("9", Code::Digit9, cmd),
            Some(ShortcutAction::GoToTab(9))
        );
        // El 0 no es una pestaña: es «restablecer zoom».
        assert_eq!(
            press("0", Code::Digit0, cmd),
            Some(ShortcutAction::ZoomReset)
        );
        // Teclado donde la fila numérica exige Shift: la tecla física sigue valiendo.
        assert_eq!(
            press("&", Code::Digit1, cmd),
            Some(ShortcutAction::GoToTab(1))
        );
        let tab = Key::Named(Named::Tab);
        let ctrl = Modifiers::CTRL;
        assert_eq!(
            map(&tab, Physical::Code(Code::Tab), ctrl),
            Some(ShortcutAction::NextTab)
        );
        assert_eq!(
            map(&tab, Physical::Code(Code::Tab), ctrl | Modifiers::SHIFT),
            Some(ShortcutAction::PrevTab)
        );
        assert_eq!(
            map(&tab, Physical::Code(Code::Tab), Modifiers::empty()),
            None
        );
    }

    #[test]
    fn maps_the_export_shortcut() {
        // AC1
        let cmd = Modifiers::COMMAND;
        assert_eq!(
            press("E", Code::KeyE, cmd | Modifiers::SHIFT),
            Some(ShortcutAction::ExportHtml)
        );
        assert_eq!(press("e", Code::KeyE, cmd), None);
    }

    #[test]
    fn maps_zoom_shortcuts() {
        // AC1
        let cmd = Modifiers::COMMAND;
        assert_eq!(press("=", Code::Equal, cmd), Some(ShortcutAction::ZoomIn));
        assert_eq!(
            press("+", Code::Equal, cmd | Modifiers::SHIFT),
            Some(ShortcutAction::ZoomIn)
        );
        assert_eq!(
            press("+", Code::NumpadAdd, cmd),
            Some(ShortcutAction::ZoomIn)
        );
        assert_eq!(press("-", Code::Minus, cmd), Some(ShortcutAction::ZoomOut));
        assert_eq!(
            press("-", Code::NumpadSubtract, cmd),
            Some(ShortcutAction::ZoomOut)
        );
        assert_eq!(
            press("0", Code::Digit0, cmd),
            Some(ShortcutAction::ZoomReset)
        );
        assert_eq!(press("-", Code::Minus, Modifiers::empty()), None);
        // Cmd+1..9 sigue saltando de pestaña.
        assert_eq!(
            press("1", Code::Digit1, cmd),
            Some(ShortcutAction::GoToTab(1))
        );
    }

    #[test]
    fn maps_the_config_shortcut() {
        // AC3
        let cmd = Modifiers::COMMAND;
        assert_eq!(
            press(",", Code::Comma, cmd),
            Some(ShortcutAction::OpenConfig)
        );
        assert_eq!(press(",", Code::Comma, Modifiers::empty()), None);
        assert_eq!(press(",", Code::Comma, cmd | Modifiers::SHIFT), None);
    }

    #[test]
    fn maps_search_shortcuts() {
        // AC1 / AC3
        let cmd = Modifiers::COMMAND;
        assert_eq!(press("f", Code::KeyF, cmd), Some(ShortcutAction::Find));
        assert_eq!(
            press("F", Code::KeyF, cmd | Modifiers::SHIFT),
            Some(ShortcutAction::FindInWorkspace)
        );
        let replace = press("h", Code::KeyH, cmd);
        let mac_replace = press("ƒ", Code::KeyF, cmd | Modifiers::ALT);
        if cfg!(target_os = "macos") {
            assert_eq!(replace, None, "Cmd+H es de macOS");
            assert_eq!(mac_replace, Some(ShortcutAction::Replace));
        } else {
            assert_eq!(replace, Some(ShortcutAction::Replace));
            assert_eq!(mac_replace, None);
        }
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
