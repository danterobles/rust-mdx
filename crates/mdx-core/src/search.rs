//! Búsqueda y reemplazo en un texto y búsqueda en todos los archivos de una carpeta.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ignore::WalkBuilder;
use regex::{NoExpand, Regex, RegexBuilder};

use crate::error::CoreError;
use crate::link::MARKDOWN_EXTENSIONS;

/// Máximo de coincidencias que devuelve la búsqueda en carpeta (evita resultados inmanejables).
pub const MAX_WORKSPACE_MATCHES: usize = 5_000;
/// Archivos más grandes que esto se omiten en la búsqueda en carpeta.
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// Longitud máxima, en caracteres, del extracto de línea de cada resultado.
const PREVIEW_CHARS: usize = 200;

/// Qué y cómo buscar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchQuery {
    pub pattern: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

impl SearchQuery {
    /// Compila la consulta. `None` si el patrón está vacío (no busca nada).
    ///
    /// # Errors
    /// [`CoreError::InvalidPattern`] si el patrón regex no es válido.
    pub fn compile(&self) -> Result<Option<Regex>, CoreError> {
        if self.pattern.is_empty() {
            return Ok(None);
        }
        let body = if self.regex {
            self.pattern.clone()
        } else {
            regex::escape(&self.pattern)
        };
        let source = if self.whole_word {
            format!(r"\b(?:{body})\b")
        } else {
            body
        };
        RegexBuilder::new(&source)
            .case_insensitive(!self.case_sensitive)
            .multi_line(true)
            .build()
            .map(Some)
            .map_err(|e| CoreError::InvalidPattern(first_line(&e.to_string())))
    }
}

fn first_line(s: &str) -> String {
    // Los errores de `regex` son multilínea; en la UI basta la línea con el motivo.
    s.lines()
        .rev()
        .find(|l| l.trim_start().starts_with("error:"))
        .map_or_else(
            || s.lines().next().unwrap_or("").to_owned(),
            |l| {
                l.trim_start()
                    .trim_start_matches("error:")
                    .trim()
                    .to_owned()
            },
        )
}

/// Coincidencias de `query` en `text`, como rangos de bytes, en orden.
///
/// # Errors
/// [`CoreError::InvalidPattern`] si el patrón regex no es válido.
pub fn find_all(text: &str, query: &SearchQuery) -> Result<Vec<Range<usize>>, CoreError> {
    let Some(re) = query.compile()? else {
        return Ok(Vec::new());
    };
    Ok(re
        .find_iter(text)
        .filter(|m| !m.is_empty())
        .map(|m| m.range())
        .collect())
}

/// Texto con el que se reemplazaría la coincidencia `range` (en modo regex expande `$1`, `$name`).
///
/// # Errors
/// [`CoreError::InvalidPattern`] si el patrón regex no es válido.
pub fn replacement_for(
    text: &str,
    query: &SearchQuery,
    range: &Range<usize>,
    replacement: &str,
) -> Result<String, CoreError> {
    if !query.regex {
        return Ok(replacement.to_owned());
    }
    let Some(re) = query.compile()? else {
        return Ok(replacement.to_owned());
    };
    let mut out = String::new();
    if let Some(caps) = re.captures_at(text, range.start).filter(|c| {
        c.get(0)
            .is_some_and(|m| m.start() == range.start && m.end() == range.end)
    }) {
        caps.expand(replacement, &mut out);
        Ok(out)
    } else {
        Ok(replacement.to_owned())
    }
}

/// Reemplaza todas las coincidencias. Devuelve el texto nuevo y cuántas se reemplazaron.
///
/// # Errors
/// [`CoreError::InvalidPattern`] si el patrón regex no es válido.
pub fn replace_all(
    text: &str,
    query: &SearchQuery,
    replacement: &str,
) -> Result<(String, usize), CoreError> {
    let Some(re) = query.compile()? else {
        return Ok((text.to_owned(), 0));
    };
    let count = re.find_iter(text).filter(|m| !m.is_empty()).count();
    if count == 0 {
        return Ok((text.to_owned(), 0));
    }
    let out = if query.regex {
        re.replace_all(text, replacement).into_owned()
    } else {
        re.replace_all(text, NoExpand(replacement)).into_owned()
    };
    Ok((out, count))
}

