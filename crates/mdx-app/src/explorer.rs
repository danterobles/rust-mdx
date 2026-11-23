//! Vista y navegación por teclado del explorador de archivos (árbol perezoso del workspace).

use std::path::{Path, PathBuf};

use iced::keyboard::{Key, Modifiers, key::Named};
use iced::widget::{
    Column, Id, Row, button, checkbox, container, mouse_area, scrollable, text, text_input,
};
use iced::{Element, Length, Theme};
use mdx_core::workspace::{Children, FileNode, NodeKind, TreeFilters, VisibleNode, Workspace};

use crate::app::{FilterKind, Message};
use crate::strings;

const INDENT: u16 = 14;

/// Estado del panel del explorador.
pub struct ExplorerState {
    pub workspace: Option<Workspace>,
    pub filters: TreeFilters,
    /// Menú de filtros abierto (clic derecho en el panel).
    pub menu_open: bool,
    /// Fila con el foco del teclado (distinta del archivo abierto en el editor).
    pub selected: Option<PathBuf>,
    /// Menú contextual abierto sobre una fila (clic derecho).
    pub context: Option<ContextTarget>,
    /// Entrada de nombre en línea (nuevo archivo/carpeta, renombrar).
    pub prompt: Option<Prompt>,
}

/// Elemento sobre el que se abrió el menú contextual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextTarget {
    pub path: PathBuf,
    pub is_dir: bool,
    /// La carpeta raíz del workspace (no se puede renombrar ni eliminar).
    pub is_root: bool,
}

/// Acciones del menú contextual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    NewFile,
    NewFolder,
    Rename,
    Delete,
    Reveal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    NewFile,
    NewFolder,
    Rename,
}

/// Entrada de nombre en línea con su error de validación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub kind: PromptKind,
    /// Carpeta destino (nuevo archivo/carpeta) o ruta a renombrar.
    pub target: PathBuf,
    pub value: String,
    pub error: Option<String>,
}

/// Identificador del campo de nombre (para enfocarlo al abrirlo).
pub const PROMPT_ID: &str = "explorer-prompt";

/// Teclas de navegación del árbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorerKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
}

/// Traduce una tecla a navegación del explorador (sin modificadores).
#[must_use]
pub fn map_key(key: &Key, modifiers: Modifiers) -> Option<ExplorerKey> {
    if !modifiers.is_empty() {
        return None;
    }
    match key {
        Key::Named(Named::ArrowUp) => Some(ExplorerKey::Up),
        Key::Named(Named::ArrowDown) => Some(ExplorerKey::Down),
        Key::Named(Named::ArrowLeft) => Some(ExplorerKey::Left),
        Key::Named(Named::ArrowRight) => Some(ExplorerKey::Right),
        Key::Named(Named::Enter) => Some(ExplorerKey::Enter),
        _ => None,
    }
}

/// Resultado de interpretar una tecla sobre el árbol visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nav {
    None,
    Select(PathBuf),
    /// Expandir o colapsar una carpeta.
    Toggle(PathBuf),
    /// Abrir un archivo en el editor.
    Open(PathBuf),
}

