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
- **Limitación conocida:** Nuevo/Abrir reemplazan el documento actual sin preguntar aunque tenga cambios (el aviso llega con US-1.2 y las pestañas con US-3.1).
- `text_editor` mínimo (sin fuente monoespaciada, resaltado ni conteos): lo completa US-1.3. Título de ventana «nombre — Mdx»; el `●` llega con US-1.2.
- `doc.text` solo se sincroniza al abrir/guardar (evita O(n) por pulsación); US-1.2 definirá el dirty en vivo.
- Filtro «Todos» con extensión `*`: comportamiento a confirmar por plataforma (👁).
- Tests de `mdx-core` permiten `unwrap` vía `cfg_attr(test, ...)`; NFR-5 sigue vigente para código de producción.
- Guardado atómico: temporal `.<nombre>.mdx-tmp-<pid>` en el mismo directorio; no preserva permisos del original (mejora futura si hace falta).
- Verificación 👁 AC1–AC4 confirmada manualmente por el propietario el 2026-11-23.