/// Una coincidencia en un archivo de la carpeta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    /// Base 0.
    pub line: usize,
    /// En bytes dentro de la línea.
    pub col_range: Range<usize>,
    /// La línea, recortada a 200 caracteres.
    pub preview: String,
}

/// Resultado de buscar en una carpeta.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceSearch {
    /// Ordenadas por archivo y posición.
    pub matches: Vec<SearchMatch>,
    pub files_searched: usize,
    /// Se alcanzó [`MAX_WORKSPACE_MATCHES`] y hay más resultados sin listar.
    pub truncated: bool,
    /// La búsqueda se canceló antes de terminar (los resultados son parciales).
    pub cancelled: bool,
}

fn is_searchable(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        e.eq_ignore_ascii_case("txt")
            || MARKDOWN_EXTENSIONS
                .iter()
                .any(|m| m.eq_ignore_ascii_case(e))
    })
}

/// Recorta `line` a [`PREVIEW_CHARS`] caracteres sin partir un carácter.
fn trim_preview(line: &str) -> String {
    let line = line.trim_end_matches('\r');
    match line.char_indices().nth(PREVIEW_CHARS) {
        Some((end, _)) => format!("{}…", &line[..end]),
        None => line.to_owned(),
    }
}

/// Busca `query` en los archivos Markdown y de texto de `root`, respetando `.gitignore` y
/// omitiendo ocultos, binarios (no UTF-8) y archivos enormes. Comprueba `cancel` entre archivos
/// y entre líneas.
///
/// # Errors
/// [`CoreError::InvalidPattern`] si el patrón regex no es válido.
pub fn search_workspace(
    root: &Path,
    query: &SearchQuery,
    cancel: &AtomicBool,
) -> Result<WorkspaceSearch, CoreError> {
    let mut result = WorkspaceSearch::default();
    let Some(re) = query.compile()? else {
        return Ok(result);
    };
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .follow_links(false)
        .build();

    'files: for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !entry.file_type().is_some_and(|t| t.is_file()) || !is_searchable(path) {
            continue;
        }
        if entry.metadata().is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        result.files_searched += 1;
        for (line_no, line) in text.split('\n').enumerate() {
            if line_no % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                break 'files;
            }
            for m in re.find_iter(line).filter(|m| !m.is_empty()) {
                if result.matches.len() >= MAX_WORKSPACE_MATCHES {
                    result.truncated = true;
                    break 'files;
                }
                result.matches.push(SearchMatch {
                    path: path.to_path_buf(),
                    line: line_no,
                    col_range: m.range(),
                    preview: trim_preview(line),
                });
            }
        }
    }
    result.matches.sort_by(|a, b| {
        (&a.path, a.line, a.col_range.start).cmp(&(&b.path, b.line, b.col_range.start))
    });
    Ok(result)
}

