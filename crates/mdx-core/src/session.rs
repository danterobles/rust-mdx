//! `session.toml`: estado que Mdx guarda al cerrar y restaura al abrir (US-3.5).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::error::CoreError;

/// Versión del esquema que entiende esta versión de Mdx.
pub const SCHEMA_VERSION: u32 = 1;
/// Máximo de carpetas y de archivos en «Recientes».
pub const MAX_RECENT: usize = 10;

/// Proporciones de los paneles respecto al ancho total de la ventana.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub explorer_ratio: f32,
    /// El editor; la vista previa ocupa el resto.
    pub editor_ratio: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            explorer_ratio: 0.20,
            editor_ratio: 0.40,
        }
    }
}

impl Layout {
    /// Acota las proporciones para que ningún panel quede inutilizable.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let finite = |v: f32, default: f32| if v.is_finite() { v } else { default };
        let explorer = finite(self.explorer_ratio, 0.20).clamp(0.05, 0.50);
        let editor = finite(self.editor_ratio, 0.40).clamp(0.10, 0.90 - explorer);
        Self {
            explorer_ratio: explorer,
            editor_ratio: editor,
        }
    }

    /// Proporción del editor dentro del espacio que queda tras el explorador (la que usa el
    /// `pane_grid` en su segundo corte).
    #[must_use]
    pub fn inner_editor_ratio(self) -> f32 {
        let l = self.sanitized();
        (l.editor_ratio / (1.0 - l.explorer_ratio)).clamp(0.05, 0.95)
    }

    /// Inversa de [`Layout::inner_editor_ratio`]: a partir de las proporciones del `pane_grid`.
    #[must_use]
    pub fn from_pane_ratios(explorer: f32, inner_editor: f32) -> Self {
        Self {
            explorer_ratio: explorer,
            editor_ratio: inner_editor * (1.0 - explorer),
        }
        .sanitized()
    }
}

/// Carpetas y archivos recientes (el más reciente primero).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recent {
    pub folders: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
}

/// Estado de la sesión.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PathBuf>,
    /// Índice en `open_files` de la pestaña activa.
    pub active_tab: usize,
    pub open_files: Vec<PathBuf>,
    pub expanded_dirs: Vec<PathBuf>,
    pub layout: Layout,
    pub recent: Recent,
    /// Zoom elegido con `Ctrl/Cmd +/-/0` (sustituye al de `config.toml` hasta que este cambie).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<f32>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            workspace: None,
            active_tab: 0,
            open_files: Vec::new(),
            expanded_dirs: Vec::new(),
            layout: Layout::default(),
            recent: Recent::default(),
            zoom: None,
        }
    }
}

/// Resultado de arrancar con una sesión: lo guardado, lo que se puede restaurar y los documentos
/// ya leídos (en el orden de las pestañas).
#[derive(Clone, Debug, Default)]
pub struct Boot {
    pub session: Session,
    pub plan: RestorePlan,
    pub documents: Vec<Document>,
    /// El archivo existía pero no era utilizable (corrupto o de una versión más nueva).
    pub unusable: bool,
}

/// Qué reabrir al restaurar una sesión, con todo lo que ya no existe descartado.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestorePlan {
    pub workspace: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub active: Option<PathBuf>,
    pub expanded_dirs: Vec<PathBuf>,
}

fn push_recent(list: &mut Vec<PathBuf>, path: &Path) -> bool {
    if list.first().is_some_and(|p| p == path) {
        return false;
    }
    list.retain(|p| p != path);
    list.insert(0, path.to_path_buf());
    list.truncate(MAX_RECENT);
    true
}

impl Session {
    /// Interpreta un `session.toml`.
    ///
    /// # Errors
    /// [`CoreError::Config`] si el TOML no es válido o es de un esquema más nuevo.
    pub fn parse(text: &str) -> Result<Self, CoreError> {
        let mut session: Session =
            toml::from_str(text).map_err(|e| CoreError::Config(e.message().to_owned()))?;
        if session.schema_version > SCHEMA_VERSION {
            return Err(CoreError::Config(format!(
                "esquema {} más nuevo que el soportado ({SCHEMA_VERSION})",
                session.schema_version
            )));
        }
        session.sanitize();
        Ok(session)
    }

    /// Serializa a TOML.
    ///
    /// # Errors
    /// [`CoreError::Config`] si no se puede serializar.
    pub fn to_toml(&self) -> Result<String, CoreError> {
        toml::to_string_pretty(self).map_err(|e| CoreError::Config(e.to_string()))
    }

