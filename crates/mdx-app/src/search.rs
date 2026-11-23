//! Vistas de búsqueda: barra «buscar / reemplazar» del documento y panel de búsqueda en carpeta.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use iced::widget::{Column, Id, Row, button, container, row, scrollable, text, text_input};
use iced::{Element, Length};
use mdx_core::search::{SearchQuery, WorkspaceSearch};

use crate::app::Message;
use crate::strings;

pub const FIND_INPUT_ID: &str = "find-input";
pub const REPLACE_INPUT_ID: &str = "replace-input";
pub const WS_INPUT_ID: &str = "workspace-search-input";

/// Opciones de búsqueda que se pueden alternar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindOption {
    CaseSensitive,
    WholeWord,
    Regex,
}

impl FindOption {
    /// Alterna la opción en `query`.
    pub fn toggle(self, query: &mut SearchQuery) {
        match self {
            Self::CaseSensitive => query.case_sensitive = !query.case_sensitive,
            Self::WholeWord => query.whole_word = !query.whole_word,
            Self::Regex => query.regex = !query.regex,
        }
    }
}

/// Estado de la barra de buscar/reemplazar del documento activo.
#[derive(Debug, Default)]
pub struct FindState {
    pub open: bool,
    pub replace_mode: bool,
    pub query: SearchQuery,
    pub replacement: String,
    /// Coincidencias como rangos de bytes en el texto del documento.
    pub matches: Vec<Range<usize>>,
    pub current: Option<usize>,
    /// Error del patrón (regex inválido).
    pub error: Option<String>,
    /// Aviso transitorio («3 reemplazos»).
    pub notice: Option<String>,
}

impl FindState {
    /// «2 de 5», «Sin resultados» o el error del patrón.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.query.pattern.is_empty() {
            return String::new();
        }
        match (self.current, self.matches.len()) {
            (_, 0) => strings::FIND_NO_RESULTS.to_owned(),
            (Some(i), n) => strings::FIND_COUNT
                .replace("{current}", &(i + 1).to_string())
                .replace("{total}", &n.to_string()),
            (None, n) => strings::FIND_COUNT
                .replace("{current}", "0")
                .replace("{total}", &n.to_string()),
        }
    }
}

/// Estado de la búsqueda en toda la carpeta.
#[derive(Debug, Default)]
pub struct WorkspaceSearchState {
    pub query: SearchQuery,
    pub running: bool,
    pub cancel: Option<Arc<AtomicBool>>,
    /// Generación de la última búsqueda lanzada (descarta respuestas viejas).
    pub generation: u64,
    pub result: Option<WorkspaceSearch>,
    pub error: Option<String>,
}

fn option_button<'a>(
    label: &'static str,
    active: bool,
    option: FindOption,
) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([2, 6])
        .style(if active {
            button::primary
        } else {
            button::secondary
        })
        .on_press(Message::FindOptionToggled(option))
        .into()
}

fn ws_option_button<'a>(
    label: &'static str,
    active: bool,
    option: FindOption,
) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([2, 6])
        .style(if active {
            button::primary
        } else {
            button::secondary
        })
        .on_press(Message::WsOptionToggled(option))
        .into()
}

/// Barra sobre el editor: campo de búsqueda, opciones, navegación y (si procede) reemplazo.
pub fn find_bar(state: &FindState) -> Element<'_, Message> {
    let q = &state.query;
    let mut first = Row::new().spacing(6).align_y(iced::Alignment::Center).push(
        text_input(strings::FIND_PLACEHOLDER, &q.pattern)
            .id(Id::new(FIND_INPUT_ID))
            .on_input(Message::FindQueryChanged)
            .on_submit(Message::FindNext)
            .size(14)
            .width(Length::FillPortion(3)),
    );
    first = first
        .push(option_button(
            strings::FIND_CASE,
            q.case_sensitive,
            FindOption::CaseSensitive,
        ))
        .push(option_button(
            strings::FIND_WORD,
            q.whole_word,
            FindOption::WholeWord,
        ))
        .push(option_button(
            strings::FIND_REGEX,
            q.regex,
            FindOption::Regex,
        ))
        .push(
            button(text(strings::FIND_PREV).size(12))
                .padding([2, 6])
                .on_press(Message::FindPrev),
        )
        .push(
            button(text(strings::FIND_NEXT).size(12))
                .padding([2, 6])
                .on_press(Message::FindNext),
        );
    let summary = match &state.error {
        Some(err) => text(err).size(12).style(text::danger),
        None => text(state.summary()).size(12),
    };
    first = first.push(summary).push(
        button(text(strings::FIND_CLOSE).size(14))
            .padding([2, 6])
            .style(button::text)
            .on_press(Message::FindClose),
    );

    let mut bar = Column::new().spacing(4).push(first);
    if state.replace_mode {
        bar = bar.push(
            row![
                text_input(strings::REPLACE_PLACEHOLDER, &state.replacement)
                    .id(Id::new(REPLACE_INPUT_ID))
                    .on_input(Message::ReplaceChanged)
                    .on_submit(Message::ReplaceOne)
                    .size(14)
                    .width(Length::FillPortion(3)),
                button(text(strings::FIND_REPLACE_ONE).size(12))
                    .padding([2, 6])
                    .on_press(Message::ReplaceOne),
                button(text(strings::FIND_REPLACE_ALL).size(12))
                    .padding([2, 6])
                    .on_press(Message::ReplaceAll),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        );
    }
    if let Some(notice) = &state.notice {
        bar = bar.push(text(notice).size(12));
    }
    container(bar)
        .padding(6)
        .width(Length::Fill)
        .style(container::bordered_box)
        .into()
}

