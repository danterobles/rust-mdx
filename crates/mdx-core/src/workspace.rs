//! Árbol de archivos de una carpeta abierta, con carga perezosa por directorio.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::error::CoreError;
use crate::link::is_markdown;

/// Filtros del árbol (valores por defecto según el PRD: US-2.1 AC3/AC4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeFilters {
    pub respect_gitignore: bool,
    pub show_hidden: bool,
    pub only_markdown: bool,
}

impl Default for TreeFilters {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            show_hidden: false,
            only_markdown: true,
        }
    }
}

/// Estado de carga de los hijos de un directorio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Children {
    NotLoaded,
    Loading,
    /// Ordenados: carpetas primero, luego alfabético sin distinguir mayúsculas (INV-4).
    Loaded(Vec<FileNode>),
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    File { is_markdown: bool },
    Dir { children: Children, expanded: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: NodeKind,
}

impl FileNode {
    #[must_use]
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, NodeKind::Dir { .. })
    }
}

/// Carpeta abierta.
#[derive(Clone, Debug)]
pub struct Workspace {
    /// Ruta canónica.
    pub root: PathBuf,
    pub tree: FileNode,
    pub filters: TreeFilters,
}

impl Workspace {
    /// Abre `root` (carpeta) cargando solo su primer nivel.
    ///
    /// # Errors
    /// [`CoreError::NotFound`] / [`CoreError::PermissionDenied`] / [`CoreError::Io`] si la ruta no
    /// se puede resolver o leer, o [`CoreError::InvalidName`] si no es una carpeta.
    pub fn open(root: &Path, filters: TreeFilters) -> Result<Self, CoreError> {
        let root = root
            .canonicalize()
            .map_err(|e| CoreError::from_io(&e, root))?;
        if !root.is_dir() {
            return Err(CoreError::InvalidName(root.display().to_string()));
        }
        let children = load_children(&root, &filters)?;
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let tree = FileNode {
            path: root.clone(),
            name,
            kind: NodeKind::Dir {
                children: Children::Loaded(children),
                expanded: true,
            },
        };
        Ok(Self {
            root,
            tree,
            filters,
        })
    }

    /// Busca un nodo por ruta.
    #[must_use]
    pub fn node(&self, path: &Path) -> Option<&FileNode> {
        fn find<'a>(node: &'a FileNode, path: &Path) -> Option<&'a FileNode> {
            if node.path == path {
                return Some(node);
            }
            if let NodeKind::Dir {
                children: Children::Loaded(kids),
                ..
            } = &node.kind
                && path.starts_with(&node.path)
            {
                return kids.iter().find_map(|k| find(k, path));
            }
            None
        }
        find(&self.tree, path)
    }

    /// Versión mutable de [`Workspace::node`].
    pub fn node_mut(&mut self, path: &Path) -> Option<&mut FileNode> {
        fn find<'a>(node: &'a mut FileNode, path: &Path) -> Option<&'a mut FileNode> {
            if node.path == path {
                return Some(node);
            }
            if let NodeKind::Dir {
                children: Children::Loaded(kids),
                ..
            } = &mut node.kind
                && path.starts_with(&node.path)
            {
                return kids.iter_mut().find_map(|k| find(k, path));
            }
            None
        }
        find(&mut self.tree, path)
    }

    /// Marca el directorio como «cargando» y lo expande. Devuelve `true` si hay que cargarlo.
    pub fn begin_expand(&mut self, dir: &Path) -> bool {
        let Some(node) = self.node_mut(dir) else {
            return false;
        };
        let NodeKind::Dir { children, expanded } = &mut node.kind else {
            return false;
        };
        *expanded = true;
        if matches!(children, Children::NotLoaded | Children::Error(_)) {
            *children = Children::Loading;
            return true;
        }
        false
    }

    /// Colapsa un directorio (conserva los hijos cargados).
    pub fn collapse(&mut self, dir: &Path) {
        if let Some(FileNode {
            kind: NodeKind::Dir { expanded, .. },
            ..
        }) = self.node_mut(dir)
        {
            *expanded = false;
        }
    }

    /// Guarda el resultado de cargar los hijos de `dir`.
    pub fn set_children(&mut self, dir: &Path, result: Result<Vec<FileNode>, CoreError>) {
        if let Some(FileNode {
            kind: NodeKind::Dir { children, .. },
            ..
        }) = self.node_mut(dir)
        {
            *children = match result {
                Ok(kids) => Children::Loaded(kids),
                Err(e) => Children::Error(e.to_string()),
            };
        }
    }

    /// Reabre la raíz con otros filtros (el árbol vuelve a colapsarse).
    ///
    /// # Errors
    /// Ver [`Workspace::open`].
    pub fn with_filters(&self, filters: TreeFilters) -> Result<Self, CoreError> {
        Self::open(&self.root, filters)
    }
}

