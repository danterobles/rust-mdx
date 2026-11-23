//! Panel «Esquema»: encabezados del documento activo, indentados por nivel.

use iced::widget::{Column, button, container, scrollable, text};
use iced::{Element, Length, Padding};
use mdx_core::outline::Outline;

use crate::app::Message;
use crate::strings;

const INDENT: f32 = 12.0;

/// Lista de encabezados; un clic lleva el cursor del editor a esa línea.
pub fn view(outline: &Outline) -> Element<'_, Message> {
    if outline.is_empty() {
        return container(text(strings::OUTLINE_EMPTY)).padding(4).into();
    }
    let mut column = Column::new().spacing(1);
    for heading in outline {
        let label = if heading.text.is_empty() {
            strings::OUTLINE_BLANK_HEADING
        } else {
            heading.text.as_str()
        };
        // Los niveles más altos (H1) algo más grandes; el resto, tamaño normal.
        let size = if heading.level == 1 { 15 } else { 14 };
        let indent = f32::from(heading.level.saturating_sub(1)) * INDENT;
        column = column.push(
            container(
                button(text(label).size(size))
                    .padding(0)
                    .style(button::text)
                    .on_press(Message::GoToLine(heading.line)),
            )
            .padding(Padding::ZERO.left(indent)),
        );
    }
    scrollable(column)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