    /// Acota proporciones y listas de recientes.
    pub fn sanitize(&mut self) {
        self.layout = self.layout.sanitized();
        self.zoom = self
            .zoom
            .filter(|z| z.is_finite())
            .map(|z| z.clamp(0.5, 3.0));
        for list in [&mut self.recent.folders, &mut self.recent.files] {
            let mut seen: Vec<PathBuf> = Vec::new();
            list.retain(|p| {
                let new = !seen.contains(p);
                if new {
                    seen.push(p.clone());
                }
                new
            });
            list.truncate(MAX_RECENT);
        }
    }

    /// Lee la sesión de `path`: ausente o inválida → sesión vacía (el archivo no se toca hasta
    /// el siguiente guardado normal). Devuelve también si el archivo existía pero no era usable.
    pub async fn load(path: &Path) -> (Self, bool) {
        match tokio::fs::read_to_string(path).await {
            Ok(text) => match Self::parse(&text) {
                Ok(session) => (session, false),
                Err(_) => (Self::default(), true),
            },
            Err(_) => (Self::default(), false),
        }
    }

    /// Carga la sesión de `path`, quita de «Recientes» lo que ya no existe y, con `restore`, lee
    /// los documentos que estaban abiertos. Lo que no existe o no se puede leer se omite.
    pub async fn boot(path: &Path, restore: bool) -> Boot {
        let (mut session, unusable) = Self::load(path).await;
        let snapshot = session.clone();
        let (pruned, mut plan) = tokio::task::spawn_blocking(move || {
            let mut pruned = snapshot.clone();
            pruned.prune_recent(Path::exists);
            let plan = if restore {
                snapshot.restore_plan(Path::exists)
            } else {
                RestorePlan::default()
            };
            (pruned, plan)
        })
        .await
        .unwrap_or_else(|_| (session.clone(), RestorePlan::default()));
        session.recent = pruned.recent;

        let mut documents = Vec::new();
        for file in std::mem::take(&mut plan.files) {
            if let Ok(doc) = crate::io::read_document(&file).await {
                plan.files.push(file);
                documents.push(doc);
            }
        }
        if plan
            .active
            .as_ref()
            .is_some_and(|a| !plan.files.contains(a))
        {
            plan.active = plan.files.last().cloned();
        }
        Boot {
            session,
            plan,
            documents,
            unusable,
        }
    }

