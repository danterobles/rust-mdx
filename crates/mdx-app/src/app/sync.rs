//! Scroll sincronizado editor → vista previa (US-3.3).

use iced::widget::scrollable;

use super::{App, Message, SyncScroll, Task, editor, preview_sync};

impl App {
    /// Mide dónde quedó cada bloque renderizado de la vista previa. El resultado llega como
    /// `PreviewMeasured` y desencadena la sincronización.
    pub(super) fn measure_preview(&self) -> Task<Message> {
        if self.sync == SyncScroll::Off || !self.preview_visible || self.preview.is_empty() {
            return Task::none();
        }
        let id = self.doc.id;
        iced::advanced::widget::operate(preview_sync::Measure::new(self.preview.len()))
            .map(move |layout| Message::PreviewMeasured(id, layout))
    }

    /// Desplaza la vista previa hasta el bloque que corresponde a la primera línea visible del
    /// editor.
    pub(super) fn sync_now(&self) -> Task<Message> {
        if self.sync == SyncScroll::Off || !self.preview_visible {
            return Task::none();
        }
        let (line, fraction) = self.view_top;
        let total_lines = self.content.line_count();
        let Some(y) = preview_sync::target_offset(
            &self.blocks,
            &self.preview_layout,
            total_lines,
            line,
            fraction,
        ) else {
            return Task::none();
        };
        iced::widget::operation::scroll_to(
            iced::widget::Id::new(preview_sync::SCROLL_ID),
            scrollable::AbsoluteOffset {
                x: None,
                y: Some(y),
            },
        )
    }

    /// Lee la primera fila visible del editor y sincroniza la vista previa con ella.
    pub(super) fn update_view_top(&mut self) -> Task<Message> {
        self.view_top = editor::top_visible_line(&mut self.content);
        self.sync_now()
    }
}
