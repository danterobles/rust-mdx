//! Scroll sincronizado editor → vista previa (US-3.3).
//!
//! 1. El editor no expone su scroll: la línea visible se obtiene en `editor::top_visible_line`.
//! 2. [`Measure`] recorre el árbol de widgets y mide dónde quedó cada bloque renderizado.
//! 3. [`target_offset`] traduce «línea de origen» → desplazamiento en píxeles de la vista previa.

use std::collections::HashMap;

use iced::advanced::widget::operation::{Outcome, Scrollable};
use iced::advanced::widget::{Id, Operation};
use iced::{Rectangle, Vector};
use mdx_core::outline::Block;

/// Identificador del `scrollable` de la vista previa.
pub const SCROLL_ID: &str = "preview-scroll";

/// Identificador del contenedor del bloque `index`.
#[must_use]
pub fn block_id(index: usize) -> Id {
    Id::from(format!("preview-block-{index}"))
}

/// Posiciones medidas del contenido renderizado de la vista previa.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// Desplazamiento vertical del inicio de cada bloque respecto al inicio del contenido.
    pub tops: Vec<f32>,
    /// Altura total del contenido.
    pub content_height: f32,
    /// Altura visible del `scrollable`.
    pub viewport_height: f32,
}

/// Recorre el árbol de widgets y mide los contenedores de bloque.
pub struct Measure {
    ids: HashMap<Id, usize>,
    scroll_id: Id,
    origin_y: Option<f32>,
    tops: Vec<Option<f32>>,
    content_height: f32,
    viewport_height: f32,
}

impl Measure {
    #[must_use]
    pub fn new(blocks: usize) -> Self {
        Self {
            ids: (0..blocks).map(|i| (block_id(i), i)).collect(),
            scroll_id: Id::from(SCROLL_ID),
            origin_y: None,
            tops: vec![None; blocks],
            content_height: 0.0,
            viewport_height: 0.0,
        }
    }
}

impl Operation<Layout> for Measure {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Layout>)) {
        operate(self);
    }

    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if let Some(index) = id.and_then(|id| self.ids.get(id)) {
            self.tops[*index] = Some(bounds.y);
        }
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
        if id == Some(&self.scroll_id) {
            self.origin_y = Some(bounds.y);
            self.content_height = content_bounds.height;
            self.viewport_height = bounds.height;
        }
    }

    fn finish(&self) -> Outcome<Layout> {
        let origin = self.origin_y.unwrap_or(0.0);
        let mut last = 0.0_f32;
        let tops = self
            .tops
            .iter()
            .map(|t| {
                // Un bloque que no se llegó a medir hereda la posición del anterior.
                last = t.map_or(last, |y| y - origin);
                last
            })
            .collect();
        Outcome::Some(Layout {
            tops,
            content_height: self.content_height,
            viewport_height: self.viewport_height,
        })
    }
}

