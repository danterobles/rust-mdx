# progress.md — Bitácora de Mdx

## US-0.1 Workspace y CI — ✅ completa salvo verificación de CI remota (rama feat/us-0.1-workspace-ci)
Plan: `Cargo.toml` (workspace), `crates/mdx-core`, `crates/mdx-app` (bin `mdx`), `xtask`, `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml`, `.gitignore`, `.github/workflows/ci.yml`, ADR 0001–0004.
- AC1 🧪 `cargo build --workspace` y `cargo test --workspace` (test `mdx_core::tests::version_is_semver_like`)
- AC2 🧪 `ci.yml` con matriz ubuntu/macos/windows (fmt, clippy, test); ~ el workflow no se pudo ejecutar sin push (requiere autorización); verificar en el primer push. Dependencias de sistema Linux de iced/rfd por confirmar en US-0.2
- AC3 🧪 `cargo clippy --workspace --all-targets -- -D warnings` limpio
- AC4 🧪 `deny.toml` + job `cargo deny` en CI
Decisiones:
- MSRV `1.92`: la exige `rfd 0.18` (iced 0.14 pide 1.88; `notify 9` pide 1.91).
- Versiones fijadas en `[workspace.dependencies]` (verificadas con `cargo info` el 2026-11-23); los crates solo declaran lo que usan hoy (`mdx-app`: `tracing`, `tracing-subscriber`, `mdx-core`). `iced` y el resto se activan en las historias que los necesitan.
- `license` y `repository` siguen en `TBD` (preguntas abiertas del PRD §12).
- AC4 🧪 `cargo deny check` → advisories/bans/licenses/sources ok (cargo-deny 0.22.0). Crates del workspace con `publish = false` y `licenses.private.ignore` mientras la licencia sea TBD.

## US-0.2 Ventana base — ✅ completa (rama feat/us-0.2-ventana-base)
Plan: `mdx-app/src/{main,app,layout,strings}.rs`; `iced::application` con `pane_grid` (Explorador | Editor | Preview) + barra de estado.
- AC1 👁 `cargo run -p mdx-app` arranca y crea la ventana (log de `iced_winit`, título "Mdx"). No pude capturar pantalla desde el agente → confirmación visual del usuario pendiente.
- AC2 🧪 `layout::tests::initial_layout_is_20_40_40` + 👁 pendiente.
- AC3 🧪 `layout::tests::dividers_can_be_resized` (resize por `State::resize`); 👁 arrastre real pendiente.
- AC4 👁 barra de estado vacía bajo el grid; pendiente de confirmar.
Decisiones:
- Raíz = split vertical 0.2; derecha = split vertical 0.5 (⇒ 20/40/40). Tema fijo `Light` hasta US-4.1.
- iced con `default-features = false` pierde `x11`/`wayland`/`crisp`: se añadieron a las features del workspace.
- `cargo deny`: se permitieron `BSL-1.0` (clipboard-win, error-code) y `NCSA` (libfuzzer-sys vía image→rav1e), ambas permisivas y solo transitivas. Cambio de política en STACK §8 → confirmar con el propietario.
- Textos en `strings.rs` (NFR-8). Dependencias de sistema Linux de iced/rfd siguen por confirmar en CI.
- Verificación 👁 AC1–AC4 confirmada manualmente por el propietario (macOS) el 2026-11-23.

## US-1.1 Nuevo / Abrir / Guardar — ✅ completa (rama feat/us-1.1-open-save)
Plan: `mdx-core` (`error.rs`, `document.rs`, `io.rs`), `mdx-app` (`shortcuts.rs`, `app.rs` con `text_editor` mínimo, `strings.rs`).
- AC1 👁 Ctrl/Cmd+N → documento «Sin título» (🧪 mapeo `shortcuts::tests::maps_file_shortcuts`)
- AC2 👁 Ctrl/Cmd+O → `rfd::AsyncFileDialog` con filtros «Markdown y texto» (`md`,`markdown`,`txt`) y «Todos» (`*`)
- AC3 👁 Ctrl/Cmd+S → guarda; sin ruta → Guardar como
- AC4 👁 Ctrl/Cmd+Shift+S → Guardar como
- AC5 🧪 `io::tests::atomic_failure_keeps_original` (fallo simulado antes del `rename`; original intacto y sin temporal)
- AC6 🧪 `io::tests::roundtrip_preserves_crlf_and_bom`, `edited_crlf_file_stays_crlf`, `document::tests::roundtrip_is_byte_identical`
- AC7 🧪 `io::tests::non_utf8_is_clear_error`; en la UI el error se muestra en la barra de estado y el documento actual no cambia.
Decisiones / limitaciones:
- `rfd 0.18` no tiene feature `tokio` (STACK.md decía lo contrario): se usan features por defecto (`xdg-portal`, `wayland`). Corregido en STACK.
- Un solo documento en memoria; `Message::Saved` lleva el texto guardado para que `is_dirty` sea correcto si se edita durante el guardado. DATA.md prevé `DocumentId` en los mensajes: llega con pestañas (US-3.1).
- **Limitación conocida (resuelta en US-1.2):** Nuevo/Abrir reemplazaban el documento sin preguntar.
- `text_editor` mínimo (sin fuente monoespaciada, resaltado ni conteos): lo completa US-1.3. Título de ventana «nombre — Mdx»; el `●` llega con US-1.2.
- `doc.text` solo se sincronizaba al abrir/guardar; desde US-1.2 se sincroniza en cada edición.
- Filtro «Todos» con extensión `*`: comportamiento a confirmar por plataforma (👁).
- Tests de `mdx-core` permiten `unwrap` vía `cfg_attr(test, ...)`; NFR-5 sigue vigente para código de producción.
- Guardado atómico: temporal `.<nombre>.mdx-tmp-<pid>` en el mismo directorio; no preserva permisos del original (mejora futura si hace falta).
- Verificación 👁 AC1–AC4 confirmada manualmente por el propietario el 2026-11-23.

