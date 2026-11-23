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
                        app.unavailable_images = parsed.unavailable_images;
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
            Message::ExportHtml => self.export_html(),
            Message::HtmlExportPicked(Some((path, text, title))) => Task::perform(
                async move {
                    mdx_core::export::export_to_file(path.clone(), text, title)
                        .await
                        .map(|()| path)
                },
                Message::HtmlExported,
            ),
            Message::HtmlExported(result) => {
                match result {
                    Ok(path) => self.set_status(
                        format!("{}: {}", strings::STATUS_EXPORTED, path.display()),
                        false,
                    ),
                    Err(err) => self.set_status(err.to_string(), true),
                }
                Task::none()
            }
            m @ (Message::PasteRequested
            | Message::ClipboardImage(_)
            | Message::PasteText(_)
            | Message::ImageSaved(..)) => self.update_paste(m),
            _ => Task::none(),
        }
    }

    /// Pide dónde guardar y exporta el texto **actual** del documento activo (con sus cambios
    /// sin guardar) a un `.html` autocontenido.
    pub(super) fn export_html(&self) -> Task<Message> {
        let title = self.doc.display_name(strings::UNTITLED);
        let stem = self
            .doc
            .path
            .as_deref()
            .and_then(Path::file_stem)
            .map_or_else(
                || strings::UNTITLED.to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
        let dir = self
            .doc
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let text = self.content.text();
        Task::perform(
            async move {
                super::editing::pick_html_save(format!("{stem}.html"), dir)
                    .await
                    .map(|path| (path, text, title))
            },
            Message::HtmlExportPicked,
        )
    }

    /// Parsea el Markdown en un hilo aparte para no bloquear la UI con documentos grandes
    /// (NFR-4). El resultado llega como `PreviewParsed` y se ignora si ya quedó obsoleto.
    pub(super) fn start_preview_parse(&mut self) -> Task<Message> {
        self.preview_gen += 1;
        self.preview_pending = false;
        let generation = self.preview_gen;
        let id = self.doc.id;
        let text = self.doc.text.clone();
        let base = self
            .doc
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
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
                    let unavailable_images = unavailable_images(&items, base.as_deref());
                    Parsed {
                        items,
                        outline: mdx_core::outline::extract(&text),
                        blocks: mdx_core::outline::top_level_blocks(&text),
                        unavailable_images,
                    }
                })
                .await
                .unwrap_or_else(|_| Parsed {
                    items: Vec::new(),
                    outline: Vec::new(),
                    blocks: Vec::new(),
                    unavailable_images: std::collections::HashSet::new(),
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

/// URLs de imagen de la vista previa que no se pueden dibujar: remotas, sin ruta resoluble o
/// cuyo archivo no existe. Comprueba el disco, por eso corre en el hilo de parseo.
fn unavailable_images(
    items: &[markdown::Item],
    base: Option<&Path>,
) -> std::collections::HashSet<String> {
    items
        .iter()
        .filter_map(|item| match item {
            markdown::Item::Image { url, .. } => Some(url),
            _ => None,
        })
        .filter(|url| !mdx_core::assets::resolve_image(url, base).is_some_and(|p| p.is_file()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(text: &str) -> Vec<markdown::Item> {
        markdown::parse(text).collect()
    }

    #[test]
    fn only_missing_remote_or_unresolvable_images_are_unavailable() {
        // AC1
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();
        std::fs::write(dir.path().join("assets/ok.png"), "x").unwrap();
        let md = "![a](assets/ok.png)\n\n![b](assets/falta.png)\n\n![c](https://x.org/r.png)\n\n![d](nota.txt)\n";
        let unavailable = unavailable_images(&items(md), Some(dir.path()));
        let mut list: Vec<_> = unavailable.iter().map(String::as_str).collect();
        list.sort_unstable();
        assert_eq!(
            list,
            ["assets/falta.png", "https://x.org/r.png", "nota.txt"]
        );
        // Sin directorio base (documento sin guardar) ninguna ruta relativa se puede resolver.
        assert_eq!(
            unavailable_images(&items("![a](assets/ok.png)"), None).len(),
            1
        );
    }
}
