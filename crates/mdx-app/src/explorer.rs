//! Vista del explorador de archivos (árbol perezoso del workspace).

use iced::widget::{Column, button, checkbox, container, mouse_area, scrollable, text};
use iced::{Element, Length};
use mdx_core::workspace::{Children, FileNode, NodeKind, TreeFilters, Workspace};

use crate::app::{FilterKind, Message};
use crate::strings;

const INDENT: u16 = 14;

/// Estado del panel del explorador.
pub struct ExplorerState {
    pub workspace: Option<Workspace>,
    pub filters: TreeFilters,
    /// Menú de filtros abierto (clic derecho en el panel).
    pub menu_open: bool,
}

/// Árbol completo del explorador, con el menú de filtros (clic derecho en el panel).
pub fn view<'a>(
    workspace: Option<&'a Workspace>,
    filters: TreeFilters,
    menu_open: bool,
) -> Element<'a, Message> {
    let body: Element<'a, Message> = match workspace {
        None => text(strings::EXPLORER_EMPTY).into(),
        Some(ws) => {
            let mut column = Column::new().spacing(2);
            column = column.push(text(&ws.tree.name).size(16));
            if menu_open {
                column = column.push(filter_menu(filters));
            }
            let mut rows = Column::new().spacing(1);
            if let NodeKind::Dir { children, .. } = &ws.tree.kind {
                rows = children_rows(rows, children, 0);
            }
            column
                .push(scrollable(rows).width(Length::Fill).height(Length::Fill))
                .into()
        }
    };
    mouse_area(container(body).width(Length::Fill).height(Length::Fill))
        .on_right_press(Message::ToggleExplorerMenu)
        .into()
}

fn filter_menu<'a>(filters: TreeFilters) -> Element<'a, Message> {
    container(
        Column::new()
            .spacing(4)
            .push(
                checkbox(filters.show_hidden)
                    .label(strings::FILTER_HIDDEN)
                    .on_toggle(|_| Message::ToggleFilter(FilterKind::Hidden)),
            )
            .push(
                checkbox(filters.respect_gitignore)
                    .label(strings::FILTER_GITIGNORE)
                    .on_toggle(|_| Message::ToggleFilter(FilterKind::Gitignore)),
            )
            .push(
                checkbox(filters.only_markdown)
                    .label(strings::FILTER_ONLY_MARKDOWN)
                    .on_toggle(|_| Message::ToggleFilter(FilterKind::OnlyMarkdown)),
            ),
    )
    .padding(8)
    .style(container::bordered_box)
    .into()
}

fn children_rows<'a>(
    mut rows: Column<'a, Message>,
    children: &'a Children,
    depth: u16,
) -> Column<'a, Message> {
    match children {
        Children::NotLoaded => rows,
        Children::Loading => rows.push(indented(text(strings::EXPLORER_LOADING).into(), depth)),
        Children::Error(msg) => rows.push(indented(text(msg).style(text::danger).into(), depth)),
        Children::Loaded(nodes) if nodes.is_empty() => {
            rows.push(indented(text(strings::EXPLORER_EMPTY_DIR).into(), depth))
        }
        Children::Loaded(nodes) => {
            for node in nodes {
                rows = node_rows(rows, node, depth);
            }
            rows
        }
    }
}

fn node_rows<'a>(rows: Column<'a, Message>, node: &'a FileNode, depth: u16) -> Column<'a, Message> {
    match &node.kind {
        NodeKind::File { .. } => rows.push(indented(text(&node.name).into(), depth)),
        NodeKind::Dir { children, expanded } => {
            let arrow = if *expanded { "▾" } else { "▸" };
            let label = button(text(format!("{arrow} {}", node.name)))
                .padding(0)
                .style(button::text)
                .on_press(Message::ToggleDir(node.path.clone()));
            let rows = rows.push(indented(label.into(), depth));
            if *expanded {
                children_rows(rows, children, depth + 1)
            } else {
                rows
            }
        }
    }
}

fn indented(content: Element<'_, Message>, depth: u16) -> Element<'_, Message> {
    container(content)
        .padding(iced::Padding::ZERO.left(f32::from(depth * INDENT)))
        .into()
}
