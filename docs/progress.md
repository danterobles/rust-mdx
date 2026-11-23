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
