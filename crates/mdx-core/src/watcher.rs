//! Observador del sistema de archivos (debounce de 200 ms) expuesto como `Stream`.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::Stream;
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use notify_debouncer_full::notify::event::{ModifyKind, RenameMode};
use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{
    DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache, new_debouncer,
};

use crate::error::CoreError;

/// Tiempo de agrupación de eventos (NFR/ARCHITECTURE §6).
pub const DEBOUNCE: Duration = Duration::from_millis(200);

/// Cambio observado en el sistema de archivos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

impl FsEvent {
    /// Rutas que el evento toca (para decidir qué directorios o documentos refrescar).
    #[must_use]
    pub fn paths(&self) -> Vec<&Path> {
        match self {
            Self::Created(p) | Self::Modified(p) | Self::Removed(p) => vec![p.as_path()],
            Self::Renamed { from, to } => vec![from.as_path(), to.as_path()],
        }
    }
}

/// Observador activo: mientras exista, se vigila la ruta. Se detiene al soltarlo.
pub struct FsWatcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    rx: UnboundedReceiver<Vec<FsEvent>>,
}

impl Stream for FsWatcher {
    type Item = Vec<FsEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

/// Vigila `root` (recursivo o solo ese directorio) y emite lotes de [`FsEvent`].
///
/// # Errors
/// [`CoreError::Io`] si el sistema operativo no puede iniciar la observación.
pub fn watch(root: &Path, recursive: bool) -> Result<FsWatcher, CoreError> {
    let (tx, rx) = unbounded();
    let base = root.to_path_buf();
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
        // Los errores transitorios del observador se descartan; el siguiente evento llega solo.
        if let Ok(events) = result {
            let batch = convert(&events, &base);
            if !batch.is_empty() {
                let _ = tx.unbounded_send(batch);
            }
        }
    })
    .map_err(|e| CoreError::Io(format!("{}: {e}", root.display())))?;
    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    debouncer
        .watch(root, mode)
        .map_err(|e| CoreError::Io(format!("{}: {e}", root.display())))?;
    Ok(FsWatcher {
        _debouncer: debouncer,
        rx,
    })
}

fn in_git_dir(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .any(|c| c.as_os_str() == ".git")
}

/// Traduce eventos de `notify` a [`FsEvent`]; descarta `.git` y accesos de solo lectura.
#[must_use]
pub fn convert(events: &[DebouncedEvent], root: &Path) -> Vec<FsEvent> {
    let mut out: Vec<FsEvent> = Vec::new();
    for ev in events {
        let paths: Vec<&PathBuf> = ev.paths.iter().filter(|p| !in_git_dir(p, root)).collect();
        if paths.is_empty() {
            continue;
        }
        let converted: Vec<FsEvent> = match ev.kind {
            EventKind::Create(_) => paths
                .iter()
                .map(|p| FsEvent::Created((*p).clone()))
                .collect(),
            EventKind::Remove(_) => paths
                .iter()
                .map(|p| FsEvent::Removed((*p).clone()))
                .collect(),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if ev.paths.len() >= 2 => {
                vec![FsEvent::Renamed {
                    from: ev.paths[0].clone(),
                    to: ev.paths[1].clone(),
                }]
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => paths
                .iter()
                .map(|p| FsEvent::Removed((*p).clone()))
                .collect(),
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => paths
                .iter()
                .map(|p| FsEvent::Created((*p).clone()))
                .collect(),
            // Algunos sistemas (FSEvents) no distinguen origen y destino del renombrado.
            EventKind::Modify(ModifyKind::Name(_)) => paths
                .iter()
                .map(|p| {
                    if p.exists() {
                        FsEvent::Created((*p).clone())
                    } else {
                        FsEvent::Removed((*p).clone())
                    }
                })
                .collect(),
            EventKind::Modify(ModifyKind::Metadata(_)) | EventKind::Access(_) => Vec::new(),
            EventKind::Modify(_) | EventKind::Any | EventKind::Other => paths
                .iter()
                .map(|p| FsEvent::Modified((*p).clone()))
                .collect(),
        };
        for event in converted {
            if out.last() != Some(&event) {
                out.push(event);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use notify_debouncer_full::notify::Event;
    use notify_debouncer_full::notify::event::{CreateKind, DataChange, MetadataKind, RemoveKind};
    use std::fs;
    use std::time::Instant;
    use tempfile::TempDir;

    fn ev(kind: EventKind, paths: &[&str]) -> DebouncedEvent {
        let mut e = Event::new(kind);
        for p in paths {
            e = e.add_path(PathBuf::from(p));
        }
        DebouncedEvent::new(e, Instant::now())
    }

    #[test]
    fn converts_create_modify_remove() {
        let root = Path::new("/r");
        let events = [
            ev(EventKind::Create(CreateKind::File), &["/r/a.md"]),
            ev(
                EventKind::Modify(ModifyKind::Data(DataChange::Content)),
                &["/r/a.md"],
            ),
            ev(EventKind::Remove(RemoveKind::File), &["/r/b.md"]),
        ];
        assert_eq!(
            convert(&events, root),
            [
                FsEvent::Created("/r/a.md".into()),
                FsEvent::Modified("/r/a.md".into()),
                FsEvent::Removed("/r/b.md".into()),
            ]
        );
    }

    #[test]
    fn converts_renames() {
        let root = Path::new("/r");
        let both = ev(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &["/r/a", "/r/b"],
        );
        let from = ev(
            EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            &["/r/c"],
        );
        let to = ev(
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            &["/r/d"],
        );
        assert_eq!(
            convert(&[both, from, to], root),
            [
                FsEvent::Renamed {
                    from: "/r/a".into(),
                    to: "/r/b".into()
                },
                FsEvent::Removed("/r/c".into()),
                FsEvent::Created("/r/d".into()),
            ]
        );
    }

    #[test]
    fn ignores_git_access_and_metadata_and_dedups() {
        let root = Path::new("/r");
        let events = [
            ev(EventKind::Create(CreateKind::File), &["/r/.git/index.lock"]),
            ev(
                EventKind::Access(notify_debouncer_full::notify::event::AccessKind::Any),
                &["/r/a"],
            ),
            ev(
                EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
                &["/r/a"],
            ),
            ev(EventKind::Modify(ModifyKind::Any), &["/r/n.md"]),
            ev(EventKind::Modify(ModifyKind::Any), &["/r/n.md"]),
        ];
        assert_eq!(
            convert(&events, root),
            [FsEvent::Modified("/r/n.md".into())]
        );
    }

    #[test]
    fn paths_of_a_rename_include_both_ends() {
        let e = FsEvent::Renamed {
            from: "/a".into(),
            to: "/b".into(),
        };
        assert_eq!(e.paths(), [Path::new("/a"), Path::new("/b")]);
    }

    /// Integración real con el sistema de archivos (puede tardar por la latencia del backend).
    #[tokio::test]
    async fn real_watcher_reports_creation_and_modification() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let mut watcher = watch(&root, true).unwrap();
        let file = root.join("nueva.md");
        fs::write(&file, "uno").unwrap();

        let mut seen = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !seen {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let batch = tokio::time::timeout(remaining, watcher.next())
                .await
                .expect("el watcher debe emitir antes de 10 s")
                .expect("el stream no debe terminar");
            seen = batch.iter().any(|e| e.paths().contains(&file.as_path()));
        }
        assert!(seen);
    }
}