    /// Guarda la sesión de forma atómica, creando el directorio si hace falta.
    ///
    /// # Errors
    /// [`CoreError`] si falla la serialización o la escritura.
    pub async fn save(&self, path: &Path) -> Result<(), CoreError> {
        let text = self.to_toml()?;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| CoreError::from_io(&e, dir))?;
        }
        crate::io::write_atomic(path, text.into_bytes()).await
    }

    /// Qué restaurar: omite en silencio todo lo que `exists` rechaza y acota `active_tab`.
    #[must_use]
    pub fn restore_plan(&self, exists: impl Fn(&Path) -> bool) -> RestorePlan {
        let active_path = self.open_files.get(self.active_tab);
        let files: Vec<PathBuf> = self
            .open_files
            .iter()
            .filter(|p| exists(p))
            .cloned()
            .collect();
        let active = active_path
            .filter(|p| files.contains(p))
            .or_else(|| files.last())
            .cloned();
        RestorePlan {
            workspace: self.workspace.clone().filter(|p| exists(p)),
            files,
            active,
            expanded_dirs: self
                .expanded_dirs
                .iter()
                .filter(|p| exists(p))
                .cloned()
                .collect(),
        }
    }

    /// Quita de «Recientes» lo que ya no existe.
    pub fn prune_recent(&mut self, exists: impl Fn(&Path) -> bool) {
        self.recent.folders.retain(|p| exists(p));
        self.recent.files.retain(|p| exists(p));
    }

    /// Registra una carpeta abierta. Devuelve `true` si la lista cambió.
    pub fn push_recent_folder(&mut self, path: &Path) -> bool {
        push_recent(&mut self.recent.folders, path)
    }

    /// Registra un archivo abierto. Devuelve `true` si la lista cambió.
    pub fn push_recent_file(&mut self, path: &Path) -> bool {
        push_recent(&mut self.recent.files, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn sample() -> Session {
        Session {
            workspace: Some(p("/w")),
            active_tab: 1,
            open_files: vec![p("/w/a.md"), p("/w/b.md")],
            expanded_dirs: vec![p("/w/docs")],
            layout: Layout {
                explorer_ratio: 0.25,
                editor_ratio: 0.35,
            },
            recent: Recent {
                folders: vec![p("/w")],
                files: vec![p("/w/b.md")],
            },
            ..Session::default()
        }
    }

    #[test]
    fn toml_roundtrip_preserves_everything() {
        let s = sample();
        let text = s.to_toml().unwrap();
        assert!(text.contains("schema_version = 1"));
        assert_eq!(Session::parse(&text).unwrap(), s);
    }

    #[test]
    fn zoom_roundtrips_and_is_clamped() {
        let with_zoom = Session {
            zoom: Some(1.5),
            ..Session::default()
        };
        let text = with_zoom.to_toml().unwrap();
        assert!(text.contains("zoom = 1.5"));
        assert_eq!(Session::parse(&text).unwrap().zoom, Some(1.5));
        assert_eq!(Session::parse("zoom = 99.0\n").unwrap().zoom, Some(3.0));
        assert_eq!(Session::parse("zoom = 0.01\n").unwrap().zoom, Some(0.5));
        assert_eq!(
            Session::parse("").unwrap().zoom,
            None,
            "sesiones antiguas sin zoom"
        );
        assert!(!Session::default().to_toml().unwrap().contains("zoom"));
    }

    #[test]
    fn empty_and_partial_files_use_defaults() {
        assert_eq!(Session::parse("").unwrap(), Session::default());
        let s = Session::parse("active_tab = 2\n").unwrap();
        assert_eq!(s.active_tab, 2);
        assert!(s.open_files.is_empty());
    }

    #[test]
    fn newer_schema_and_invalid_toml_are_rejected() {
        assert!(Session::parse("schema_version = 9\n").is_err());
        assert!(Session::parse("open_files = [").is_err());
    }

    #[test]
    fn missing_paths_skipped() {
        // INV-5
        let s = sample();
        let plan =
            s.restore_plan(|path| path != Path::new("/w/a.md") && path != Path::new("/w/docs"));
        assert_eq!(plan.files, [p("/w/b.md")]);
        assert_eq!(plan.active, Some(p("/w/b.md")));
        assert_eq!(plan.workspace, Some(p("/w")));
        assert!(plan.expanded_dirs.is_empty());
        let nothing = s.restore_plan(|_| false);
        assert_eq!(nothing, RestorePlan::default());
    }

    #[test]
    fn active_tab_is_clamped_when_out_of_range_or_missing() {
        let mut s = sample();
        s.active_tab = 99;
        let plan = s.restore_plan(|_| true);
        assert_eq!(plan.active, Some(p("/w/b.md")), "se acota a la última");
        s.active_tab = 0;
        let plan = s.restore_plan(|path| path != Path::new("/w/a.md"));
        assert_eq!(plan.active, Some(p("/w/b.md")), "la activa ya no existe");
    }

    #[test]
    fn recents_are_most_recent_first_deduplicated_and_capped() {
        let mut s = Session::default();
        for i in 0..15 {
            assert!(s.push_recent_file(&p(&format!("/f{i}.md"))));
        }
        assert_eq!(s.recent.files.len(), MAX_RECENT);
        assert_eq!(s.recent.files[0], p("/f14.md"));
        // Reabrir uno existente lo sube al principio sin duplicarlo.
        assert!(s.push_recent_file(&p("/f10.md")));
        assert_eq!(s.recent.files[0], p("/f10.md"));
        assert_eq!(
            s.recent
                .files
                .iter()
                .filter(|f| **f == p("/f10.md"))
                .count(),
            1
        );
        // Repetir el primero no cambia nada.
        assert!(!s.push_recent_file(&p("/f10.md")));
        assert!(s.push_recent_folder(&p("/carpeta")));
        assert_eq!(s.recent.folders, [p("/carpeta")]);
    }

    #[test]
    fn parsing_sanitizes_duplicate_and_oversized_recents() {
        let many: Vec<String> = (0..30).map(|i| format!("\"/f{}.md\"", i % 12)).collect();
        let text = format!("[recent]\nfiles = [{}]\n", many.join(", "));
        let s = Session::parse(&text).unwrap();
        assert_eq!(s.recent.files.len(), MAX_RECENT);
        let unique: std::collections::HashSet<_> = s.recent.files.iter().collect();
        assert_eq!(unique.len(), s.recent.files.len());
    }

    #[test]
    fn prune_recent_drops_vanished_entries() {
        let mut s = sample();
        s.prune_recent(|path| path != Path::new("/w/b.md"));
        assert!(s.recent.files.is_empty());
        assert_eq!(s.recent.folders, [p("/w")]);
    }

    #[test]
    fn layout_is_clamped_and_converts_to_and_from_pane_ratios() {
        let wild = Layout {
            explorer_ratio: 0.9,
            editor_ratio: 5.0,
        }
        .sanitized();
        assert!(wild.explorer_ratio <= 0.5 && wild.editor_ratio <= 0.9 - wild.explorer_ratio);
        let nan = Layout {
            explorer_ratio: f32::NAN,
            editor_ratio: f32::INFINITY,
        }
        .sanitized();
        assert!(nan.explorer_ratio.is_finite() && nan.editor_ratio.is_finite());
        let l = Layout::default();
        assert!(
            (l.inner_editor_ratio() - 0.5).abs() < 1e-6,
            "20/40/40 → 0.5 interno"
        );
        let back = Layout::from_pane_ratios(0.2, 0.5);
        assert!((back.explorer_ratio - 0.2).abs() < 1e-6 && (back.editor_ratio - 0.4).abs() < 1e-6);
    }

    #[tokio::test]
    async fn save_and_load_roundtrip_on_disk_creating_the_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("estado/mdx/session.toml");
        let s = sample();
        s.save(&path).await.unwrap();
        let (loaded, unusable) = Session::load(&path).await;
        assert_eq!(loaded, s);
        assert!(!unusable);
    }

    #[tokio::test]
    async fn load_of_missing_or_corrupt_file_gives_an_empty_session() {
        let dir = TempDir::new().unwrap();
        let (missing, unusable) = Session::load(&dir.path().join("no.toml")).await;
        assert_eq!(missing, Session::default());
        assert!(!unusable);
        let bad = dir.path().join("mala.toml");
        std::fs::write(&bad, "[[[").unwrap();
        let (session, unusable) = Session::load(&bad).await;
        assert_eq!(session, Session::default());
        assert!(unusable, "se avisa de que existía pero no era usable");
    }

    #[tokio::test]
    async fn boot_restores_existing_files_in_order_and_skips_the_rest() {
        // AC3
        let dir = TempDir::new().unwrap();
        let (a, b, gone) = (
            dir.path().join("a.md"),
            dir.path().join("b.md"),
            dir.path().join("x.md"),
        );
        std::fs::write(&a, "A").unwrap();
        std::fs::write(&b, "B").unwrap();
        let bad = dir.path().join("bin.md");
        std::fs::write(&bad, b"\xFF\xFE").unwrap();
        let session = Session {
            workspace: Some(dir.path().to_path_buf()),
            active_tab: 3,
            open_files: vec![b.clone(), gone.clone(), bad, a.clone()],
            recent: Recent {
                folders: vec![dir.path().to_path_buf(), gone.clone()],
                files: vec![gone, a.clone()],
            },
            ..Session::default()
        };
        let file = dir.path().join("estado/session.toml");
        session.save(&file).await.unwrap();

        let boot = Session::boot(&file, true).await;
        assert_eq!(boot.plan.files, [b.clone(), a.clone()]);
        assert_eq!(
            boot.documents
                .iter()
                .map(|d| d.text.as_str())
                .collect::<Vec<_>>(),
            ["B", "A"]
        );
        assert_eq!(
            boot.plan.active,
            Some(a.clone()),
            "active_tab fuera de rango → la última"
        );
        assert_eq!(boot.plan.workspace, Some(dir.path().to_path_buf()));
        assert_eq!(boot.session.recent.folders, [dir.path().to_path_buf()]);
        assert_eq!(boot.session.recent.files, [a]);
        assert!(!boot.unusable);
    }

    #[tokio::test]
    async fn boot_without_restore_keeps_only_the_recents() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a.md");
        std::fs::write(&a, "A").unwrap();
        let session = Session {
            open_files: vec![a.clone()],
            recent: Recent {
                folders: vec![],
                files: vec![a.clone()],
            },
            ..Session::default()
        };
        let file = dir.path().join("session.toml");
        session.save(&file).await.unwrap();
        let boot = Session::boot(&file, false).await;
        assert_eq!(boot.plan, RestorePlan::default());
        assert!(boot.documents.is_empty());
        assert_eq!(boot.session.recent.files, [a]);
    }

    #[tokio::test]
    async fn boot_with_a_missing_or_corrupt_file_never_fails() {
        let dir = TempDir::new().unwrap();
        let boot = Session::boot(&dir.path().join("no.toml"), true).await;
        assert_eq!(boot.session, Session::default());
        assert!(!boot.unusable && boot.documents.is_empty());
        let bad = dir.path().join("mala.toml");
        std::fs::write(&bad, "[[[").unwrap();
        let boot = Session::boot(&bad, true).await;
        assert!(boot.unusable);
        assert_eq!(
            std::fs::read_to_string(&bad).unwrap(),
            "[[[",
            "no se sobrescribe al arrancar"
        );
    }
}
