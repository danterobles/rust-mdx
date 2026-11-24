//! Virtualización de la vista previa: solo se construyen los bloques cercanos a lo visible.
//!
//! Cada bloque Markdown se dibuja con un widget de texto cuyo coste en memoria es proporcional
//! al texto. Con documentos largos (cientos de bloques) construirlos todos multiplica el uso de
//! memoria; aquí se calculan, con funciones puras, la ventana de bloques a dibujar y las alturas
//! (estimadas al principio y medidas después) de los que quedan fuera, que se sustituyen por
//! espaciadores de la misma altura.

use std::ops::Range;

use iced::widget::markdown::Item;
use mdx_core::outline::Block;

/// A partir de cuántos bloques se virtualiza. Por debajo se dibujan todos (comportamiento exacto
/// y sin estimaciones).
pub const MIN_BLOCKS: usize = 150;
/// Margen, en píxeles, que se dibuja por encima y por debajo de lo visible.
pub const OVERSCAN_PX: f32 = 800.0;
/// Tamaño de letra relativo de cada nivel de encabezado (estimación).
const HEADING_SCALE: [f32; 6] = [2.0, 1.5, 1.25, 1.0, 1.0, 1.0];

/// Altura estimada de cada bloque (incluye la separación que sigue al bloque), a partir de su
/// tipo y de las líneas de origen que ocupa. Las estimaciones se sustituyen por medidas reales a
/// medida que los bloques se dibujan.
#[must_use]
pub fn estimate_heights(items: &[Item], blocks: &[Block], text_px: f32, spacing: f32) -> Vec<f32> {
    let line = text_px * 1.3;
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            #[allow(clippy::cast_precision_loss)]
            let source_lines = blocks
                .get(i)
                .map_or(1, |b| b.end_line.saturating_sub(b.start_line).max(1))
                as f32;
            let body = match item {
                Item::Heading(level, _) => {
                    let index = usize::from(*level as u8).saturating_sub(1).min(5);
                    HEADING_SCALE[index] * text_px * 1.3
                }
                Item::Paragraph(_) => source_lines * line * 1.2,
                #[allow(clippy::cast_precision_loss)]
                Item::CodeBlock { lines, .. } => lines.len().max(1) as f32 * text_px * 1.2 + 24.0,
                Item::List { .. } => source_lines * line * 1.15,
                Item::Quote(_) => source_lines * line + 16.0,
                Item::Rule => 12.0,
                Item::Table { .. } => source_lines * (line + 10.0),
                Item::Image { .. } => 220.0,
            };
            body.max(line) + spacing
        })
        .collect()
}

/// Posición del inicio de cada bloque (`tops[i]` = suma de las alturas anteriores); tiene un
/// elemento más que `heights`: la altura total.
#[must_use]
pub fn prefix_tops(heights: &[f32]) -> Vec<f32> {
    let mut tops = Vec::with_capacity(heights.len() + 1);
    let mut acc = 0.0;
    tops.push(acc);
    for h in heights {
        acc += h;
        tops.push(acc);
    }
    tops
}

/// Bloques a dibujar para una vista que muestra `[scroll_y, scroll_y + viewport_h]`, más el
/// margen `overscan` por ambos lados. Siempre devuelve al menos un bloque si los hay.
#[must_use]
pub fn visible_range(tops: &[f32], scroll_y: f32, viewport_h: f32, overscan: f32) -> Range<usize> {
    let n = tops.len().saturating_sub(1);
    if n == 0 {
        return 0..0;
    }
    let from = (scroll_y - overscan).max(0.0);
    let to = scroll_y + viewport_h + overscan;
    // Primer bloque que termina después de `from`.
    let start = tops[1..]
        .partition_point(|&bottom| bottom <= from)
        .min(n - 1);
    // Primer bloque que empieza a partir de `to`.
    let end = tops[..n]
        .partition_point(|&top| top < to)
        .clamp(start + 1, n);
    start..end
}

/// Primer bloque cuyo final queda por debajo de `y` (el que ocupa el borde superior de la vista).
#[must_use]
pub fn block_at(tops: &[f32], y: f32) -> usize {
    let n = tops.len().saturating_sub(1);
    if n == 0 {
        return 0;
    }
    tops[1..].partition_point(|&bottom| bottom <= y).min(n - 1)
}