## US-1.2 Estado "sin guardar" — ✅ completa (rama feat/us-1.2-sin-guardar)
Plan: `app.rs` (`dirty`, `PendingAction`, `GuardChoice`, `after_save`), `strings.rs`, `main.rs` (`exit_on_close_request(false)`).
- AC1 🧪 `app::tests::editing_marks_dirty_in_title` + 👁 título `● nombre — Mdx`
- AC2 👁 al cerrar con cambios aparece el diálogo Guardar / Descartar / Cancelar (`rfd::AsyncMessageDialog`, botones personalizados). Guardar → guarda y luego cierra (si el guardado falla o se cancela «Guardar como», no cierra). Descartar → cierra. Cancelar → sigue abierto.
- AC3 🧪 `app::tests::returning_to_saved_state_clears_indicator` (comparación por hash xxh3 contra `saved_hash`)
Decisiones:
- El mismo diálogo protege Nuevo (Ctrl/Cmd+N) y Abrir (Ctrl/Cmd+O) con cambios pendientes: cierra el riesgo de pérdida de datos anotado en US-1.1. Es una extensión mínima del AC2, no una funcionalidad nueva.
- `dirty` se cachea y se recalcula solo en acciones de edición (`Action::is_edit`), al guardar y al cargar. Cada edición materializa `content.text()` + hash: **medir en NFR-4 (1 MB)** al cierre de F1; si no es fluido, pasar a hash incremental/debounce.
- Si el usuario edita mientras se guarda, el documento queda sucio (se compara contra el contenido actual).
- Mensaje de aviso con nombre: «{name}» se sustituye en `strings::UNSAVED_BODY`.
- Con `exit_on_close_request(false)` el cierre lo decide la app (`iced::exit()`).
- Verificación 👁 AC1–AC2 confirmada manualmente por el propietario el 2026-11-23.

## US-1.3 Editor — ✅ completa (rama feat/us-1.3-editor)
Plan: `mdx-core` (`history.rs`, `document::count_words`), `mdx-app` (`editor.rs`, `statusbar.rs`, `app.rs`, `shortcuts.rs`).
- AC1 👁 `Font::MONOSPACE` en el `text_editor`.
- AC2 👁 `.highlight("markdown", InspiredGitHub)` (iced_highlighter/syntect; tema claro acorde al tema fijo `Light`).
- AC3 🧪 `history::tests::*` (round trip, ráfagas, redo descartado, multibyte, límite de memoria, texto divergente) y `app::tests::undo_and_redo_typing_burst`, `undo_restores_cursor_and_edit_after_undo_drops_redo`. Atajos: `shortcuts::tests::maps_undo_redo`.
- AC4 🧪 `app::tests::indent_and_unindent_selection`; `key_binding` mapea Tab/Shift+Tab a `Edit::Indent/Unindent`.
- AC5 🧪 `statusbar::tests::*` (formato, columna en caracteres), `document::tests::counts_words`, `app::tests::stats_follow_cursor_and_words`. Se muestra a la derecha de la barra de estado; los mensajes (abierto/guardado/errores) siguen a la izquierda.
Decisiones:
- **iced 0.14 no tiene deshacer/rehacer** en `text_editor`: se implementó `History` propio (ADR 0005), sin dependencias nuevas.
- El parche de deshacer se aplica seleccionando el rango y pegando/borrando para no reconstruir el `Content` (conserva scroll).
- La indentación con `Edit::Indent` usa el ancho de tabulación por defecto del motor (8 espacios, observado en test). Sin selección, inserta espacios en el cursor; con selección, indenta las líneas. No configurable en v1.
- Palabras = secuencias separadas por espacios en blanco; se recalculan en cada edición (O(n)). **Medir en NFR-4 (1 MB)** al cierre de F1.
- Columna mostrada en caracteres (no bytes).
- Verificación 👁 AC1–AC5 confirmada manualmente por el propietario el 2026-11-23.

