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

## US-0.2 Ventana base — 🟡 implementada, pendiente verificación visual 👁 (rama feat/us-0.2-ventana-base)
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