/// Panel lateral de búsqueda en carpeta: consulta, opciones y resultados agrupados por archivo.
pub fn workspace_panel<'a>(
    state: &'a WorkspaceSearchState,
    root: Option<&'a Path>,
) -> Element<'a, Message> {
    let Some(root) = root else {
        return container(text(strings::WS_NO_FOLDER)).padding(4).into();
    };
    let q = &state.query;
    let mut column = Column::new().spacing(6).push(
        text_input(strings::WS_PLACEHOLDER, &q.pattern)
            .id(Id::new(WS_INPUT_ID))
            .on_input(Message::WsQueryChanged)
            .on_submit(Message::WsSearch)
            .size(14),
    );
    let action = if state.running {
        button(text(strings::WS_CANCEL).size(12))
            .padding([2, 6])
            .on_press(Message::WsCancel)
    } else {
        button(text(strings::WS_SEARCH).size(12))
            .padding([2, 6])
            .on_press(Message::WsSearch)
    };
    column = column.push(
        Row::new()
            .spacing(4)
            .push(ws_option_button(
                strings::FIND_CASE,
                q.case_sensitive,
                FindOption::CaseSensitive,
            ))
            .push(ws_option_button(
                strings::FIND_WORD,
                q.whole_word,
                FindOption::WholeWord,
            ))
            .push(ws_option_button(
                strings::FIND_REGEX,
                q.regex,
                FindOption::Regex,
            ))
            .push(action),
    );
    if state.running {
        column = column.push(text(strings::WS_RUNNING).size(12));
    }
    if let Some(err) = &state.error {
        column = column.push(text(err).size(12).style(text::danger));
    }
    if let Some(result) = &state.result {
        column = column.push(text(result_summary(result)).size(12));
        column = column.push(results_list(result, root));
    }
    column.into()
}

/// «12 resultados en 3 archivos» (con avisos de truncado o cancelación).
#[must_use]
pub fn result_summary(result: &WorkspaceSearch) -> String {
    let files = distinct_files(result);
    let mut s = if result.matches.is_empty() {
        strings::WS_NO_RESULTS.replace("{files}", &result.files_searched.to_string())
    } else {
        strings::WS_SUMMARY
            .replace("{matches}", &result.matches.len().to_string())
            .replace("{files}", &files.to_string())
    };
    if result.truncated {
        s.push_str(strings::WS_TRUNCATED);
    }
    if result.cancelled {
        s.push_str(strings::WS_CANCELLED);
    }
    s
}

fn distinct_files(result: &WorkspaceSearch) -> usize {
    let mut count = 0;
    let mut last: Option<&PathBuf> = None;
    for m in &result.matches {
        if last != Some(&m.path) {
            count += 1;
            last = Some(&m.path);
        }
    }
    count
}

fn results_list<'a>(result: &'a WorkspaceSearch, root: &'a Path) -> Element<'a, Message> {
    let mut list = Column::new().spacing(1);
    let mut current: Option<&PathBuf> = None;
    for m in &result.matches {
        if current != Some(&m.path) {
            current = Some(&m.path);
            let rel = m.path.strip_prefix(root).unwrap_or(&m.path);
            list = list.push(text(rel.display().to_string()).size(13));
        }
        let label = format!("{}: {}", m.line + 1, m.preview.trim());
        list = list.push(
            container(
                button(text(label).size(12))
                    .padding(0)
                    .style(button::text)
                    .on_press(Message::OpenResult(
                        m.path.clone(),
                        m.line,
                        m.col_range.clone(),
                    )),
            )
            .padding(iced::Padding::ZERO.left(10.0)),
        );
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdx_core::search::SearchMatch;

    fn m(path: &str, line: usize) -> SearchMatch {
        SearchMatch {
            path: PathBuf::from(path),
            line,
            col_range: 0..1,
            preview: "x".into(),
        }
    }

    #[test]
    fn find_summary_reports_position_and_emptiness() {
        let mut s = FindState::default();
        assert_eq!(s.summary(), "");
        s.query.pattern = "a".into();
        assert_eq!(s.summary(), "Sin resultados");
        s.matches = vec![0..1, 3..4, 8..9];
        s.current = Some(1);
        assert_eq!(s.summary(), "2 de 3");
        s.current = None;
        assert_eq!(s.summary(), "0 de 3");
    }

    #[test]
    fn options_toggle_independently() {
        let mut q = SearchQuery::default();
        FindOption::Regex.toggle(&mut q);
        FindOption::WholeWord.toggle(&mut q);
        assert!(q.regex && q.whole_word && !q.case_sensitive);
        FindOption::Regex.toggle(&mut q);
        assert!(!q.regex);
    }

    #[test]
    fn workspace_summary_counts_files_and_flags_truncation() {
        let r = WorkspaceSearch {
            matches: vec![m("/a.md", 0), m("/a.md", 4), m("/b.md", 1)],
            files_searched: 7,
            truncated: true,
            cancelled: false,
        };
        assert_eq!(
            result_summary(&r),
            "3 resultados en 2 archivos (hay más; afine la búsqueda)"
        );
        let none = WorkspaceSearch {
            files_searched: 7,
            ..WorkspaceSearch::default()
        };
        assert_eq!(result_summary(&none), "Sin resultados en 7 archivos");
        let cancelled = WorkspaceSearch {
            cancelled: true,
            ..r
        };
        assert!(result_summary(&cancelled).ends_with("(cancelada: resultados parciales)"));
    }
}
