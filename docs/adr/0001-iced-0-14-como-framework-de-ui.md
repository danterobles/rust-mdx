# ADR 0001 — iced 0.14 como framework de UI

**Estado:** Aceptado

## Contexto
Mdx necesita UI de escritorio multiplataforma 100 % Rust con editor de texto, render de Markdown y paneles redimensionables.

## Decisión
Usar `iced` 0.14 (`text_editor`, `markdown`, `pane_grid`, `scrollable`), arquitectura Elm.

## Alternativas
egui (menos adecuado para producto pulido), GPUI (pre-1.0, ecosistema fragmentado; reevaluar en v2), Slint (licencia dual y DSL propio), Tauri (webview, UI no Rust).

## Consecuencias
API pre-1.0: se fija `0.14` y las migraciones son historias propias (`migrate/iced-<ver>`). Widgets propios requieren ADR.
