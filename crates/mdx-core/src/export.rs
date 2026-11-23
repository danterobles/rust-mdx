//! Exportación de un documento Markdown a HTML autocontenido (US-4.3).

use comrak::{Options, markdown_to_html};

/// Hoja de estilos embebida. Se adapta sola al modo claro/oscuro del navegador.
const CSS: &str = r"
:root {
  color-scheme: light dark;
  --fg: #1f2328; --bg: #ffffff; --muted: #59636e; --border: #d1d9e0;
  --code-bg: #f6f8fa; --link: #0969da; --quote: #59636e;
}
@media (prefers-color-scheme: dark) {
  :root {
    --fg: #e6edf3; --bg: #0d1117; --muted: #9198a1; --border: #3d444d;
    --code-bg: #151b23; --link: #4493f8; --quote: #9198a1;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg);
  font: 16px/1.6 system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; }
article { max-width: 860px; margin: 0 auto; padding: 32px 24px 64px; }
h1, h2, h3, h4, h5, h6 { margin: 1.6em 0 0.6em; line-height: 1.25; font-weight: 600; }
h1 { font-size: 2em; padding-bottom: .3em; border-bottom: 1px solid var(--border); }
h2 { font-size: 1.5em; padding-bottom: .3em; border-bottom: 1px solid var(--border); }
h3 { font-size: 1.25em; } h4 { font-size: 1em; } h5 { font-size: .875em; }
h6 { font-size: .85em; color: var(--muted); }
p, ul, ol, blockquote, pre, table { margin: 0 0 1em; }
a { color: var(--link); text-decoration: none; } a:hover { text-decoration: underline; }
ul, ol { padding-left: 2em; }
li > p { margin: .25em 0; }
li.task-list-item, ul:has(> li.task-list-item) { list-style: none; }
li.task-list-item input { margin: 0 .5em 0 -1.4em; vertical-align: middle; }
blockquote { padding: 0 1em; color: var(--quote); border-left: .25em solid var(--border); }
hr { height: .25em; padding: 0; margin: 1.5em 0; background: var(--border); border: 0; }
code { font: .875em ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  background: var(--code-bg); padding: .2em .4em; border-radius: 6px; }
pre { background: var(--code-bg); padding: 16px; overflow: auto; border-radius: 6px; line-height: 1.45; }
pre code { background: none; padding: 0; font-size: .875em; }
table { border-collapse: collapse; display: block; max-width: 100%; overflow: auto; }
th, td { padding: 6px 13px; border: 1px solid var(--border); }
th { font-weight: 600; } tr:nth-child(2n) td { background: var(--code-bg); }
img { max-width: 100%; }
.footnotes { margin-top: 2em; padding-top: 1em; border-top: 1px solid var(--border);
  color: var(--muted); font-size: .9em; }
";

/// Opciones de `comrak`: `CommonMark` + GFM (tablas, tachado, tareas, autoenlaces), notas al pie, identificadores en los encabezados y bloque de metadatos YAML
/// omitido. El HTML crudo del documento **se escapa** (modo seguro).
fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.header_id_prefix = Some(String::new());
    options.extension.front_matter_delimiter = Some("---".to_owned());
    // `render.r#unsafe` queda en `false` (por defecto): el HTML crudo no se emite.
    options.render.r#unsafe = false;
    options
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Cuerpo HTML (sin documento ni estilos) del Markdown `text`.
#[must_use]
pub fn body_html(text: &str) -> String {
    markdown_to_html(text, &options())
}

/// Documento HTML autocontenido: `<!doctype html>`, CSS embebido y el contenido de `text`.
/// `title` va en `<title>` (se escapa). No depende de recursos externos salvo las imágenes que
/// el propio Markdown referencie.
#[must_use]
pub fn to_html(text: &str, title: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"es\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>{CSS}</style>\n</head>\n<body>\n<article>\n{}</article>\n</body>\n</html>\n",
        escape_html(title),
        body_html(text)
    )
}

