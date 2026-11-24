//! Punto de entrada de Mdx.

// Los tests comparan colecciones vacías con `assert!(..is_empty())`.
#![cfg_attr(test, allow(clippy::assert_is_empty))]

mod app;
mod clipboard;
mod editor;
mod explorer;
mod layout;
mod outline;
mod preview_sync;
mod search;
mod shortcuts;
mod state;
mod statusbar;
mod strings;
mod theme;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    tracing::info!(version = mdx_core::version(), "RustMDX iniciado");

    // `mdx <ruta>`: carpeta (workspace) o archivo.
    let arg = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    iced::application(
        move || app::App::new(arg.clone()),
        app::App::update,
        app::App::view,
    )
    .title(app::App::title)
    .theme(app::App::theme)
    .subscription(app::App::subscription)
    .exit_on_close_request(false)
    .window(iced::window::Settings {
        size: iced::Size::new(1200.0, 750.0),
        icon: window_icon(),
        ..iced::window::Settings::default()
    })
    .run()?;
    Ok(())
}

/// Ícono de la ventana (el de los instaladores está en `assets/icons/`).
fn window_icon() -> Option<iced::window::Icon> {
    const ICON: &[u8] = include_bytes!("../../../assets/icons/png/rustmdx-256.png");
    iced::window::icon::from_file_data(ICON, None).ok()
}
