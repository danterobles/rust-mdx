//! Zoom del editor y de la vista previa (US-4.5).

use super::{App, Message, Task, strings};

/// Zoom mínimo y máximo (coinciden con el rango de `appearance.zoom`).
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 3.0;
/// Incremento de cada pulsación de `Ctrl/Cmd +` o `-`.
pub const ZOOM_STEP: f32 = 0.1;

/// Cambio de zoom pedido por el usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomChange {
    /// `+1` acerca y `-1` aleja un paso.
    Step(i8),
    /// `Ctrl/Cmd+0`: vuelve al zoom de `config.toml`.
    Reset,
}

/// Aplica un paso al zoom: redondea a décimas (sin arrastrar error de coma flotante) y acota.
#[must_use]
pub fn stepped(zoom: f32, steps: i8) -> f32 {
    let tenths =
        ((zoom / ZOOM_STEP).round() + f32::from(steps)).clamp(MIN_ZOOM * 10.0, MAX_ZOOM * 10.0);
    (tenths * ZOOM_STEP * 10.0).round() / 10.0
}

impl App {
    /// Zoom definido en `config.toml` (el que restablece `Ctrl/Cmd+0`).
    pub(super) fn config_zoom(&self) -> f32 {
        self.config.appearance.zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    }

    pub(super) fn change_zoom(&mut self, change: ZoomChange) -> Task<Message> {
        let new = match change {
            ZoomChange::Step(steps) => stepped(self.zoom, steps),
            ZoomChange::Reset => self.config_zoom(),
        };
        if (new - self.zoom).abs() < f32::EPSILON {
            return Task::none();
        }
        self.zoom = new;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let percent = (new * 100.0).round() as u32;
        self.set_status(
            strings::STATUS_ZOOM.replace("{percent}", &percent.to_string()),
            false,
        );
        // El texto de la vista previa cambia de tamaño: se re-estiman las alturas y se vuelve a
        // medir (scroll sincronizado y vista previa virtualizada).
        self.vp.heights.clear();
        let reset = self.reset_virtual(false);
        Task::batch([reset, self.measure_preview()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_tenths_without_floating_point_drift() {
        let mut z = 1.0;
        for _ in 0..7 {
            z = stepped(z, 1);
        }
        assert!((z - 1.7).abs() < 1e-6, "{z}");
        for _ in 0..7 {
            z = stepped(z, -1);
        }
        assert!((z - 1.0).abs() < 1e-6, "{z}");
    }

    #[test]
    fn zoom_is_clamped_to_the_supported_range() {
        assert!((stepped(3.0, 1) - 3.0).abs() < 1e-6);
        assert!((stepped(0.5, -1) - 0.5).abs() < 1e-6);
        assert!((stepped(0.55, -3) - 0.5).abs() < 1e-6);
        assert!((stepped(2.96, 1) - 3.0).abs() < 1e-6);
    }
}