/// Genera el HTML de `text` y lo escribe de forma atómica en `path` (sin bloquear la UI).
///
/// # Errors
/// [`CoreError`] si no se puede escribir el archivo.
pub async fn export_to_file(
    path: std::path::PathBuf,
    text: String,
    title: String,
) -> Result<(), crate::error::CoreError> {
    let html = tokio::task::spawn_blocking(move || to_html(&text, &title))
        .await
        .map_err(|e| crate::error::CoreError::Io(e.to_string()))?;
    crate::io::write_atomic(&path, html.into_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOWCASE: &str = "\
---
title: metadatos que no deben salir
---

# Título principal

Texto con *cursiva*, **negrita**, ~~tachado~~, `código` y un [enlace](https://example.com).
Autoenlace: https://www.rust-lang.org

## Listas

- uno
- dos
  - anidado

1. primero
2. segundo

- [x] tarea hecha
- [ ] tarea pendiente

> Una cita
> en dos líneas.

```rust
fn main() {
    println!(\"Hola <mundo> & compañía\");
}
```

| A | B |
|:--|--:|
| 1 | 2 |

Nota al pie[^1].

[^1]: Texto de la nota.

---

HTML crudo: <script>alert(1)</script> y <b>negrita</b>.

[peligroso](javascript:alert(1))
";

    #[test]
    fn snapshot_of_the_exported_body() {
        // AC2
        insta::assert_snapshot!(body_html(SHOWCASE));
    }

    #[test]
    fn snapshot_of_the_whole_document() {
        // AC1/AC2: estructura completa (el CSS se sustituye para que el snapshot sea legible).
        let html = to_html("# Hola\n\ntexto\n", "Mi <nota> & más");
        let without_css = html.replace(CSS, "/* CSS */");
        insta::assert_snapshot!(without_css);
    }

    #[test]
    fn the_document_is_self_contained() {
        // AC1: CSS embebido y ningún recurso externo.
        let html = to_html(SHOWCASE, "Doc");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<style>") && html.contains("prefers-color-scheme: dark"));
        for external in [
            "<link ",
            "<script src",
            "@import",
            "http://fonts",
            "https://fonts",
        ] {
            assert!(!html.contains(external), "recurso externo: {external}");
        }
    }

    #[test]
    fn gfm_features_are_rendered() {
        let html = body_html(SHOWCASE);
        for expected in [
            "<table>",
            "<del>tachado</del>",
            "type=\"checkbox\"",
            "<a href=\"https://www.rust-lang.org\">",
            "<h1 id=\"título-principal\">",
            "footnote",
            "<blockquote>",
            "<pre><code class=\"language-rust\">",
        ] {
            assert!(html.contains(expected), "falta {expected} en:\n{html}");
        }
    }

    #[test]
    fn raw_html_scripts_and_javascript_links_are_neutralised() {
        let html = body_html(SHOWCASE);
        assert!(!html.contains("<script>"), "{html}");
        assert!(!html.contains("javascript:"), "{html}");
        assert!(html.contains("&lt;script&gt;") || html.contains("<!-- raw HTML omitted -->"));
    }

    #[test]
    fn front_matter_does_not_appear_in_the_output() {
        let html = body_html(SHOWCASE);
        assert!(!html.contains("metadatos que no deben salir"));
    }

    #[test]
    fn the_title_is_escaped() {
        let html = to_html("x", "</title><script>1</script>");
        assert!(html.contains("<title>&lt;/title&gt;&lt;script&gt;1&lt;/script&gt;</title>"));
        assert!(!html.contains("<title></title>"));
    }

    #[test]
    fn empty_input_still_gives_a_valid_document() {
        let html = to_html("", "Vacío");
        assert!(html.contains("<article>\n</article>"));
    }

    #[tokio::test]
    async fn export_to_file_writes_the_document_atomically() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("salida.html");
        std::fs::write(&path, "viejo").unwrap();
        export_to_file(path.clone(), "# Hola\n".to_owned(), "Doc".to_owned())
            .await
            .unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.starts_with("<!doctype html>") && written.contains("<h1 id=\"hola\">"));
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "sin temporales"
        );
    }

    #[tokio::test]
    async fn export_to_an_unwritable_place_is_a_clear_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("no/existe/salida.html");
        let err = export_to_file(path, String::new(), String::new())
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            crate::error::CoreError::NotFound(_) | crate::error::CoreError::Io(_)
        ));
    }
}