/// Desplazamiento (px) de la vista previa que corresponde a la posición `(line, fraction)` del
/// editor: línea de origen (base 0) y fracción 0..1 recorrida dentro de esa línea.
///
/// Con la medición de bloques coincidente se ubica el bloque que contiene la línea y se
/// interpola dentro de él; si no coincide con los bloques de origen, se usa una proporción por
/// líneas. Devuelve `None` si aún no hay medidas.
// Los números de línea y de bloque caben de sobra en la mantisa de un `f32`.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn target_offset(
    blocks: &[Block],
    layout: &Layout,
    total_lines: usize,
    line: usize,
    fraction: f32,
) -> Option<f32> {
    let max_scroll = (layout.content_height - layout.viewport_height).max(0.0);
    if layout.content_height <= 0.0 {
        return None;
    }
    let position = line as f32 + fraction.clamp(0.0, 1.0);
    let y = if !blocks.is_empty() && blocks.len() == layout.tops.len() {
        // Primer bloque que termina después de `line`: contiene la línea o es el siguiente.
        let index = blocks
            .partition_point(|b| b.end_line <= line)
            .min(blocks.len() - 1);
        let block = &blocks[index];
        let top = layout.tops[index];
        let bottom = layout
            .tops
            .get(index + 1)
            .copied()
            .unwrap_or(layout.content_height);
        if line < block.start_line {
            // Línea en blanco entre bloques: se alinea con el inicio del siguiente.
            top
        } else {
            let span = (block.end_line - block.start_line).max(1) as f32;
            let t = ((position - block.start_line as f32) / span).clamp(0.0, 1.0);
            top + t * (bottom - top)
        }
    } else if total_lines > 0 {
        (position / total_lines as f32).clamp(0.0, 1.0) * max_scroll
    } else {
        return None;
    };
    Some(y.clamp(0.0, max_scroll))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(start: usize, end: usize) -> Block {
        Block {
            start_line: start,
            end_line: end,
        }
    }

    /// Tres bloques en las líneas 0, 2–4 y 6; alturas 100, 300 y 100 con 20 px de separación.
    fn fixture() -> (Vec<Block>, Layout) {
        (
            vec![block(0, 1), block(2, 5), block(6, 7)],
            Layout {
                tops: vec![0.0, 120.0, 440.0],
                content_height: 540.0,
                viewport_height: 200.0,
            },
        )
    }

    fn approx(a: Option<f32>, b: f32) {
        let a = a.expect("hay medidas");
        assert!((a - b).abs() < 0.01, "{a} != {b}");
    }

    #[test]
    fn aligns_with_the_block_start() {
        let (blocks, layout) = fixture();
        approx(target_offset(&blocks, &layout, 7, 0, 0.0), 0.0);
        approx(target_offset(&blocks, &layout, 7, 2, 0.0), 120.0);
        approx(target_offset(&blocks, &layout, 7, 6, 0.0), 340.0); // limitado al máximo de scroll
    }

    #[test]
    fn interpolates_inside_a_block() {
        let (blocks, layout) = fixture();
        // Línea 3 de un bloque de 3 líneas (2..5): 1/3 del alto hasta el siguiente bloque.
        approx(
            target_offset(&blocks, &layout, 7, 3, 0.0),
            120.0 + 320.0 / 3.0,
        );
        // La fracción dentro de la línea (texto ajustado en varias filas) también cuenta.
        approx(
            target_offset(&blocks, &layout, 7, 2, 0.5),
            120.0 + 320.0 / 6.0,
        );
    }

    #[test]
    fn blank_lines_between_blocks_align_with_the_next_block() {
        let (blocks, layout) = fixture();
        approx(target_offset(&blocks, &layout, 7, 1, 0.0), 120.0);
    }

    #[test]
    fn never_scrolls_past_the_end_or_before_the_start() {
        let (blocks, layout) = fixture();
        approx(target_offset(&blocks, &layout, 7, 99, 0.0), 340.0);
        approx(target_offset(&blocks, &layout, 7, 0, -3.0), 0.0);
    }

    #[test]
    fn falls_back_to_a_line_proportion_when_blocks_do_not_match() {
        let (_, layout) = fixture();
        // Mitad del documento → mitad del recorrido máximo (340 px).
        approx(target_offset(&[block(0, 5)], &layout, 10, 5, 0.0), 170.0);
        approx(target_offset(&[], &layout, 10, 10, 0.0), 340.0);
    }

    #[test]
    fn needs_measurements() {
        let (blocks, _) = fixture();
        assert_eq!(target_offset(&blocks, &Layout::default(), 7, 3, 0.0), None);
        let unscrollable = Layout {
            tops: vec![0.0],
            content_height: 100.0,
            viewport_height: 400.0,
        };
        assert_eq!(
            target_offset(&[block(0, 3)], &unscrollable, 3, 2, 0.0),
            Some(0.0)
        );
    }
}
