//! Disposición de paneles (lógica pura, testeable sin ventana).

use iced::widget::pane_grid::{self, Axis, Configuration};

/// Paneles de la ventana principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Explorer,
    Editor,
    Preview,
}

/// Proporción del explorador respecto al ancho total (20 %).
pub const EXPLORER_RATIO: f32 = 0.2;
/// Proporción del editor dentro del espacio restante (40 % / 40 % del total).
pub const EDITOR_RATIO: f32 = 0.5;

/// Crea el estado inicial del `pane_grid`: Explorador 20 % · Editor 40 % · Preview 40 %.
#[must_use]
pub fn initial_panes() -> pane_grid::State<PaneKind> {
    pane_grid::State::with_configuration(Configuration::Split {
        axis: Axis::Vertical,
        ratio: EXPLORER_RATIO,
        a: Box::new(Configuration::Pane(PaneKind::Explorer)),
        b: Box::new(Configuration::Split {
            axis: Axis::Vertical,
            ratio: EDITOR_RATIO,
            a: Box::new(Configuration::Pane(PaneKind::Editor)),
            b: Box::new(Configuration::Pane(PaneKind::Preview)),
        }),
    })
}

/// Disposición con la vista previa oculta: Explorador 20 % · Editor 80 %.
#[must_use]
pub fn panes_without_preview() -> pane_grid::State<PaneKind> {
    pane_grid::State::with_configuration(Configuration::Split {
        axis: Axis::Vertical,
        ratio: EXPLORER_RATIO,
        a: Box::new(Configuration::Pane(PaneKind::Explorer)),
        b: Box::new(Configuration::Pane(PaneKind::Editor)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::pane_grid::Node;

    #[test]
    fn initial_layout_is_20_40_40() {
        let state = initial_panes();
        assert_eq!(state.len(), 3);
        let Node::Split {
            axis, ratio, a, b, ..
        } = state.layout()
        else {
            panic!("la raíz debe ser un split");
        };
        assert_eq!(*axis, Axis::Vertical);
        assert!((ratio - 0.2).abs() < f32::EPSILON);
        let Node::Pane(first) = **a else {
            panic!("izquierda debe ser un panel")
        };
        assert_eq!(state.get(first), Some(&PaneKind::Explorer));
        let Node::Split {
            ratio: inner,
            a: ea,
            b: pb,
            ..
        } = &**b
        else {
            panic!("derecha debe ser un split");
        };
        assert!((inner - 0.5).abs() < f32::EPSILON);
        let (Node::Pane(e), Node::Pane(p)) = (&**ea, &**pb) else {
            panic!("hojas esperadas")
        };
        assert_eq!(state.get(*e), Some(&PaneKind::Editor));
        assert_eq!(state.get(*p), Some(&PaneKind::Preview));
    }

    #[test]
    fn dividers_can_be_resized() {
        let mut state = initial_panes();
        let Node::Split { id, .. } = state.layout() else {
            panic!("split esperado")
        };
        let id = *id;
        state.resize(id, 0.3);
        let Node::Split { ratio, .. } = state.layout() else {
            panic!("split esperado")
        };
        assert!((ratio - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn layout_without_preview_has_two_panes() {
        let state = panes_without_preview();
        assert_eq!(state.len(), 2);
        assert!(state.iter().all(|(_, k)| *k != PaneKind::Preview));
    }
}
