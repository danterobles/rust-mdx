//! Punto de entrada de Mdx.

mod app;
mod layout;
mod strings;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    tracing::info!(version = mdx_core::version(), "Mdx iniciado");

    iced::application(app::App::new, app::App::update, app::App::view)
        .title(app::App::title)
        .theme(app::App::theme)
        .window_size((1200.0, 750.0))
        .run()?;
    Ok(())
}
