//! Vista previa virtualizada (NFR-3): con documentos largos solo se dibujan los bloques cercanos
//! a lo visible; el resto son espaciadores con la altura estimada o medida.

use std::ops::Range;

use iced::widget::scrollable;

use super::{App, Layout, Message, PREVIEW_BASE_TEXT_PX, Task, preview_sync};
use crate::preview_virtual::{self as pv, MIN_BLOCKS, OVERSCAN_PX};

/// Altura que se supone a la vista hasta que el `scrollable` informa de la real.
const DEFAULT_VIEWPORT_H: f32 = 900.0;
/// Máximo de medidas encadenadas sin que el usuario se desplace (evita bucles).
const MAX_ROUNDS: u8 = 6;

/// Estado de la virtualización de la vista previa del documento activo.
#[derive(Debug, Default)]
pub struct VirtualPreview {
    /// Altura (estimada o medida) de cada bloque, con su separación incluida.
    pub heights: Vec<f32>,
    /// Bloques que se están dibujando.
    pub range: Range<usize>,
    pub scroll_y: f32,
    pub viewport_h: f32,
    rounds: u8,
}

impl VirtualPreview {
    fn tops(&self) -> Vec<f32> {
        pv::prefix_tops(&self.heights)
    }

    fn viewport_h(&self) -> f32 {
        if self.viewport_h > 1.0 {
            self.viewport_h
        } else {
            DEFAULT_VIEWPORT_H
        }
    }

    fn window(&self) -> Range<usize> {
        pv::visible_range(&self.tops(), self.scroll_y, self.viewport_h(), OVERSCAN_PX)
    }

    /// Alturas de espaciador por encima y por debajo de la ventana de bloques.
    #[must_use]
    pub fn spacers(&self) -> (f32, f32) {
        let tops = self.tops();
        let total = tops.last().copied().unwrap_or(0.0);
        let before = tops.get(self.range.start).copied().unwrap_or(0.0);
        let after = total - tops.get(self.range.end).copied().unwrap_or(total);
        (before.max(0.0), after.max(0.0))
    }
}

impl App {
    /// `true` si la vista previa se dibuja por ventanas (documento largo y vista previa visible).
    pub(super) fn is_virtual(&self) -> bool {
        self.preview_visible && self.preview.len() > MIN_BLOCKS
    }

    fn preview_text_px(&self) -> f32 {
        PREVIEW_BASE_TEXT_PX * self.zoom
    }

    /// Separación entre bloques que usa el visor de Markdown.
    pub(super) fn preview_spacing(&self) -> f32 {
        iced::widget::markdown::Settings::with_text_size(
            self.preview_text_px(),
            iced::widget::markdown::Style::from(&iced::Theme::Light),
        )
        .spacing
        .0
    }

    /// Recalcula alturas y ventana (tras un parseo, cambio de pestaña o de zoom).
    /// Devuelve la tarea que lleva la vista al principio si el documento es nuevo.
    pub(super) fn reset_virtual(&mut self, to_top: bool) -> Task<Message> {
        if !self.is_virtual() {
            self.vp = VirtualPreview::default();
            return Task::none();
        }
        if to_top {
            self.vp.scroll_y = 0.0;
        }
        // Al editar sin cambiar el número de bloques se conservan las alturas ya medidas (así la
        // vista previa no se mueve); en cualquier otro caso se vuelve a estimar.
        if to_top || self.vp.heights.len() != self.preview.len() {
            self.vp.heights = pv::estimate_heights(
                &self.preview,
                &self.blocks,
                self.preview_text_px(),
                self.preview_spacing(),
            );
        }
        self.vp.rounds = 0;
        self.vp.range = self.vp.window();
        self.sync_layout_from_heights();
        if to_top {
            iced::widget::operation::scroll_to(
                iced::widget::Id::new(preview_sync::SCROLL_ID),
                scrollable::AbsoluteOffset {
                    x: None,
                    y: Some(0.0),
                },
            )
        } else {
            Task::none()
        }
    }