/// Orden de hijos: carpetas primero y luego alfabético sin distinguir mayúsculas (INV-4).
#[must_use]
pub fn compare_nodes(a: &FileNode, b: &FileNode) -> Ordering {
    b.is_dir()
        .cmp(&a.is_dir())
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.name.cmp(&b.name))
}

/// Lee **solo** los hijos directos de `dir` (carga perezosa), aplicando `filters`.
///
/// Con `only_markdown` se muestran todas las carpetas y solo los archivos Markdown (las carpetas
/// sin `.md` no se ocultan: evita recorrer el árbol completo).
///
/// # Errors
/// [`CoreError`] si `dir` no se puede leer.
pub fn load_children(dir: &Path, filters: &TreeFilters) -> Result<Vec<FileNode>, CoreError> {
    let walker = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(!filters.show_hidden)
        .git_ignore(filters.respect_gitignore)
        .git_global(filters.respect_gitignore)
        .git_exclude(filters.respect_gitignore)
        .ignore(filters.respect_gitignore)
        .parents(filters.respect_gitignore)
        // Respeta `.gitignore` aunque la carpeta no sea (todavía) un repositorio git.
        .require_git(false)
        .follow_links(false)
        .build();

    let mut nodes = Vec::new();
    for entry in walker {
        // Una entrada ilegible no debe impedir listar el resto.
        let Ok(entry) = entry else { continue };
        if entry.depth() == 0 {
            continue;
        }
        let path = entry.path().to_path_buf();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
        if is_dir {
            nodes.push(FileNode {
                path,
                name,
                kind: NodeKind::Dir {
                    children: Children::NotLoaded,
                    expanded: false,
                },
            });
        } else {
            let md = is_markdown(&path);
            if filters.only_markdown && !md {
                continue;
            }
            nodes.push(FileNode {
                path,
                name,
                kind: NodeKind::File { is_markdown: md },
            });
        }
    }
    if nodes.is_empty() {
        // Distingue «carpeta vacía» de «no se pudo leer».
        std::fs::read_dir(dir).map_err(|e| CoreError::from_io(&e, dir))?;
    }
    nodes.sort_by(compare_nodes);
    Ok(nodes)
}

/// Versión asíncrona de [`Workspace::open`] (no bloquea el hilo de UI).
///
/// # Errors
/// Ver [`Workspace::open`].
pub async fn open_async(root: PathBuf, filters: TreeFilters) -> Result<Workspace, CoreError> {
    tokio::task::spawn_blocking(move || Workspace::open(&root, filters))
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?
}