## US-1.4 Vista previa en vivo — ✅ completa (rama feat/us-1.4-preview)
Plan: `mdx-core::link` (clasificación de enlaces), `mdx-app` (`app.rs`: preview, debounce, enlaces, toggle; `layout.rs`; `shortcuts.rs`), `fixtures/gfm_showcase.md`.
- AC1 🧪 `app::tests::preview_updates_only_after_debounce`, `loaded_document_renders_preview_immediately`; 👁 pendiente. Debounce: `time::every(25 ms)` solo mientras `preview_pending`; re-parsea cuando `now - última edición ≥ 150 ms` (ARCHITECTURE decía 50 ms de tick: se bajó a 25 ms para quedar más cerca de «≤ 150 ms»).
- AC2 👁 `iced::widget::markdown` (pulldown-cmark con tablas, tachado y listas de tareas activados por iced); código resaltado vía feature `highlighter`. Verificar con `fixtures/gfm_showcase.md`.
- AC3 🧪 `link::tests::*` (http/https externos, relativos con `../`, `#ancla`, `%20`, esquemas no soportados), `app::tests::unsupported_link_reports_error_and_keeps_document`; 👁 apertura real en navegador/editor. Los `.md` relativos pasan por el diálogo de cambios sin guardar (`PendingAction::OpenPath`).
- AC4 🧪 `shortcuts::tests::maps_toggle_preview_by_physical_key`, `app::tests::toggle_preview_hides_and_restores_pane`; 👁 pendiente.
Decisiones:
- **Tablas sí soportadas** por el widget `markdown` de iced 0.14: no hizo falta ADR ni widget propio.
- Ctrl/Cmd+\ se detecta por **tecla física** (`Code::Backslash`) porque en teclados como el español `\` requiere Alt.
- Ocultar la vista previa reconstruye el layout (Explorador 20 % · Editor 80 %); al mostrarla se restaura 20/40/40. Los anchos arrastrados por el usuario no se recuerdan (podría guardarse con la sesión en US-3.5).
- La preview se re-parsea completa en cada pausa (el parseo incremental de iced solo sirve para añadir al final). **Medir en NFR-4 (1 MB)**; si no es fluido, mover el parseo a un `Task`.
- `open::that_detached` se ejecuta dentro de un `Task` (no en `update`).
- Enlaces a `.md` relativos requieren que el documento tenga ruta; sin ruta se informa «Enlace no soportado».
- Imágenes: fuera de esta historia (US-4.4).
- Verificación 👁 AC1–AC4 confirmada manualmente por el propietario el 2026-11-23.

## Cierre de F1 — mediciones (rama chore/cierre-f1)
- `cargo xtask gen-fixtures` genera `fixtures/large_1mb.md` y `large_5mb.md` (ignorados por git); `cargo xtask size` verifica NFR-1.
- Test de rendimiento `app::tests::perf_edit_cost_on_large_documents` (`#[ignore]`): `cargo test --release -p mdx-app perf_ -- --ignored --nocapture`.
- **Hallazgo NFR-4:** el re-parseo de la vista previa en el hilo de UI costaba 258 ms (1 MB) y 3,5 s (5 MB) tras cada pausa. Se movió a `Task` + `spawn_blocking` con generación para descartar resultados viejos (`Message::PreviewParsed`); `preview` pasó de `markdown::Content` a `Vec<markdown::Item>`.
- **Observación abierta:** abrir un archivo grande bloquea la UI mientras `text_editor::Content::with_text` lo carga (0,7 s con 1 MB; 3,4 s con 5 MB). No se puede mover a otro hilo con la API actual de iced; aceptable para «usable con 5 MB» pero a vigilar. El coste de dibujar la preview de un documento enorme (iced no virtualiza) solo se puede ver a mano.
- **NFR-3:** la métrica fiable en macOS es el *physical footprint* (63 MB, pico 71 MB), no el RSS (102 MB). `tiny-skia` usa más memoria (RSS 153 MB) que `wgpu`.
- **NFR-2:** sin instrumentación de «primer frame»; ≈130 ms hasta que wgpu selecciona el adaptador. Confirmación subjetiva/👁 del propietario.
- Cobertura: `cargo-llvm-cov` 0.9.2 instalado; `mdx-core` ≈ 97 % por archivo.
- Hito F1: se añadió `README.md` en la raíz para poder editarlo de principio a fin (👁 del propietario).
- Mediciones hechas solo en macOS (arm64); Windows/Linux quedan para QA de F5.

## Cierre de F1 → v0.2.0 (2026-11-23)
- Decisión del propietario: ir directo a 0.2.0 sin cerrar F0 (se omite 0.1.0). F0 sigue con AC2 de US-0.1 en `[~]` (CI remoto sin verificar por falta de `push`), licencia y URL del repositorio en `TBD`.
- Historias F1: US-1.1, US-1.2, US-1.3 y US-1.4 completas (AC verificados).
- **Pendientes de verificación 👁 al cerrar** (no bloquean según decisión del propietario, quedan abiertos): Hito F1 (editar README sin pérdida de datos), NFR-2 (primer frame), NFR-3 con archivo de 100 KB, NFR-4 visual con `large_1mb/5mb`.
- Tag `v0.2.0`: **no creado**; requiere autorización explícita.
