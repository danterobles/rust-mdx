//! Tema de la interfaz (US-4.1): claro, oscuro o el del sistema.

use iced::Theme;
use iced::highlighter;
use iced::theme::Mode;
use mdx_core::config::ThemeSetting;

/// Tema de iced según el ajuste del usuario y el modo del sistema operativo.
///
/// `System` sigue al SO; si el SO no informa de su modo (`Mode::None`), se usa el claro.
#[must_use]
pub fn resolve(setting: ThemeSetting, system: Mode) -> Theme {
    match setting {
        ThemeSetting::Light => Theme::Light,
        ThemeSetting::Dark => Theme::Dark,
        ThemeSetting::System => match system {
            Mode::Dark => Theme::Dark,
            Mode::Light | Mode::None => Theme::Light,
        },
    }
}

/// Tema de resaltado de sintaxis del editor acorde con el tema de la interfaz.
#[must_use]
pub fn highlight_theme(theme: &Theme) -> highlighter::Theme {
    if theme.extended_palette().is_dark {
        highlighter::Theme::Base16Mocha
    } else {
        highlighter::Theme::InspiredGitHub
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_light_and_dark_ignore_the_system() {
        for mode in [Mode::None, Mode::Light, Mode::Dark] {
            assert_eq!(resolve(ThemeSetting::Light, mode), Theme::Light);
            assert_eq!(resolve(ThemeSetting::Dark, mode), Theme::Dark);
        }
    }

    #[test]
    fn system_follows_the_os_and_defaults_to_light_when_unknown() {
        assert_eq!(resolve(ThemeSetting::System, Mode::Dark), Theme::Dark);
        assert_eq!(resolve(ThemeSetting::System, Mode::Light), Theme::Light);
        assert_eq!(resolve(ThemeSetting::System, Mode::None), Theme::Light);
    }

    #[test]
    fn syntax_highlighting_matches_the_interface_brightness() {
        assert_eq!(
            highlight_theme(&Theme::Light),
            highlighter::Theme::InspiredGitHub
        );
        assert_eq!(
            highlight_theme(&Theme::Dark),
            highlighter::Theme::Base16Mocha
        );
    }
}
