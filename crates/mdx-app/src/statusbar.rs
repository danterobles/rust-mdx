//! Texto de la barra de estado (lógica pura, testeable).

use mdx_core::LineEnding;

use crate::strings;

/// Datos mostrados a la derecha de la barra de estado.
pub struct Stats {
    /// Línea y columna base 1 (la columna cuenta caracteres, no bytes).
    pub line: usize,
    pub col: usize,
    pub words: usize,
    pub line_ending: LineEnding,
    pub has_bom: bool,
}

impl Stats {
    /// «Ln 3, Col 5  ·  12 palabras  ·  LF  ·  UTF-8».
    #[must_use]
    pub fn render(&self) -> String {
        let pos = strings::STATUS_LINE_COL
            .replace("{line}", &self.line.to_string())
            .replace("{col}", &self.col.to_string());
        let words = if self.words == 1 {
            strings::STATUS_WORD.to_owned()
        } else {
            strings::STATUS_WORDS.replace("{n}", &self.words.to_string())
        };
        let eol = match self.line_ending {
            LineEnding::Lf => strings::EOL_LF,
            LineEnding::CrLf => strings::EOL_CRLF,
        };
        let enc = if self.has_bom {
            strings::ENCODING_UTF8_BOM
        } else {
            strings::ENCODING_UTF8
        };
        format!("{pos}  ·  {words}  ·  {eol}  ·  {enc}")
    }
}

/// Columna base 1 en caracteres a partir de la columna en bytes del cursor.
#[must_use]
pub fn display_col(line_text: &str, byte_col: usize) -> usize {
    let end = byte_col.min(line_text.len());
    line_text.get(..end).map_or(end, |s| s.chars().count()) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_fields() {
        let s = Stats {
            line: 3,
            col: 5,
            words: 12,
            line_ending: LineEnding::CrLf,
            has_bom: true,
        };
        assert_eq!(
            s.render(),
            "Ln 3, Col 5  ·  12 palabras  ·  CRLF  ·  UTF-8 con BOM"
        );
    }

    #[test]
    fn singular_word_and_plain_utf8() {
        let s = Stats {
            line: 1,
            col: 1,
            words: 1,
            line_ending: LineEnding::Lf,
            has_bom: false,
        };
        assert_eq!(s.render(), "Ln 1, Col 1  ·  1 palabra  ·  LF  ·  UTF-8");
    }

    #[test]
    fn column_counts_chars_not_bytes() {
        assert_eq!(display_col("añb", 0), 1);
        assert_eq!(display_col("añb", 3), 3); // tras «añ» (3 bytes) hay 2 chars
        assert_eq!(display_col("añb", 99), 4);
    }
}