/// Sustituye por las alturas reales las de los bloques de `range` que se midieron.
///
/// `found[i]` es la posición medida del inicio del bloque `i` (para `i == range.end`, la del
/// espaciador inferior, que es el final del último bloque dibujado). Devuelve la variación total
/// de altura de los bloques que quedan **por encima** de `anchor_block` (el de la parte superior
/// de la vista), para compensar el desplazamiento y que el contenido no «salte».
pub fn apply_measure(
    heights: &mut [f32],
    found: &[Option<f32>],
    range: &Range<usize>,
    anchor_block: usize,
) -> f32 {
    let mut delta_above = 0.0;
    for i in range.clone() {
        let (Some(Some(top)), Some(Some(next))) = (found.get(i), found.get(i + 1)) else {
            continue;
        };
        let measured = next - top;
        if !measured.is_finite() || measured < 1.0 {
            continue;
        }
        if let Some(h) = heights.get_mut(i) {
            if i < anchor_block {
                delta_above += measured - *h;
            }
            *h = measured;
        }
    }
    delta_above
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::markdown;

    fn tops_for(n: usize, h: f32) -> Vec<f32> {
        prefix_tops(&vec![h; n])
    }

    #[test]
    fn prefix_tops_has_one_more_element_than_heights() {
        assert_eq!(prefix_tops(&[10.0, 20.0, 5.0]), [0.0, 10.0, 30.0, 35.0]);
        assert_eq!(prefix_tops(&[]), [0.0]);
    }

    #[test]
    fn the_visible_window_covers_the_viewport_plus_overscan() {
        // 1000 bloques de 100 px; vista de 500 px en y = 10 000.
        let tops = tops_for(1000, 100.0);
        let r = visible_range(&tops, 10_000.0, 500.0, 800.0);
        // Debe incluir [9200, 11300]: bloques 92..113.
        assert_eq!(r, 92..113);
        // Sin margen solo lo visible (+ el bloque parcial).
        assert_eq!(visible_range(&tops, 10_000.0, 500.0, 0.0), 100..105);
    }

    #[test]
    fn the_window_is_clamped_at_both_ends_and_never_empty() {
        let tops = tops_for(10, 100.0);
        assert_eq!(visible_range(&tops, 0.0, 300.0, 100.0), 0..4);
        assert_eq!(visible_range(&tops, 99_999.0, 300.0, 100.0), 9..10);
        // Un desplazamiento negativo (rebote) deja al menos el primer bloque.
        assert_eq!(visible_range(&tops, -500.0, 300.0, 0.0), 0..1);
        assert_eq!(visible_range(&[0.0], 0.0, 100.0, 10.0), 0..0, "sin bloques");
        let one = tops_for(1, 50.0);
        assert_eq!(visible_range(&one, 10.0, 100.0, 0.0), 0..1);
    }

    #[test]
    fn block_at_finds_the_block_on_the_top_edge() {
        let tops = prefix_tops(&[100.0, 50.0, 200.0]);
        assert_eq!(block_at(&tops, 0.0), 0);
        assert_eq!(block_at(&tops, 99.9), 0);
        assert_eq!(block_at(&tops, 100.0), 1);
        assert_eq!(block_at(&tops, 160.0), 2);
        assert_eq!(block_at(&tops, 9_999.0), 2, "se acota al último");
    }

    #[test]
    fn measuring_replaces_estimates_and_reports_the_change_above_the_anchor() {
        let mut heights = vec![100.0, 100.0, 100.0, 100.0];
        // Se dibujaron los bloques 1 y 2 (el espaciador inferior es el «bloque 3»):
        // el 1 mide 120, el 2 mide 80.
        let found = vec![None, Some(100.0), Some(220.0), Some(300.0), None];
        let delta = apply_measure(&mut heights, &found, &(1..3), 2);
        assert_eq!(heights, [100.0, 120.0, 80.0, 100.0]);
        assert!(
            (delta - 20.0).abs() < 1e-4,
            "solo el bloque 1 está sobre el ancla: {delta}"
        );
        // Con el ancla en el bloque 1 no hay variación por encima.
        let mut again = vec![100.0; 4];
        assert!(apply_measure(&mut again, &found, &(1..3), 1).abs() < 1e-4);
    }

    #[test]
    fn measuring_ignores_missing_or_absurd_positions() {
        let mut heights = vec![50.0, 50.0];
        let found = vec![Some(0.0), None, Some(10.0)];
        assert!(apply_measure(&mut heights, &found, &(0..2), 2).abs() < f32::EPSILON);
        assert_eq!(heights, [50.0, 50.0]);
        let flat = vec![Some(5.0), Some(5.0), Some(f32::NAN)];
        let _ = apply_measure(&mut heights, &flat, &(0..2), 2);
        assert_eq!(heights, [50.0, 50.0]);
    }

    #[test]
    fn estimates_grow_with_the_amount_of_text_and_cover_every_block_kind() {
        let md = "# Título\n\nUn párrafo.\n\nOtro párrafo\ncon varias\nlíneas de origen\npara el test.\n\n\
                  ```\na\nb\nc\nd\n```\n\n- uno\n- dos\n\n> cita\n\n---\n\n| a | b |\n|--|--|\n| 1 | 2 |\n\n\
                  ![img](x.png)\n";
        let items: Vec<_> = markdown::parse(md).collect();
        let blocks = mdx_core::outline::top_level_blocks(md);
        assert_eq!(
            items.len(),
            blocks.len(),
            "mismos bloques que la vista previa"
        );
        let h = estimate_heights(&items, &blocks, 16.0, 10.0);
        assert_eq!(h.len(), items.len());
        assert!(h.iter().all(|x| *x > 10.0 && x.is_finite()));
        assert!(h[2] > h[1], "más líneas de origen → más alto: {h:?}");
        assert!(h[0] > h[1], "un H1 es más alto que un párrafo corto");
    }

    #[test]
    fn estimates_tolerate_mismatched_blocks() {
        let items: Vec<_> = markdown::parse("a\n\nb\n").collect();
        let h = estimate_heights(&items, &[], 16.0, 10.0);
        assert_eq!(h.len(), 2);
        assert!(h.iter().all(|x| *x > 0.0));
    }
}
