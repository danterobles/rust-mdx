# Mdx

Editor de Markdown de escritorio, ligero y multiplataforma (Windows, macOS, Linux), escrito en Rust con [iced](https://iced.rs).

## Estado

En desarrollo (pre-1.0). Funciona hoy: crear, abrir y guardar archivos `.md`, editor con resaltado, deshacer/rehacer, vista previa en vivo (CommonMark + GFM) y barra de estado.

## Uso

```bash
cargo run -p mdx-app
```

| Acción | Windows/Linux | macOS |
|---|---|---|
| Nuevo | Ctrl+N | Cmd+N |
| Abrir | Ctrl+O | Cmd+O |
| Guardar / Guardar como | Ctrl+S / Ctrl+Shift+S | Cmd+S / Cmd+Shift+S |
| Deshacer / Rehacer | Ctrl+Z / Ctrl+Shift+Z (o Ctrl+Y) | Cmd+Z / Cmd+Shift+Z |
| Alternar vista previa | Ctrl+\ | Cmd+\ |

## Desarrollo

Consulta [`CLAUDE.md`](CLAUDE.md) y la carpeta [`docs/`](docs/) (PRD, arquitectura, datos, stack y checklist).

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Licencia

[MIT](LICENSE). Código fuente: <https://github.com/danterobles/rust-mdx>.