/// Versión asíncrona de [`load_children`].
///
/// # Errors
/// Ver [`load_children`].
pub async fn load_children_async(
    dir: PathBuf,
    filters: TreeFilters,
) -> Result<Vec<FileNode>, CoreError> {
    tokio::task::spawn_blocking(move || load_children(&dir, &filters))
        .await
        .map_err(|e| CoreError::Io(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn names(nodes: &[FileNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.name.as_str()).collect()
    }

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, "x").unwrap();
    }

    #[test]
    fn sort_order_dirs_first_then_case_insensitive() {
        // INV-4 / AC5
        let dir = TempDir::new().unwrap();
        for f in [
            "b.md",
            "A.md",
            "c.md",
            "Zeta/n.md",
            "alfa/n.md",
            "Beta/n.md",
        ] {
            touch(dir.path(), f);
        }
        let kids = load_children(dir.path(), &TreeFilters::default()).unwrap();
        assert_eq!(
            names(&kids),
            ["alfa", "Beta", "Zeta", "A.md", "b.md", "c.md"]
        );
    }

    #[test]
    fn only_markdown_filter_keeps_all_dirs() {
        // AC4
        let dir = TempDir::new().unwrap();
        for f in [
            "nota.md",
            "NOTA2.MARKDOWN",
            "foto.png",
            "texto.txt",
            "vacia/foto.png",
        ] {
            touch(dir.path(), f);
        }
        let kids = load_children(dir.path(), &TreeFilters::default()).unwrap();
        assert_eq!(names(&kids), ["vacia", "nota.md", "NOTA2.MARKDOWN"]);

        let all = TreeFilters {
            only_markdown: false,
            ..TreeFilters::default()
        };
        let kids = load_children(dir.path(), &all).unwrap();
        assert_eq!(kids.len(), 5);
    }

    #[test]
    fn hidden_files_toggle() {
        // AC3
        let dir = TempDir::new().unwrap();
        for f in [".oculto.md", ".git/config", "visible.md"] {
            touch(dir.path(), f);
        }
        let kids = load_children(dir.path(), &TreeFilters::default()).unwrap();
        assert_eq!(names(&kids), ["visible.md"]);
        let show = TreeFilters {
            show_hidden: true,
            ..TreeFilters::default()
        };
        let kids = load_children(dir.path(), &show).unwrap();
        assert_eq!(names(&kids), [".git", ".oculto.md", "visible.md"]);
    }

    #[test]
    fn gitignore_is_respected_and_togglable() {
        // AC3: incluye subdirectorios (el .gitignore de la raíz aplica a hijos cargados después).
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "ignorado.md\nbuild/\n").unwrap();
        for f in [
            "ignorado.md",
            "ok.md",
            "build/x.md",
            "src/ignorado.md",
            "src/ok.md",
        ] {
            touch(dir.path(), f);
        }
        let root = load_children(dir.path(), &TreeFilters::default()).unwrap();
        assert_eq!(names(&root), ["src", "ok.md"]);
        let src = load_children(&dir.path().join("src"), &TreeFilters::default()).unwrap();
        assert_eq!(names(&src), ["ok.md"]);

        let off = TreeFilters {
            respect_gitignore: false,
            ..TreeFilters::default()
        };
        let root = load_children(dir.path(), &off).unwrap();
        assert_eq!(names(&root), ["build", "src", "ignorado.md", "ok.md"]);
    }

    #[test]
    fn loading_is_lazy_one_level_only() {
        // AC2
        let dir = TempDir::new().unwrap();
        touch(dir.path(), "a/b/c/profundo.md");
        let ws = Workspace::open(dir.path(), TreeFilters::default()).unwrap();
        let NodeKind::Dir {
            children: Children::Loaded(kids),
            expanded,
        } = &ws.tree.kind
        else {
            panic!("la raíz debe estar cargada");
        };
        assert!(*expanded);
        assert_eq!(names(kids), ["a"]);
        let NodeKind::Dir { children, .. } = &kids[0].kind else {
            panic!("a es carpeta")
        };
        assert_eq!(*children, Children::NotLoaded);
    }

    #[test]
    fn expand_collapse_and_set_children() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), "a/uno.md");
        let mut ws = Workspace::open(dir.path(), TreeFilters::default()).unwrap();
        let a = ws.root.join("a");
        assert!(ws.begin_expand(&a), "primera expansión requiere cargar");
        assert!(
            !ws.begin_expand(&a),
            "ya está cargando: no se pide otra vez"
        );
        let kids = load_children(&a, &ws.filters).unwrap();
        ws.set_children(&a, Ok(kids));
        ws.collapse(&a);
        assert!(!ws.begin_expand(&a), "ya cargado: solo se re-expande");
        let node = ws.node(&a.join("uno.md")).unwrap();
        assert_eq!(node.name, "uno.md");
    }

    #[test]
    fn open_errors() {
        let dir = TempDir::new().unwrap();
        assert!(matches!(
            Workspace::open(&dir.path().join("no-existe"), TreeFilters::default()),
            Err(CoreError::NotFound(_))
        ));
        touch(dir.path(), "f.md");
        assert!(matches!(
            Workspace::open(&dir.path().join("f.md"), TreeFilters::default()),
            Err(CoreError::InvalidName(_))
        ));
    }

    /// AC6: `cargo test -p mdx-core -- --ignored`.
    #[test]
    #[ignore = "rendimiento"]
    fn perf_ten_thousand_files() {
        let dir = TempDir::new().unwrap();
        for i in 0..10_000 {
            fs::write(dir.path().join(format!("nota_{i:05}.md")), "x").unwrap();
        }
        for d in 0..100 {
            for i in 0..100 {
                touch(dir.path(), &format!("carpeta_{d:03}/n_{i:03}.md"));
            }
        }
        let t = std::time::Instant::now();
        let kids = load_children(dir.path(), &TreeFilters::default()).unwrap();
        let elapsed = t.elapsed();
        eprintln!("PERF load_children(10 000 archivos + 100 carpetas) = {elapsed:?}");
        assert_eq!(kids.len(), 10_100);
        assert!(elapsed.as_millis() < 1_000, "{elapsed:?}");
    }
}