/// Lógica pura de navegación por teclado (US-2.2 AC2).
#[must_use]
pub fn navigate(visible: &[VisibleNode], selected: Option<&Path>, key: ExplorerKey) -> Nav {
    if visible.is_empty() {
        return Nav::None;
    }
    let index = selected.and_then(|s| visible.iter().position(|n| n.path == s));
    let select = |i: usize| Nav::Select(visible[i].path.clone());
    let Some(i) = index else {
        // Sin selección: ↓ va a la primera fila, ↑ a la última; el resto no hace nada.
        return match key {
            ExplorerKey::Down => select(0),
            ExplorerKey::Up => select(visible.len() - 1),
            _ => Nav::None,
        };
    };
    let node = &visible[i];
    match key {
        ExplorerKey::Down => select((i + 1).min(visible.len() - 1)),
        ExplorerKey::Up => select(i.saturating_sub(1)),
        ExplorerKey::Right => {
            if node.is_dir && !node.expanded {
                Nav::Toggle(node.path.clone())
            } else if node.is_dir
                && visible
                    .get(i + 1)
                    .is_some_and(|n| n.depth == node.depth + 1)
            {
                select(i + 1)
            } else {
                Nav::None
            }
        }
        ExplorerKey::Left => {
            if node.is_dir && node.expanded {
                Nav::Toggle(node.path.clone())
            } else if node.depth > 0 {
                // Sube a la carpeta padre: la fila anterior con un nivel menos.
                visible[..i]
                    .iter()
                    .rposition(|n| n.depth + 1 == node.depth)
                    .map_or(Nav::None, select)
            } else {
                Nav::None
            }
        }
        ExplorerKey::Enter => {
            if node.is_dir {
                Nav::Toggle(node.path.clone())
            } else {
                Nav::Open(node.path.clone())
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    None,
    Selected,
    /// Archivo abierto en el editor.
    Active,
}

struct Ctx<'a> {
    selected: Option<&'a Path>,
    active: Option<&'a Path>,
}

impl Ctx<'_> {
    fn mark(&self, path: &Path) -> Mark {
        if self.active == Some(path) {
            Mark::Active
        } else if self.selected == Some(path) {
            Mark::Selected
        } else {
            Mark::None
        }
    }
}

/// Árbol completo del explorador, con el menú de filtros (clic derecho en el panel).
/// `active` es el archivo abierto en el editor (se resalta, US-2.2 AC3).
pub fn view<'a>(state: &'a ExplorerState, active: Option<&'a Path>) -> Element<'a, Message> {
    let body: Element<'a, Message> = match &state.workspace {
        None => text(strings::EXPLORER_EMPTY).into(),
        Some(ws) => {
            let ctx = Ctx {
                selected: state.selected.as_deref(),
                active,
            };
            let mut column = Column::new().spacing(2);
            let root_target = ContextTarget {
                path: ws.root.clone(),
                is_dir: true,
                is_root: true,
            };
            column = column.push(
                mouse_area(text(&ws.tree.name).size(16))
                    .on_right_press(Message::OpenContext(root_target)),
            );
            if let Some(target) = &state.context {
                column = column.push(context_menu(target));
            }
            if let Some(prompt) = &state.prompt {
                column = column.push(prompt_view(prompt));
            }
            if state.menu_open {
                column = column.push(filter_menu(state.filters));
            }
            let mut rows = Column::new().spacing(1);
            if let NodeKind::Dir { children, .. } = &ws.tree.kind {
                rows = children_rows(rows, children, 0, &ctx);
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

fn context_menu(target: &ContextTarget) -> Element<'_, Message> {
    let item = |label: &'static str, action: ContextAction| {
        button(text(label))
            .padding([2, 6])
            .style(button::text)
            .on_press(Message::ContextAction(action))
    };
    let name = target.path.file_name().map_or_else(
        || target.path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mut menu = Column::new().spacing(2).push(text(name).size(13));
    menu = menu
        .push(item(strings::CTX_NEW_FILE, ContextAction::NewFile))
        .push(item(strings::CTX_NEW_FOLDER, ContextAction::NewFolder));
    if !target.is_root {
        menu = menu
            .push(item(strings::CTX_RENAME, ContextAction::Rename))
            .push(item(strings::CTX_DELETE, ContextAction::Delete));
    }
    menu = menu
        .push(item(strings::CTX_REVEAL, ContextAction::Reveal))
        .push(
            button(text(strings::CTX_CLOSE).size(12))
                .padding([2, 6])
                .style(button::secondary)
                .on_press(Message::CloseContext),
        );
    container(menu)
        .padding(8)
        .style(container::bordered_box)
        .into()
}

fn prompt_view(prompt: &Prompt) -> Element<'_, Message> {
    let label = match prompt.kind {
        PromptKind::NewFile => strings::PROMPT_NEW_FILE,
        PromptKind::NewFolder => strings::PROMPT_NEW_FOLDER,
        PromptKind::Rename => strings::PROMPT_RENAME,
    };
    let mut column = Column::new().spacing(4).push(text(label).size(13)).push(
        text_input("", &prompt.value)
            .id(Id::new(PROMPT_ID))
            .on_input(Message::PromptChanged)
            .on_submit(Message::PromptSubmit),
    );
    if let Some(err) = &prompt.error {
        column = column.push(text(err).size(13).style(text::danger));
    }
    column = column.push(
        Row::new()
            .spacing(6)
            .push(button(text(strings::PROMPT_ACCEPT)).on_press(Message::PromptSubmit))
            .push(
                button(text(strings::PROMPT_CANCEL))
                    .style(button::secondary)
                    .on_press(Message::PromptCancel),
            ),
    );
    container(column)
        .padding(8)
        .style(container::bordered_box)
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
    ctx: &Ctx<'_>,
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
                rows = node_rows(rows, node, depth, ctx);
            }
            rows
        }
    }
}

fn node_rows<'a>(
    rows: Column<'a, Message>,
    node: &'a FileNode,
    depth: u16,
    ctx: &Ctx<'_>,
) -> Column<'a, Message> {
    let mark = ctx.mark(&node.path);
    match &node.kind {
        NodeKind::File { .. } => {
            // Clic simple abre (preview); doble clic fija. Se usa `mouse_area` y no `button`
            // porque el botón captura el clic y `mouse_area` no recibiría el doble clic.
            let row = mouse_area(highlighted(text(&node.name).into(), mark))
                .on_press(Message::FileClicked(node.path.clone()))
                .on_double_click(Message::FilePinned(node.path.clone()))
                .on_right_press(Message::OpenContext(ContextTarget {
                    path: node.path.clone(),
                    is_dir: false,
                    is_root: false,
                }));
            rows.push(indented(row.into(), depth))
        }
        NodeKind::Dir { children, expanded } => {
            let arrow = if *expanded { "▾" } else { "▸" };
            let label = button(text(format!("{arrow} {}", node.name)))
                .padding(0)
                .style(button::text)
                .on_press(Message::ToggleDir(node.path.clone()));
            let row = mouse_area(highlighted(label.into(), mark)).on_right_press(
                Message::OpenContext(ContextTarget {
                    path: node.path.clone(),
                    is_dir: true,
                    is_root: false,
                }),
            );
            let rows = rows.push(indented(row.into(), depth));
            if *expanded {
                children_rows(rows, children, depth + 1, ctx)
            } else {
                rows
            }
        }
    }
}

