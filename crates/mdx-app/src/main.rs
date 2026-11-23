//! Punto de entrada de Mdx. La UI (iced) llega en US-0.2.

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    tracing::info!(version = mdx_core::version(), "Mdx iniciado");
}