    /// El `Layout` que usa el scroll sincronizado, derivado de las alturas conocidas.
    fn sync_layout_from_heights(&mut self) {
        let tops = self.vp.tops();
        let total = tops.last().copied().unwrap_or(0.0);
        self.preview_layout = Layout {
            tops: tops[..self.vp.heights.len()].to_vec(),
            content_height: total,
            viewport_height: self.vp.viewport_h(),
            found: Vec::new(),
        };
    }

    /// El usuario (o una sincronización) desplazó la vista previa.
    pub(super) fn on_preview_scrolled(&mut self, scroll_y: f32, viewport_h: f32) -> Task<Message> {
        if !self.is_virtual() {
            return Task::none();
        }
        self.vp.scroll_y = scroll_y.max(0.0);
        self.vp.viewport_h = viewport_h;
        self.vp.rounds = 0;
        self.update_window()
    }

    /// Recalcula la ventana; si cambió, hay que medir los bloques recién dibujados.
    fn update_window(&mut self) -> Task<Message> {
        let range = self.vp.window();
        if range == self.vp.range {
            return Task::none();
        }
        self.vp.range = range;
        self.measure_preview()
    }

    /// Mide los bloques de la ventana actual y la altura de lo que queda fuera.
    pub(super) fn measure_virtual(&self) -> Task<Message> {
        let id = self.doc.id;
        iced::advanced::widget::operate(preview_sync::Measure::for_range(
            self.preview.len(),
            self.vp.range.clone(),
        ))
        .map(move |layout| Message::PreviewMeasured(id, layout))
    }

    /// Resultado de medir en modo virtual: sustituye estimaciones por alturas reales, compensa el
    /// desplazamiento y, si la ventana cambió, vuelve a medir.
    pub(super) fn on_measured_virtual(&mut self, layout: &Layout) -> Task<Message> {
        if layout.viewport_height > 1.0 {
            self.vp.viewport_h = layout.viewport_height;
        }
        let tops_before = self.vp.tops();
        let anchor = pv::block_at(&tops_before, self.vp.scroll_y);
        let range = self.vp.range.clone();
        let delta = pv::apply_measure(&mut self.vp.heights, &layout.found, &range, anchor);
        self.sync_layout_from_heights();
        let mut tasks = Vec::new();
        if delta.abs() > 0.5 {
            // Los bloques sobre la vista cambiaron de altura: se corrige para que no «salte».
            self.vp.scroll_y = (self.vp.scroll_y + delta).max(0.0);
            tasks.push(iced::widget::operation::scroll_by(
                iced::widget::Id::new(preview_sync::SCROLL_ID),
                scrollable::AbsoluteOffset { x: 0.0, y: delta },
            ));
        }
        let new_range = self.vp.window();
        if new_range != self.vp.range && self.vp.rounds < MAX_ROUNDS {
            self.vp.range = new_range;
            self.vp.rounds += 1;
            tasks.push(self.measure_preview());
        } else {
            tasks.push(self.sync_now());
        }
        Task::batch(tasks)
    }

    /// Antes de desplazarse a `y` por la sincronización: la ventana debe cubrir ese destino.
    pub(super) fn virtual_jump_to(&mut self, y: f32) -> Task<Message> {
        self.vp.scroll_y = y.max(0.0);
        self.vp.rounds = 0;
        self.update_window()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacers_account_for_everything_outside_the_window() {
        let vp = VirtualPreview {
            heights: vec![100.0; 10],
            range: 3..6,
            ..VirtualPreview::default()
        };
        let (before, after) = vp.spacers();
        assert!(
            (before - 300.0).abs() < 1e-4 && (after - 400.0).abs() < 1e-4,
            "{before} {after}"
        );
        let all = VirtualPreview {
            heights: vec![100.0; 4],
            range: 0..4,
            ..VirtualPreview::default()
        };
        assert_eq!(all.spacers(), (0.0, 0.0));
    }
}
