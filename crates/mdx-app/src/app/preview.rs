//! Vista previa en vivo y enlaces.

use super::{
    App, LinkTarget, Message, Path, Task, initial_panes, link, markdown, panes_without_preview,
    strings,
};

impl App {
    /// Parsea el Markdown en un hilo aparte para no bloquear la UI con documentos grandes
    /// (NFR-4). El resultado llega como `PreviewParsed` y se ignora si ya quedó obsoleto.
    pub(super) fn start_preview_parse(&mut self) -> Task<Message> {
        self.preview_gen += 1;
        self.preview_pending = false;
        let generation = self.preview_gen;
        let id = self.doc.id;
        let text = self.doc.text.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || markdown::parse(&text).collect::<Vec<_>>())
                    .await
                    .unwrap_or_default()
            },
            move |items| Message::PreviewParsed(id, generation, items),
        )
    }

    pub(super) fn toggle_preview(&mut self) -> Task<Message> {
        self.preview_visible = !self.preview_visible;
        if self.preview_visible {
            self.panes = initial_panes();
            self.start_preview_parse()
        } else {
            self.panes = panes_without_preview();
            Task::none()
        }
    }

    pub(super) fn handle_link(&mut self, url: &str) -> Task<Message> {
        let base = self.doc.path.as_deref().and_then(Path::parent);
        match link::classify(url, base) {
            LinkTarget::External(url) => {
                Task::perform(async move { open::that_detached(url) }, |result| {
                    Message::LinkOpened(result.map_err(|e| e.to_string()))
                })
            }
            LinkTarget::Markdown(path) => self.open_path(path, false),
            LinkTarget::Unsupported => {
                self.set_status(strings::STATUS_LINK_UNSUPPORTED.replace("{url}", url), true);
                Task::none()
            }
        }
    }
}
