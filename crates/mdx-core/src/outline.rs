//! Esquema (outline) de un documento Markdown: sus encabezados H1–H6.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Un encabezado del documento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    /// 1..=6.
    pub level: u8,
    /// Texto plano, sin marcado inline (`**negrita**` → `negrita`).
    pub text: String,
    /// Línea de origen, base 0.
    pub line: usize,
}

/// Encabezados de un documento, en orden de aparición.
pub type Outline = Vec<Heading>;

fn level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Extrae los encabezados de `text`.
///
/// Reconoce encabezados ATX (`# Título`) y Setext (`Título` + `===`), ignora los que aparecen
/// dentro de bloques de código o del bloque de metadatos YAML inicial, y devuelve el texto plano
/// (código en línea incluido, sin marcas de énfasis ni enlaces).
#[must_use]
pub fn extract(text: &str) -> Outline {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;

    // Offset del inicio de cada línea, para convertir offsets de bytes en números de línea.
    let mut line_starts = vec![0usize];
    line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    let line_of = |offset: usize| line_starts.partition_point(|&start| start <= offset) - 1;

    let mut outline = Vec::new();
    let mut current: Option<Heading> = None;
    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(Heading {
                    level: level_number(level),
                    text: String::new(),
                    line: line_of(range.start),
                });
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(heading) = current.take() {
                    let text = heading.text.trim().to_owned();
                    outline.push(Heading { text, ..heading });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some(h) = &mut current {
                    h.text.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(h) = &mut current {
                    h.text.push(' ');
                }
            }
            _ => {}
        }
    }
    outline
}

/// Rango de líneas (base 0, fin exclusivo) de un bloque de nivel superior del documento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub start_line: usize,
    pub end_line: usize,
}

/// Bloques de nivel superior (párrafos, encabezados, listas, citas, código, tablas, reglas…) en
/// orden de aparición, con las líneas de origen que ocupan.
///
/// Se omiten los bloques de metadatos y el HTML en bloque, que el visor de la vista previa no
/// dibuja, de modo que la lista coincide uno a uno con los elementos de la vista previa.
#[must_use]
pub fn top_level_blocks(text: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
    let mut line_starts = vec![0usize];
    line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    let line_of = |offset: usize| line_starts.partition_point(|&start| start <= offset) - 1;
    let to_block = |range: &std::ops::Range<usize>| {
        let start_line = line_of(range.start);
        // El rango suele incluir el salto de línea final: se cuenta hasta la última línea con texto.
        let last = text[..range.end.min(text.len())]
            .trim_end_matches(['\n', '\r'])
            .len();
        let end_line = line_of(last.saturating_sub(1).max(range.start)) + 1;
        Block {
            start_line,
            end_line: end_line.max(start_line + 1),
        }
    };

    let mut blocks = Vec::new();
    let mut depth = 0usize;
    let mut skip = false;
    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    skip = matches!(tag, Tag::MetadataBlock(_) | Tag::HtmlBlock);
                }
                depth += 1;
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 0 && !skip {
                    blocks.push(to_block(&range));
                }
            }
            Event::Rule if depth == 0 => blocks.push(to_block(&range)),
            _ => {}
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_levels_text_and_lines() {
        let o = extract("# Uno\n\ntexto\n\n## Dos\n### Tres\n");
        assert_eq!(
            o,
            [
                Heading {
                    level: 1,
                    text: "Uno".into(),
                    line: 0
                },
                Heading {
                    level: 2,
                    text: "Dos".into(),
                    line: 4
                },
                Heading {
                    level: 3,
                    text: "Tres".into(),
                    line: 5
                },
            ]
        );
    }

    #[test]
    fn strips_inline_markup() {
        let o = extract("# Hola **mundo** con `código` y [enlace](http://x) _énfasis_\n");
        assert_eq!(o[0].text, "Hola mundo con código y enlace énfasis");
    }

    #[test]
    fn recognises_setext_headings() {
        let o = extract("Título principal\n================\n\nSubtítulo\n---------\n");
        assert_eq!(o.len(), 2);
        assert_eq!((o[0].level, o[0].line), (1, 0));
        assert_eq!((o[1].level, o[1].line), (2, 3));
    }

    #[test]
    fn ignores_headings_inside_code_blocks_and_front_matter() {
        let text =
            "---\ntitle: x\n# no soy un encabezado\n---\n\n```md\n# tampoco\n```\n\n# Real\n";
        let o = extract(text);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].text, "Real");
        assert_eq!(o[0].line, 9);
    }

    #[test]
    fn handles_crlf_empty_and_no_headings() {
        assert_eq!(extract(""), Vec::new());
        assert_eq!(extract("solo texto\nsin encabezados\n"), Vec::new());
        let o = extract("# A\r\n\r\n## B\r\n");
        assert_eq!(o.iter().map(|h| h.line).collect::<Vec<_>>(), [0, 2]);
    }

    #[test]
    fn empty_heading_has_empty_text_and_multibyte_lines_are_correct() {
        let o = extract("ñandú\n\n#\n\n## Árbol\n");
        assert_eq!(
            o[0],
            Heading {
                level: 1,
                text: String::new(),
                line: 2
            }
        );
        assert_eq!(o[1].line, 4);
    }

    #[test]
    fn snapshot_of_a_realistic_document() {
        let text = "\
# Mdx

Editor de **Markdown**.

## Instalación

```bash
# esto es un comentario, no un encabezado
cargo install mdx
```

### Requisitos

- Rust

## Uso

Título alternativo
------------------

#### Detalle `fino`

##### Nivel 5
###### Nivel 6
";
        insta::assert_debug_snapshot!(extract(text));
    }

    #[test]
    fn top_level_blocks_cover_their_source_lines() {
        let text = "# Título\n\nPárrafo uno\ncontinúa\n\n- a\n- b\n\n```\ncódigo\n```\n\n---\n";
        let blocks = top_level_blocks(text);
        let lines: Vec<_> = blocks.iter().map(|b| (b.start_line, b.end_line)).collect();
        assert_eq!(lines, [(0, 1), (2, 4), (5, 7), (8, 11), (12, 13)]);
    }

    #[test]
    fn top_level_blocks_skip_front_matter_and_handle_empty_input() {
        assert_eq!(top_level_blocks(""), Vec::new());
        let blocks = top_level_blocks("---\ntitle: x\n---\n\n# Real\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start_line, 4);
    }

    #[test]
    fn top_level_blocks_keep_nested_content_inside_one_block() {
        let text = "> cita\n> > anidada\n\n1. uno\n   - sub\n2. dos\n";
        let blocks = top_level_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!((blocks[0].start_line, blocks[0].end_line), (0, 2));
        assert_eq!((blocks[1].start_line, blocks[1].end_line), (3, 6));
    }
}
