//! Estado, mensajes, `update` y `view` de la aplicación (arquitectura Elm).

use std::path::{Path, PathBuf};

use iced::widget::{Column, container, pane_grid, text, text_editor};
use iced::{Element, Event, Length, Subscription, Task, Theme, event, keyboard};
use mdx_core::io::SavedInfo;
use mdx_core::{CoreError, Document};

use crate::layout::{PaneKind, initial_panes};
use crate::shortcuts::{self, ShortcutAction};
use crate::strings;

pub struct App {
    panes: pane_grid::State<PaneKind>,
    doc: Document,
    content: text_editor::Content,
    status: Option<Status>,
}

struct Status {
    text: String,
    is_error: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    PaneResized(pane_grid::ResizeEvent),
    Edit(text_editor::Action),
    Shortcut(ShortcutAction),
    FilePicked(Option<PathBuf>),
    DocumentLoaded(Result<Document, CoreError>),
    SavePicked(Option<PathBuf>),
    Saved(Result<SavedInfo, CoreError>, String),
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                panes: initial_panes(),
                doc: Document::untitled(),
                content: text_editor::Content::new(),
                status: None,
            },
            Task::none(),
        )
    }

    pub fn title(&self) -> String {
        format!(
            "{} — {}",
            self.doc.display_name(strings::UNTITLED),
            strings::APP_TITLE
        )
    }

    #[allow(clippy::unused_self)]
    pub fn theme(&self) -> Theme {
        Theme::Light
    }

    #[allow(clippy::unused_self)]
    pub fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, _status, _window| match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) => shortcuts::map(&key, physical_key, modifiers).map(Message::Shortcut),
            _ => None,
        })
    }

    // iced exige `Message` por valor; crecerá con las próximas historias.
    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
                Task::none()
            }
            Message::Edit(action) => {
                self.content.perform(action);
                Task::none()
            }
            Message::Shortcut(action) => self.handle_shortcut(action),
            Message::FilePicked(Some(path)) => Task::perform(
                async move { mdx_core::io::read_document(&path).await },
                Message::DocumentLoaded,
            ),
            Message::FilePicked(None) | Message::SavePicked(None) => Task::none(),
            Message::DocumentLoaded(result) => {
                self.handle_loaded(result);
                Task::none()
            }
            Message::SavePicked(Some(path)) => self.save_to(path),
            Message::Saved(result, text) => {
                self.handle_saved(result, text);
                Task::none()
            }
        }
    }

    fn handle_shortcut(&mut self, action: ShortcutAction) -> Task<Message> {
        match action {
            ShortcutAction::New => {
                self.doc = Document::untitled();
                self.content = text_editor::Content::new();
                self.set_status(strings::STATUS_NEW.to_owned(), false);
                Task::none()
            }
            ShortcutAction::Open => Task::perform(pick_file_to_open(), Message::FilePicked),
            ShortcutAction::Save => match self.doc.path.clone() {
                Some(path) => self.save_to(path),
                None => self.save_as(),
            },
            ShortcutAction::SaveAs => self.save_as(),
        }
    }

    fn save_as(&self) -> Task<Message> {
        let suggested = self.doc.display_name(&format!("{}.md", strings::UNTITLED));
        let start_dir = self
            .doc
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        Task::perform(pick_file_to_save(suggested, start_dir), Message::SavePicked)
    }

    fn save_to(&self, path: PathBuf) -> Task<Message> {
        let text = self.content.text();
        let (line_ending, has_bom) = (self.doc.line_ending, self.doc.has_bom);
        Task::perform(
            async move {
                let result = mdx_core::io::save_text(&path, &text, line_ending, has_bom).await;
                (result, text)
            },
            |(result, text)| Message::Saved(result, text),
        )
    }

    fn handle_loaded(&mut self, result: Result<Document, CoreError>) {
        match result {
            Ok(doc) => {
                self.content = text_editor::Content::with_text(&doc.text);
                let name = doc.display_name(strings::UNTITLED);
                self.doc = doc;
                self.set_status(format!("{}: {name}", strings::STATUS_OPENED), false);
            }
            // AC7: el archivo no se abre y el documento actual queda intacto.
            Err(err) => self.set_status(err.to_string(), true),
        }
    }

    fn handle_saved(&mut self, result: Result<SavedInfo, CoreError>, text: String) {
        match result {
            Ok(info) => {
                self.doc.path = Some(info.path);
                self.doc.text = text;
                self.doc.saved_hash = Some(info.saved_hash);
                self.doc.disk_mtime = info.mtime;
                let name = self.doc.display_name(strings::UNTITLED);
                self.set_status(format!("{}: {name}", strings::STATUS_SAVED), false);
            }
            Err(err) => self.set_status(err.to_string(), true),
        }
    }

    fn set_status(&mut self, text: String, is_error: bool) {
        self.status = Some(Status { text, is_error });
    }

    pub fn view(&self) -> Element<'_, Message> {
        let grid = pane_grid(&self.panes, |_, kind, _| {
            let body: Element<'_, Message> = match kind {
                PaneKind::Explorer => text(strings::PANE_EXPLORER).into(),
                PaneKind::Editor => text_editor(&self.content)
                    .on_action(Message::Edit)
                    .height(Length::Fill)
                    .into(),
                PaneKind::Preview => text(strings::PANE_PREVIEW).into(),
            };
            pane_grid::Content::new(
                container(body)
                    .padding(8)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
        })
        .spacing(4)
        .on_resize(6, Message::PaneResized);

        let status: Element<'_, Message> = match &self.status {
            Some(s) if s.is_error => text(&s.text).style(text::danger).into(),
            Some(s) => text(&s.text).into(),
            None => text("").into(),
        };
        let status_bar = container(status)
            .padding([4, 8])
            .width(Length::Fill)
            .style(container::bordered_box);

        Column::new().push(grid).push(status_bar).into()
    }
}

async fn pick_file_to_open() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_OPEN_TITLE)
        .add_filter(strings::FILTER_MARKDOWN, &["md", "markdown", "txt"])
        .add_filter(strings::FILTER_ALL, &["*"])
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn pick_file_to_save(suggested: String, dir: Option<PathBuf>) -> Option<PathBuf> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_SAVE_TITLE)
        .set_file_name(suggested)
        .add_filter(strings::FILTER_MARKDOWN, &["md", "markdown", "txt"]);
    if let Some(dir) = dir {
        dialog = dialog.set_directory(dir);
    }
    dialog.save_file().await.map(|h| h.path().to_path_buf())
}
