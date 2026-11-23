//! Vista previa en vivo y enlaces.

use super::{
    App, LinkTarget, Message, PREVIEW_DEBOUNCE, Parsed, Path, SyncScroll, Task, initial_panes,
    link, markdown, panes_without_preview, strings,
};

impl App {
    /// Mensajes de la vista previa: debounce, resultados del parseo y scroll sincronizado.
    pub(super) fn update_preview(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PreviewTick(now) => {
                if self.preview_pending
                    && now.saturating_duration_since(self.preview_edit_at) >= PREVIEW_DEBOUNCE
                {
                    return self.start_preview_parse();
                }
                Task::none()
            }
            Message::PreviewParsed(id, generation, parsed) => {
                // Puede llegar para un documento que ya no es el activo.
                let _ = self.with_doc(id, |app| {
                    if generation == app.preview_gen {
                        // Con la vista previa oculta no se parsea (items vacíos): se conserva.
                        if app.preview_visible {
                            app.preview = parsed.items;
                        }
                        app.outline = parsed.outline;
                        app.blocks = parsed.blocks;
                    }
                });
                // Nuevo contenido renderizado: se mide de nuevo (solo la pestaña activa).
                if id == self.doc.id {
                    self.measure_preview()
                } else {
                    Task::none()
                }
            }
            Message::PreviewMeasured(id, layout) => {
                if id != self.doc.id {
                    return Task::none();
                }
                self.preview_layout = layout;
                self.sync_now()
            }
            Message::SetSyncScroll(enabled) => {
                self.sync = if enabled {
                    SyncScroll::On
                } else {
                    SyncScroll::Off
                };
                if enabled {
                    self.measure_preview()
                } else {
                    Task::none()
                }
            }
            _ => Task::none(),
        }
    }

    /// Parsea el Markdown en un hilo aparte para no bloquear la UI con documentos grandes
    /// (NFR-4). El resultado llega como `PreviewParsed` y se ignora si ya quedó obsoleto.
    pub(super) fn start_preview_parse(&mut self) -> Task<Message> {
        self.preview_gen += 1;
        self.preview_pending = false;
        let generation = self.preview_gen;
        let id = self.doc.id;
        let text = self.doc.text.clone();
        // El esquema se calcula siempre; los elementos de la vista previa solo si está visible.
        let want_items = self.preview_visible;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let items = if want_items {
                        markdown::parse(&text).collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    Parsed {
                        items,
                        outline: mdx_core::outline::extract(&text),
                        blocks: mdx_core::outline::top_level_blocks(&text),
                    }
                })
                .await
                .unwrap_or_else(|_| Parsed {
                    items: Vec::new(),
                    outline: Vec::new(),
                    blocks: Vec::new(),
                })
            },
            move |parsed| Message::PreviewParsed(id, generation, parsed),
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
