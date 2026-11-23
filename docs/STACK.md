# STACK.md — Stack tecnológico de Mdx

> Lista cerrada de herramientas y dependencias. Agregar, quitar o cambiar de versión mayor/menor algo de esta lista requiere ADR y entrada en `CHANGELOG.md`.
> **Versiones:** las marcadas con 🔎 deben verificarse en crates.io al iniciar el proyecto (US-0.1) y fijarse aquí con la versión exacta encontrada.

---

## 1. Toolchain

| Herramienta | Versión | Notas |
|---|---|---|
| Rust | stable, edition 2024 | MSRV = **1.92** (la exige `rfd 0.18`; iced 0.14 pide 1.88); fijada en `rust-toolchain.toml` y `rust-version` del workspace |
| Cargo resolver | `"3"` | |
| rustfmt | stable | `rustfmt.toml` en raíz |
| clippy | stable | `-D warnings` en CI |

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

## 2. Dependencias de producción

### 2.1 `mdx-app`

| Crate | Versión | Features | Para qué |
|---|---|---|---|
| `iced` | `0.14` | `tokio`, `highlighter`, `markdown`, `image`, `svg`, `wgpu`, `tiny-skia` (fallback), `crisp`, `x11`, `wayland` | Framework UI |
| `rfd` | `0.18` | default (`xdg-portal`, `wayland`); 0.18 no tiene feature `tokio` | Diálogos nativos de abrir/guardar |
| `open` | `5.4`| — | Abrir enlaces y "revelar en el sistema" |
| `arboard` | `3.7`| `image-data` | Pegar imágenes del portapapeles (US-4.4) |
| `anyhow` | `1` | — | Errores en el borde de la app |
| `tracing` | `0.1` | — | Logs |
| `tracing-subscriber` | `0.3` | `env-filter` | Salida de logs (`RUST_LOG`) |
| `mdx-core` | path | — | Lógica |

### 2.2 `mdx-core`

| Crate | Versión | Features | Para qué |
|---|---|---|---|
| `pulldown-cmark` | `0.13` | default | Outline y mapeo de líneas (preview usa el de iced) |
| `comrak` | `1.2`| `default-features = false` + lo necesario | Exportación HTML GFM |
| `ignore` | `0.4`| — | Recorrido de directorios respetando `.gitignore` |
| `notify` | `9`| — | Watcher del sistema de archivos |
| `notify-debouncer-full` | `0.8`| — | Debounce de eventos |
| `trash` | `5.4`| — | Enviar a la papelera |
| `directories` | `6`| — | Rutas de config/estado por SO |
| `serde` | `1` | `derive` | Serialización |
| `toml` | `1.2`| — | `config.toml`, `session.toml` |
| `regex` | `1` | — | Búsqueda |
| `xxhash-rust` | `0.8`| `xxh3` | Hash de contenido (dirty state) |
| `thiserror` | `2` | — | Errores tipados |
| `tokio` | `1` | `fs`, `rt`, `sync` | I/O async |
| `tokio-util` | `0.7`| — | `CancellationToken` para búsqueda |
| `futures` | `0.3` | — | `Stream` del watcher/búsqueda |
| `tracing` | `0.1` | — | Logs |

## 3. Dependencias de desarrollo

| Crate | Para qué |
|---|---|
| `tempfile` | Directorios temporales en tests (obligatorio para todo test que toque disco) |
| `insta` (`toml`, `yaml`) | Snapshot tests |
| `pretty_assertions` | Diffs legibles |
| `tokio` (`macros`, `rt-multi-thread`) | `#[tokio::test]` |

## 4. Herramientas de desarrollo (instalar con `cargo install`)

| Herramienta | Uso |
|---|---|
| `cargo-deny` | Licencias, advisories, duplicados |
| `cargo-insta` | Revisar snapshots |
| `cargo-llvm-cov` | Cobertura (NFR-9) |
| `cargo-packager` | Instaladores (F5) |
| `git-cliff` (opcional) | Generar borradores de CHANGELOG desde commits convencionales |
| `cargo-release` (opcional) | Bump de versión coordinado en el workspace |

## 5. Configuración del workspace

```toml
# Cargo.toml (raíz)
[workspace]
resolver = "3"
members = ["crates/mdx-core", "crates/mdx-app", "xtask"]

[workspace.package]
version = "0.0.0"          # ÚNICA fuente de versión; ver CLAUDE.md §Versionado
edition = "2024"
license = "TBD"            # pregunta abierta del PRD
repository = "TBD"
rust-version = "1.92"      # MSRV: rfd 0.18 (iced 0.14: 1.88)

[workspace.dependencies]
iced = { version = "0.14", default-features = false, features = [...] }
# ... resto de la tabla §2, fijadas aquí y heredadas con `.workspace = true`

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"

[profile.dev.package."*"]
opt-level = 2              # dependencias optimizadas en debug: UI fluida al desarrollar
```

En `mdx-core/src/lib.rs` además:
```rust
#![deny(clippy::unwrap_used, clippy::expect_used)]
```

## 6. Integración continua (GitHub Actions)

| Workflow | Disparador | Jobs |
|---|---|---|
| `ci.yml` | push / PR | Matriz `ubuntu-latest`, `macos-latest`, `windows-latest`: `fmt --check`, `clippy -D warnings`, `test --workspace`; Linux además `cargo deny check` y `xtask size` |
| `release.yml` | tag `v*` | `cargo packager --release` por plataforma; adjunta artefactos al GitHub Release; cuerpo del release = sección del CHANGELOG de esa versión |

Dependencias de sistema en Linux CI: `libxkbcommon-dev`, `libwayland-dev`, `libgtk-3-dev` (para `rfd`), `pkg-config` 🔎 confirmar con la documentación de iced y rfd.

## 7. Empaquetado (F5)

| Plataforma | Formato | Arquitecturas |
|---|---|---|
| Windows 10+ | `.msi` | x86_64 (arm64 opcional) |
| macOS 13+ | `.dmg` (universal) | x86_64 + aarch64 |
| Linux | `.AppImage`, `.deb` | x86_64 |

Identificador: `mx.doitconsulting.mdx` (pendiente de confirmar, PRD §12).

## 8. Política de licencias (`deny.toml`)

Permitidas: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`, `MPL-2.0`, `CC0-1.0`, `OFL-1.1` (fuentes), `BSL-1.0` y `NCSA` (permisivas, solo transitivas de iced; añadidas en US-0.2).
Prohibidas: `GPL-*`, `AGPL-*`, `LGPL-*` (salvo ADR), licencias desconocidas.

## 9. Política de actualización de dependencias

- Parches (`x.y.Z`): libres; `cargo update` mensual en commit `chore(deps): ...`.
- Menores en crates 0.x y mayores en crates ≥ 1.0: ADR corto + entrada en CHANGELOG (**Changed**).
- **iced**: migración de versión = historia propia (`US-M.x`), nunca mezclada con features.

## 10. Alternativas descartadas (resumen; detalle en ADR-0001)

| Opción | Motivo de descarte |
|---|---|
| egui | Excelente para prototipos/herramientas, menos adecuado para un producto de escritorio pulido |
| GPUI (Zed) | Mejor rendimiento, pero pre-1.0, ecosistema fragmentado en forks y poca documentación. Reevaluar para v2 |
| Slint | Licencia dual (GPL/royalty-free) y lenguaje de marcado propio |
| Tauri | Webview: más peso en runtime y la UI no sería Rust |
