//! Estado, mensajes, `update` y `view` de la aplicación (arquitectura Elm).

use iced::widget::{Column, container, pane_grid, text};
use iced::{Element, Length, Task, Theme};

use crate::layout::{PaneKind, initial_panes};
use crate::strings;

pub struct App {
    panes: pane_grid::State<PaneKind>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PaneResized(pane_grid::ResizeEvent),
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                panes: initial_panes(),
            },
            Task::none(),
        )
    }

    #[allow(clippy::unused_self)]
    pub fn title(&self) -> String {
        strings::APP_TITLE.to_owned()
    }

    #[allow(clippy::unused_self)]
    pub fn theme(&self) -> Theme {
        Theme::Light
    }

    // iced exige `Message` por valor; crecerá con las próximas historias.
    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PaneResized(event) => self.panes.resize(event.split, event.ratio),
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let grid = pane_grid(&self.panes, |_, kind, _| {
            let label = match kind {
                PaneKind::Explorer => strings::PANE_EXPLORER,
                PaneKind::Editor => strings::PANE_EDITOR,
                PaneKind::Preview => strings::PANE_PREVIEW,
            };
            pane_grid::Content::new(
                container(text(label))
                    .padding(8)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
        })
        .spacing(4)
        .on_resize(6, Message::PaneResized);

        let status_bar = container(text(""))
            .padding([4, 8])
            .width(Length::Fill)
            .style(container::bordered_box);

        Column::new().push(grid).push(status_bar).into()
    }
}