/// Versión asíncrona de [`search_workspace`] (corre fuera del hilo de UI).
///
/// # Errors
/// Ver [`search_workspace`].
pub async fn search_workspace_async(
    root: PathBuf,
    query: SearchQuery,
    cancel: Arc<AtomicBool>,
) -> Result<WorkspaceSearch, CoreError> {
    tokio::task::spawn_blocking(move || search_workspace(&root, &query, &cancel))
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn q(pattern: &str) -> SearchQuery {
        SearchQuery {
            pattern: pattern.into(),
            ..SearchQuery::default()
        }
    }

    fn texts<'a>(text: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
        ranges.iter().map(|r| &text[r.clone()]).collect()
    }

    #[test]
    fn plain_search_is_case_insensitive_by_default_and_literal() {
        let text = "Hola hola HOLA a.b axb";
        let m = find_all(text, &q("hola")).unwrap();
        assert_eq!(texts(text, &m), ["Hola", "hola", "HOLA"]);
        // `.` es literal fuera del modo regex.
        let m = find_all(text, &q("a.b")).unwrap();
        assert_eq!(texts(text, &m), ["a.b"]);
    }

    #[test]
    fn options_case_sensitive_whole_word_and_regex() {
        // AC2
        let text = "casa Casa casas encasar";
        let cs = SearchQuery {
            case_sensitive: true,
            ..q("casa")
        };
        assert_eq!(
            find_all(text, &cs).unwrap().len(),
            3,
            "casa, casas, encasar (no Casa)"
        );
        let word = SearchQuery {
            whole_word: true,
            ..q("casa")
        };
        assert_eq!(
            texts(text, &find_all(text, &word).unwrap()),
            ["casa", "Casa"]
        );
        let re = SearchQuery {
            regex: true,
            ..q(r"cas\w+")
        };
        assert_eq!(
            texts(text, &find_all(text, &re).unwrap()),
            ["casa", "Casa", "casas", "casar"]
        );
        let both = SearchQuery {
            regex: true,
            whole_word: true,
            case_sensitive: true,
            ..q("casa|Casa")
        };
        assert_eq!(
            texts(text, &find_all(text, &both).unwrap()),
            ["casa", "Casa"]
        );
    }

    #[test]
    fn empty_pattern_matches_nothing_and_invalid_regex_is_a_clear_error() {
        assert_eq!(find_all("abc", &q("")).unwrap(), Vec::<Range<usize>>::new());
        let bad = SearchQuery {
            regex: true,
            ..q("(abc")
        };
        let err = find_all("abc", &bad).unwrap_err();
        assert!(matches!(err, CoreError::InvalidPattern(m) if !m.is_empty()));
    }

    #[test]
    fn matches_are_byte_ranges_valid_for_multibyte_text() {
        let text = "ñandú y ñu";
        let m = find_all(text, &q("ñ")).unwrap();
        assert_eq!(texts(text, &m), ["ñ", "ñ"]);
        assert_eq!(m[0], 0..2);
    }

    #[test]
    fn replace_all_literal_does_not_expand_dollar_signs() {
        let (out, n) = replace_all("a1 a2 a3", &q("a"), "$1").unwrap();
        assert_eq!((out.as_str(), n), ("$11 $12 $13", 3));
    }

    #[test]
    fn replace_all_regex_expands_captures() {
        let re = SearchQuery {
            regex: true,
            ..q(r"(\w+)@(\w+)")
        };
        let (out, n) = replace_all("ana@casa y luis@mar", &re, "$2:$1").unwrap();
        assert_eq!((out.as_str(), n), ("casa:ana y mar:luis", 2));
        assert_eq!(
            replace_all("sin nada", &re, "x").unwrap(),
            ("sin nada".to_owned(), 0)
        );
    }

    #[test]
    fn replacement_for_one_match_expands_only_in_regex_mode() {
        let text = "ana@casa y luis@mar";
        let re = SearchQuery {
            regex: true,
            ..q(r"(\w+)@(\w+)")
        };
        let ranges = find_all(text, &re).unwrap();
        assert_eq!(
            replacement_for(text, &re, &ranges[1], "$2:$1").unwrap(),
            "mar:luis"
        );
        let lit = q("ana");
        let r = find_all(text, &lit).unwrap();
        assert_eq!(replacement_for(text, &lit, &r[0], "$1").unwrap(), "$1");
    }

    #[test]
    fn empty_matches_are_ignored() {
        let re = SearchQuery {
            regex: true,
            ..q("x*")
        };
        assert_eq!(find_all("abc", &re).unwrap(), Vec::<Range<usize>>::new());
        assert_eq!(replace_all("abc", &re, "-").unwrap().1, 0);
    }

    fn write(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    fn names(r: &WorkspaceSearch, root: &Path) -> Vec<String> {
        r.matches
            .iter()
            .map(|m| {
                format!(
                    "{}:{}",
                    m.path.strip_prefix(root).unwrap().display(),
                    m.line
                )
            })
            .collect()
    }

    #[test]
    fn workspace_search_groups_by_file_with_lines_and_respects_gitignore() {
        // AC3
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".gitignore", "ignorado.md\nbuild/\n");
        write(dir.path(), "b.md", "uno\ntema aquí\ntema otra vez\n");
        write(dir.path(), "a.md", "tema primero\n");
        write(dir.path(), "sub/c.txt", "nada\nel tema\n");
        write(dir.path(), "ignorado.md", "tema\n");
        write(dir.path(), "build/x.md", "tema\n");
        write(dir.path(), ".oculto.md", "tema\n");
        write(dir.path(), "foto.png", "tema");
        let r = search_workspace(dir.path(), &q("tema"), &AtomicBool::new(false)).unwrap();
        assert_eq!(
            names(&r, dir.path()),
            ["a.md:0", "b.md:1", "b.md:2", "sub/c.txt:1"]
        );
        assert_eq!(r.files_searched, 3);
        assert!(!r.truncated && !r.cancelled);
        assert_eq!(r.matches[1].preview, "tema aquí");
        assert_eq!(r.matches[0].col_range, 0..4);
    }

    #[test]
    fn workspace_search_skips_non_utf8_and_honours_options() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("bin.md"), b"\xFF\xFEtema").unwrap();
        write(dir.path(), "t.md", "Tema\ntemas\n");
        let word = SearchQuery {
            whole_word: true,
            case_sensitive: true,
            ..q("Tema")
        };
        let r = search_workspace(dir.path(), &word, &AtomicBool::new(false)).unwrap();
        assert_eq!(names(&r, dir.path()), ["t.md:0"]);
    }

    #[test]
    fn workspace_search_can_be_cancelled() {
        // AC3: cancelable.
        let dir = TempDir::new().unwrap();
        for i in 0..50 {
            write(dir.path(), &format!("n{i}.md"), "tema\n");
        }
        let r = search_workspace(dir.path(), &q("tema"), &AtomicBool::new(true)).unwrap();
        assert!(r.cancelled);
        assert!(r.matches.len() < 50);
    }

    #[test]
    fn workspace_search_truncates_huge_result_sets() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "grande.md",
            &"x\n".repeat(MAX_WORKSPACE_MATCHES + 100),
        );
        let r = search_workspace(dir.path(), &q("x"), &AtomicBool::new(false)).unwrap();
        assert_eq!(r.matches.len(), MAX_WORKSPACE_MATCHES);
        assert!(r.truncated);
    }

    #[test]
    fn preview_is_trimmed_without_splitting_characters() {
        let dir = TempDir::new().unwrap();
        let line = format!("tema {}", "ñ".repeat(500));
        write(dir.path(), "l.md", &line);
        let r = search_workspace(dir.path(), &q("tema"), &AtomicBool::new(false)).unwrap();
        let p = &r.matches[0].preview;
        assert_eq!(p.chars().count(), PREVIEW_CHARS + 1);
        assert!(p.ends_with('…'));
    }

    #[test]
    fn workspace_search_reports_invalid_patterns_and_ignores_empty_ones() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "a.md", "x");
        let bad = SearchQuery {
            regex: true,
            ..q("[")
        };
        assert!(search_workspace(dir.path(), &bad, &AtomicBool::new(false)).is_err());
        let r = search_workspace(dir.path(), &q(""), &AtomicBool::new(false)).unwrap();
        assert_eq!(r, WorkspaceSearch::default());
    }

    #[tokio::test]
    async fn async_wrapper_returns_the_same_results() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "a.md", "tema\n");
        let r = search_workspace_async(
            dir.path().to_path_buf(),
            q("tema"),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        assert_eq!(r.matches.len(), 1);
    }
}