fn highlighted(content: Element<'_, Message>, mark: Mark) -> Element<'_, Message> {
    container(content)
        .width(Length::Fill)
        .padding([1, 4])
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            match mark {
                Mark::None => container::Style::default(),
                Mark::Selected => container::Style::default()
                    .background(palette.background.strong.color)
                    .border(iced::border::rounded(3)),
                Mark::Active => container::Style::default()
                    .background(palette.primary.weak.color)
                    .color(palette.primary.weak.text)
                    .border(iced::border::rounded(3)),
            }
        })
        .into()
}

fn indented(content: Element<'_, Message>, depth: u16) -> Element<'_, Message> {
    container(content)
        .padding(iced::Padding::ZERO.left(f32::from(depth * INDENT)))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, depth: usize, is_dir: bool, expanded: bool) -> VisibleNode {
        VisibleNode {
            path: PathBuf::from(path),
            depth,
            is_dir,
            expanded,
        }
    }

    /// /a (expandida)  ├ /a/b (colapsada)  └ /a/uno.md ;  /z.md
    fn tree() -> Vec<VisibleNode> {
        vec![
            node("/a", 0, true, true),
            node("/a/b", 1, true, false),
            node("/a/uno.md", 1, false, false),
            node("/z.md", 0, false, false),
        ]
    }

    fn nav(sel: Option<&str>, key: ExplorerKey) -> Nav {
        navigate(&tree(), sel.map(Path::new), key)
    }

    fn sel(p: &str) -> Nav {
        Nav::Select(PathBuf::from(p))
    }

    #[test]
    fn up_down_move_and_clamp() {
        assert_eq!(nav(None, ExplorerKey::Down), sel("/a"));
        assert_eq!(nav(None, ExplorerKey::Up), sel("/z.md"));
        assert_eq!(nav(Some("/a"), ExplorerKey::Down), sel("/a/b"));
        assert_eq!(nav(Some("/z.md"), ExplorerKey::Down), sel("/z.md"));
        assert_eq!(nav(Some("/a"), ExplorerKey::Up), sel("/a"));
        assert_eq!(nav(Some("/z.md"), ExplorerKey::Up), sel("/a/uno.md"));
    }

    #[test]
    fn right_expands_then_enters_children() {
        assert_eq!(
            nav(Some("/a/b"), ExplorerKey::Right),
            Nav::Toggle(PathBuf::from("/a/b"))
        );
        assert_eq!(nav(Some("/a"), ExplorerKey::Right), sel("/a/b"));
        assert_eq!(nav(Some("/z.md"), ExplorerKey::Right), Nav::None);
    }

    #[test]
    fn left_collapses_then_goes_to_parent() {
        assert_eq!(
            nav(Some("/a"), ExplorerKey::Left),
            Nav::Toggle(PathBuf::from("/a"))
        );
        assert_eq!(nav(Some("/a/uno.md"), ExplorerKey::Left), sel("/a"));
        assert_eq!(nav(Some("/a/b"), ExplorerKey::Left), sel("/a"));
        assert_eq!(nav(Some("/z.md"), ExplorerKey::Left), Nav::None);
    }

    #[test]
    fn enter_opens_files_and_toggles_dirs() {
        assert_eq!(
            nav(Some("/a/uno.md"), ExplorerKey::Enter),
            Nav::Open(PathBuf::from("/a/uno.md"))
        );
        assert_eq!(
            nav(Some("/a"), ExplorerKey::Enter),
            Nav::Toggle(PathBuf::from("/a"))
        );
        assert_eq!(nav(None, ExplorerKey::Enter), Nav::None);
    }

    #[test]
    fn empty_tree_does_nothing() {
        assert_eq!(navigate(&[], None, ExplorerKey::Down), Nav::None);
    }

    #[test]
    fn maps_only_plain_navigation_keys() {
        assert_eq!(
            map_key(&Key::Named(Named::ArrowDown), Modifiers::empty()),
            Some(ExplorerKey::Down)
        );
        assert_eq!(
            map_key(&Key::Named(Named::Enter), Modifiers::empty()),
            Some(ExplorerKey::Enter)
        );
        assert_eq!(
            map_key(&Key::Named(Named::ArrowDown), Modifiers::SHIFT),
            None
        );
        assert_eq!(
            map_key(&Key::Character("a".into()), Modifiers::empty()),
            None
        );
    }
}
