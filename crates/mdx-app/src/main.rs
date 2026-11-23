//! Punto de entrada de Mdx.

mod app;
mod editor;
mod explorer;
mod layout;
mod outline;
mod shortcuts;
mod state;
mod statusbar;
mod strings;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    tracing::info!(version = mdx_core::version(), "Mdx iniciado");

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
    .window_size((1200.0, 750.0))
    .run()?;
    Ok(())
}
