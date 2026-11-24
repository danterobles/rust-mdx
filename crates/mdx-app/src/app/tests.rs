use super::*;
use iced::widget::text_editor::{Action, Edit};
use std::sync::atomic::{AtomicBool, Ordering};

fn type_char(app: &mut App, c: char) {
    let _ = app.update(Message::Edit(Action::Edit(Edit::Insert(c))));
}

#[test]
fn starts_clean_and_untitled() {
    let (app, _) = App::new(None);
    assert_eq!(app.title(), "Sin título — RustMDX");
}

#[test]
fn editing_marks_dirty_in_title() {
    // AC1
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    assert_eq!(app.title(), "● Sin título — RustMDX");
}

#[test]
fn returning_to_saved_state_clears_indicator() {
    // AC3: borrar lo escrito devuelve el hash al del estado guardado.
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let _ = app.update(Message::Edit(Action::Edit(Edit::Backspace)));
    assert_eq!(app.title(), "Sin título — RustMDX");
}

#[test]
fn saved_content_is_clean_and_new_edit_is_dirty_again() {
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let info = SavedInfo {
        path: PathBuf::from("/tmp/x.md"),
        saved_hash: mdx_core::document::hash_text("a"),
        mtime: None,
    };
    let _ = app.update(Message::Saved(app.doc.id, Ok(info)));
    assert_eq!(app.title(), "x.md — RustMDX");
    type_char(&mut app, 'b');
    assert_eq!(app.title(), "● x.md — RustMDX");
}

#[test]
fn discard_closes_the_tab_and_cancel_keeps_it() {
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let id = app.doc.id;
    let _ = app.update(Message::GuardAnswered(
        PendingAction::CloseTab(id),
        GuardChoice::Cancel,
    ));
    assert_eq!(app.content.text(), "a");
    let _ = app.update(Message::GuardAnswered(
        PendingAction::CloseTab(id),
        GuardChoice::Discard,
    ));
    // Al cerrar la última pestaña queda un «Sin título» vacío.
    assert_eq!(app.content.text(), "");
    assert_ne!(app.doc.id, id);
    assert_eq!(app.title(), "Sin título — RustMDX");
    assert_eq!(app.tabs.tabs().len(), 1);
}

#[test]
fn undo_and_redo_typing_burst() {
    // AC3: una ráfaga de tecleo es un solo paso.
    let (mut app, _) = App::new(None);
    for c in "hola".chars() {
        type_char(&mut app, c);
    }
    let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
    assert_eq!(app.content.text(), "");
    assert_eq!(app.title(), "Sin título — RustMDX");
    let _ = app.update(Message::Shortcut(ShortcutAction::Redo));
    assert_eq!(app.content.text(), "hola");
    assert_eq!(app.title(), "● Sin título — RustMDX");
}

#[test]
fn undo_restores_cursor_and_edit_after_undo_drops_redo() {
    let (mut app, _) = App::new(None);
    for c in "ab".chars() {
        type_char(&mut app, c);
    }
    let _ = app.update(Message::Edit(Action::Edit(Edit::Enter)));
    type_char(&mut app, 'c');
    let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
    assert_eq!(app.content.text(), "ab\n");
    let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
    assert_eq!(app.content.text(), "ab");
    assert_eq!(editor::cursor_pos(&app.content), (0, 2));
    type_char(&mut app, 'z');
    let _ = app.update(Message::Shortcut(ShortcutAction::Redo));
    assert_eq!(app.content.text(), "abz");
}

#[test]
fn indent_and_unindent_selection() {
    // AC4: con la línea seleccionada, Tab la indenta y Shift+Tab la desindenta.
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let _ = app.update(Message::Edit(Action::SelectAll));
    let _ = app.update(Message::Edit(Action::Edit(Edit::Indent)));
    let indented = app.content.text();
    assert!(indented.ends_with('a') && indented.len() > 1 && indented.trim() == "a");
    let _ = app.update(Message::Edit(Action::Edit(Edit::Unindent)));
    assert_eq!(app.content.text(), "a");
}

#[test]
fn stats_follow_cursor_and_words() {
    // AC5
    let (mut app, _) = App::new(None);
    for c in "uno dos".chars() {
        type_char(&mut app, c);
    }
    let _ = app.update(Message::Edit(Action::Edit(Edit::Enter)));
    type_char(&mut app, 'ñ');
    let stats = app.stats();
    assert_eq!((stats.line, stats.col, stats.words), (2, 2, 3));
}

fn parsed_of(text: &str) -> Parsed {
    Parsed {
        items: markdown::parse(text).collect(),
        outline: mdx_core::outline::extract(text),
        blocks: mdx_core::outline::top_level_blocks(text),
        unavailable_images: std::collections::HashSet::new(),
    }
}

/// Simula la llegada del resultado del parseo en segundo plano.
fn deliver_preview(app: &mut App) {
    let parsed = parsed_of(&app.doc.text);
    let _ = app.update(Message::PreviewParsed(app.doc.id, app.preview_gen, parsed));
}

#[test]
fn preview_updates_only_after_debounce() {
    // AC1: el re-parseo se lanza ≥150 ms después de la última edición.
    let (mut app, _) = App::new(None);
    for c in "# T".chars() {
        type_char(&mut app, c);
    }
    assert!(app.preview_pending);
    let launched = app.preview_gen;
    let edited = app.preview_edit_at;
    let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(100)));
    assert!(app.preview_pending, "aún dentro del debounce");
    assert_eq!(app.preview_gen, launched);
    let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(150)));
    assert!(!app.preview_pending);
    assert_eq!(app.preview_gen, launched + 1);
    assert!(app.preview.is_empty());
    deliver_preview(&mut app);
    assert!(!app.preview.is_empty());
}

#[test]
fn stale_preview_results_are_discarded() {
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let edited = app.preview_edit_at;
    let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(150)));
    let stale = app.preview_gen - 1;
    let _ = app.update(Message::PreviewParsed(
        app.doc.id,
        stale,
        parsed_of("# viejo"),
    ));
    assert!(app.preview.is_empty());
}

#[test]
fn loaded_document_starts_preview_parse() {
    let doc = Document::from_bytes(Path::new("a.md"), b"# Hola\n\ntexto", None).unwrap();
    let (mut app, _) = App::new(None);
    let before = app.preview_gen;
    let _ = app.update(Message::DocumentLoaded(Ok(doc)));
    assert_eq!(app.preview_gen, before + 1);
    assert!(!app.preview_pending);
    deliver_preview(&mut app);
    assert!(!app.preview.is_empty());
}

#[test]
fn toggle_preview_hides_and_restores_pane() {
    // AC4
    let (mut app, _) = App::new(None);
    assert_eq!(app.panes.len(), 3);
    let _ = app.update(Message::Shortcut(ShortcutAction::TogglePreview));
    assert!(!app.preview_visible);
    assert_eq!(app.panes.len(), 2);
    let _ = app.update(Message::Shortcut(ShortcutAction::TogglePreview));
    assert_eq!(app.panes.len(), 3);
}

#[test]
fn unsupported_link_reports_error_and_keeps_document() {
    // AC3
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let _ = app.update(Message::LinkClicked("mailto:a@b.c".into()));
    assert!(app.status.as_ref().is_some_and(|s| s.is_error));
    assert_eq!(app.content.text(), "a");
}

/// NFR-4: coste por edición con documentos grandes (`cargo test --workspace -- --ignored`).
/// Ejecutar con `--release` para cifras representativas; el umbral es holgado para debug.
#[test]
#[ignore = "rendimiento: requiere fixtures/large_*.md (cargo xtask gen-fixtures)"]
fn perf_edit_cost_on_large_documents() {
    const N: u128 = 20;
    for (name, budget_ms) in [("large_1mb.md", 50u128), ("large_5mb.md", 250u128)] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        let bytes = std::fs::read(&path).expect("ejecuta `cargo xtask gen-fixtures`");
        let doc = Document::from_bytes(&path, &bytes, None).unwrap();
        let (mut app, _) = App::new(None);
        let t = Instant::now();
        let _ = app.update(Message::DocumentLoaded(Ok(doc)));
        let load_ms = t.elapsed().as_millis();

        // Medir el coste de una pulsación (editar + diff + hash + palabras).
        let mut worst = 0u128;
        let mut total = 0u128;
        for _ in 0..N {
            let t = Instant::now();
            type_char(&mut app, 'x');
            let ms = t.elapsed().as_millis();
            worst = worst.max(ms);
            total += ms;
        }
        // Coste del parseo de la vista previa (corre fuera del hilo de UI en la app real).
        let t = Instant::now();
        let items: Vec<_> = markdown::parse(&app.doc.text).collect();
        let preview_ms = t.elapsed().as_millis();
        drop(items);
        // Deshacer.
        let t = Instant::now();
        let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
        let undo_ms = t.elapsed().as_millis();
        eprintln!(
            "PERF {name}: carga={load_ms} ms, edición media={} ms (peor {worst} ms), \
             parseo preview (hilo aparte)={preview_ms} ms, deshacer={undo_ms} ms",
            total / N
        );
        assert!(
            total / N <= budget_ms,
            "{name}: edición media {} ms > {budget_ms} ms",
            total / N
        );
    }
}

/// NFR-4 en F3: coste de las funciones añadidas en la fase (buscar, scroll sincronizado,
/// esquema). `cargo test --release -p mdx-app perf_f3 -- --ignored --nocapture`.
#[test]
#[ignore = "rendimiento: requiere fixtures/large_*.md (cargo xtask gen-fixtures)"]
fn perf_f3_features_on_large_documents() {
    let time = |f: &mut dyn FnMut()| {
        let t = Instant::now();
        f();
        t.elapsed().as_millis()
    };
    for name in ["large_1mb.md", "large_5mb.md"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        let bytes = std::fs::read(&path).expect("ejecuta `cargo xtask gen-fixtures`");
        let doc = Document::from_bytes(&path, &bytes, None).unwrap();
        let (mut app, _) = App::new(None);
        let _ = app.update(Message::DocumentLoaded(Ok(doc)));

        // Parseos que corren en un hilo aparte en la app real.
        let text = app.doc.text.clone();
        let outline_ms = time(&mut || {
            let _ = mdx_core::outline::extract(&text);
        });
        let blocks_ms = time(&mut || {
            let _ = mdx_core::outline::top_level_blocks(&text);
        });

        // Buscar: abrir la barra y escribir un patrón con muchas coincidencias.
        let _ = app.update(Message::Shortcut(ShortcutAction::Find));
        let find_ms = time(&mut || {
            let _ = app.update(Message::FindQueryChanged("Sección".to_owned()));
        });
        let matches = app.find.matches.len();
        let next_ms = time(&mut || {
            let _ = app.update(Message::FindNext);
        });
        // Pulsación con la barra de búsqueda abierta (recalcula coincidencias).
        let mut worst = 0u128;
        let mut total = 0u128;
        for _ in 0..10 {
            let ms = time(&mut || type_char(&mut app, 'x'));
            worst = worst.max(ms);
            total += ms;
        }
        // Reemplazar todo (un paso de deshacer).
        let _ = app.update(Message::Shortcut(ShortcutAction::Replace));
        let _ = app.update(Message::ReplaceChanged("Parte".to_owned()));
        let replace_ms = time(&mut || {
            let _ = app.update(Message::ReplaceAll);
        });
        let undo_ms = time(&mut || {
            let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
        });
        // Scroll de rueda: sondeo de la primera línea visible.
        let scroll_ms = time(&mut || {
            for _ in 0..10 {
                let _ = app.update(Message::Edit(Action::Scroll { lines: 5 }));
            }
        }) / 10;
        eprintln!(
            "PERF-F3 {name}: esquema={outline_ms} ms, bloques={blocks_ms} ms (ambos en hilo aparte), \
             buscar «Sección» ({matches} coincidencias)={find_ms} ms, siguiente={next_ms} ms, \
             pulsación con la barra abierta={} ms (peor {worst} ms), reemplazar todo={replace_ms} ms, \
             deshacer={undo_ms} ms, scroll (sondeo)={scroll_ms} ms",
            total / 10
        );
    }
}

#[test]
fn replace_all_and_undo_work_on_documents_big_enough_to_rebuild_the_editor() {
    // El camino «masivo»: más de 64 KB se aplica reconstruyendo el editor.
    let text = "uno dos\n".repeat(20_000);
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, &text, "uno");
    let _ = app.update(Message::Shortcut(ShortcutAction::Replace));
    let _ = app.update(Message::ReplaceChanged("1".to_owned()));
    let _ = app.update(Message::ReplaceAll);
    assert!(app.content.text().starts_with("1 dos\n1 dos\n"));
    assert_eq!(app.find.notice.as_deref(), Some("20000 reemplazos"));
    assert_eq!(app.title(), "● a.md — RustMDX");
    let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
    assert_eq!(app.content.text(), text);
    assert_eq!(app.title(), "a.md — RustMDX", "volvió al estado guardado");
    let _ = app.update(Message::Shortcut(ShortcutAction::Redo));
    assert!(app.content.text().starts_with("1 dos\n"));
}

fn temp_workspace() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/a.md"), "x").unwrap();
    std::fs::write(dir.path().join("r.md"), "x").unwrap();
    let ws = Workspace::open(dir.path(), TreeFilters::default()).unwrap();
    (dir, ws)
}

#[test]
fn workspace_opened_is_stored() {
    // AC1 (lógica): al abrir una carpeta queda el árbol cargado.
    let (_dir, ws) = temp_workspace();
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    assert!(app.explorer.workspace.is_some());
    assert!(app.status.as_ref().is_some_and(|s| !s.is_error));
}

#[test]
fn toggle_dir_loads_lazily_then_collapses() {
    // AC2
    let (_dir, ws) = temp_workspace();
    let sub = ws.root.join("sub");
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    let state = |app: &App, p: &Path| app.explorer.workspace.as_ref().unwrap().node(p).cloned();

    let _ = app.update(Message::ToggleDir(sub.clone()));
    let node = state(&app, &sub).unwrap();
    assert!(matches!(
        node.kind,
        mdx_core::workspace::NodeKind::Dir {
            children: mdx_core::workspace::Children::Loading,
            expanded: true
        }
    ));

    let kids = workspace::load_children(&sub, &TreeFilters::default());
    let _ = app.update(Message::DirLoaded(sub.clone(), kids));
    assert!(state(&app, &sub.join("a.md")).is_some());

    let _ = app.update(Message::ToggleDir(sub.clone()));
    assert!(matches!(
        state(&app, &sub).unwrap().kind,
        mdx_core::workspace::NodeKind::Dir {
            expanded: false,
            ..
        }
    ));
}

#[test]
fn toggling_filters_updates_flags() {
    // AC3 / AC4
    let (mut app, _) = App::new(None);
    assert!(app.explorer.filters.only_markdown && !app.explorer.filters.show_hidden);
    let _ = app.update(Message::ToggleFilter(FilterKind::OnlyMarkdown));
    let _ = app.update(Message::ToggleFilter(FilterKind::Hidden));
    let _ = app.update(Message::ToggleFilter(FilterKind::Gitignore));
    let f = app.explorer.filters;
    assert!(!f.only_markdown && f.show_hidden && !f.respect_gitignore);
    let _ = app.update(Message::ToggleExplorerMenu);
    assert!(app.explorer.menu_open);
}

#[test]
fn failed_workspace_open_reports_error() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::WorkspaceOpened(Err(CoreError::NotFound(
        PathBuf::from("/x"),
    ))));
    assert!(app.explorer.workspace.is_none());
    assert!(app.status.as_ref().is_some_and(|s| s.is_error));
}

fn load(app: &mut App, name: &str) -> PathBuf {
    let path = PathBuf::from(format!("/proyecto/{name}"));
    let doc = Document::from_bytes(&path, b"# x", None).unwrap();
    let _ = app.update(Message::DocumentLoaded(Ok(doc)));
    path
}

fn unpinned_tabs(app: &App) -> usize {
    app.tabs.tabs().iter().filter(|t| !t.pinned).count()
}

#[test]
fn single_clicks_reuse_one_preview_tab() {
    // AC1 (INV-7): clic simple sobre archivos distintos nunca acumula pestañas preview.
    let (mut app, _) = App::new(None);
    for name in ["a.md", "b.md", "c.md"] {
        let path = PathBuf::from(format!("/proyecto/{name}"));
        let _ = app.update(Message::FileClicked(path));
        load(&mut app, name);
    }
    assert_eq!(unpinned_tabs(&app), 1);
    assert_eq!(app.tabs.tabs().len(), 1);
    assert_eq!(app.doc.path.as_deref(), Some(Path::new("/proyecto/c.md")));
    assert_eq!(
        app.explorer.selected.as_deref(),
        Some(Path::new("/proyecto/c.md"))
    );
}

#[test]
fn double_click_pins_the_opened_file() {
    // AC1: el doble clic llega con el archivo aún cargándose.
    let (mut app, _) = App::new(None);
    let path = PathBuf::from("/proyecto/a.md");
    let _ = app.update(Message::FileClicked(path.clone()));
    let _ = app.update(Message::FilePinned(path));
    load(&mut app, "a.md");
    assert_eq!(unpinned_tabs(&app), 0);
    assert!(app.tabs.is_pinned(app.doc.id));
}

#[test]
fn double_click_on_open_preview_pins_it_without_reloading() {
    let (mut app, _) = App::new(None);
    let path = load(&mut app, "a.md");
    assert_eq!(unpinned_tabs(&app), 1);
    let id = app.doc.id;
    let _ = app.update(Message::FilePinned(path));
    assert!(app.tabs.is_pinned(id));
    assert_eq!(app.doc.id, id, "no se recarga el documento");
}

#[test]
fn clicking_a_file_with_unsaved_changes_does_not_discard_them() {
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let _ = app.update(Message::FileClicked(PathBuf::from("/proyecto/b.md")));
    assert_eq!(
        app.content.text(),
        "a",
        "debe preguntar antes de reemplazar"
    );
}

#[test]
fn keyboard_navigation_moves_selection_and_toggles() {
    // AC2
    let (_dir, ws) = temp_workspace();
    let sub = ws.root.join("sub");
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    let _ = app.update(Message::ExplorerKey(ExplorerKey::Down));
    assert_eq!(app.explorer.selected.as_deref(), Some(sub.as_path()));
    let _ = app.update(Message::ExplorerKey(ExplorerKey::Right));
    let expanded = |app: &App| {
        matches!(
            app.explorer
                .workspace
                .as_ref()
                .unwrap()
                .node(&sub)
                .map(|n| &n.kind),
            Some(mdx_core::workspace::NodeKind::Dir { expanded: true, .. })
        )
    };
    assert!(expanded(&app));
    let _ = app.update(Message::ExplorerKey(ExplorerKey::Left));
    assert!(!expanded(&app));
}

fn open_prompt(app: &mut App, kind: PromptKind, target: &str, value: &str) {
    app.explorer.prompt = Some(Prompt {
        kind,
        target: PathBuf::from(target),
        value: value.to_owned(),
        error: None,
    });
}

#[test]
fn context_menu_opens_for_the_clicked_row_and_closes() {
    // AC1
    let (mut app, _) = App::new(None);
    let target = ContextTarget {
        path: PathBuf::from("/p/a.md"),
        is_dir: false,
        is_root: false,
    };
    let _ = app.update(Message::OpenContext(target.clone()));
    assert_eq!(app.explorer.context, Some(target));
    let _ = app.update(Message::CloseContext);
    assert!(app.explorer.context.is_none());
}

#[test]
fn context_actions_open_the_inline_name_prompt() {
    // AC1: Nuevo archivo sobre un archivo crea en su carpeta; Renombrar precarga el nombre.
    let (mut app, _) = App::new(None);
    let target = ContextTarget {
        path: PathBuf::from("/p/sub/a.md"),
        is_dir: false,
        is_root: false,
    };
    let _ = app.update(Message::OpenContext(target.clone()));
    let _ = app.update(Message::ContextAction(ContextAction::NewFile));
    let prompt = app.explorer.prompt.clone().unwrap();
    assert_eq!(
        (prompt.kind, prompt.target.as_path()),
        (PromptKind::NewFile, Path::new("/p/sub"))
    );

    let _ = app.update(Message::OpenContext(target));
    let _ = app.update(Message::ContextAction(ContextAction::Rename));
    let prompt = app.explorer.prompt.clone().unwrap();
    assert_eq!(
        (prompt.kind, prompt.value.as_str()),
        (PromptKind::Rename, "a.md")
    );
    assert!(
        app.explorer.context.is_none(),
        "el menú se cierra al elegir"
    );
}

#[test]
fn invalid_name_shows_inline_error_without_running_any_operation() {
    // AC4
    let (mut app, _) = App::new(None);
    open_prompt(&mut app, PromptKind::NewFile, "/p", "a/b");
    let _ = app.update(Message::PromptSubmit);
    let prompt = app.explorer.prompt.clone().unwrap();
    assert!(
        prompt
            .error
            .as_deref()
            .is_some_and(|e| e.contains("no está permitido"))
    );
    // Escribir de nuevo limpia el error.
    let _ = app.update(Message::PromptChanged("ok".into()));
    assert!(app.explorer.prompt.as_ref().unwrap().error.is_none());
}

#[test]
fn duplicate_name_error_from_disk_is_shown_inline_and_keeps_the_prompt() {
    // AC4
    let (mut app, _) = App::new(None);
    open_prompt(&mut app, PromptKind::Rename, "/p/a.md", "b.md");
    let _ = app.update(Message::OpDone(Err(CoreError::AlreadyExists(
        PathBuf::from("/p/b.md"),
    ))));
    let prompt = app.explorer.prompt.clone().unwrap();
    assert!(
        prompt
            .error
            .as_deref()
            .is_some_and(|e| e.contains("Ya existe"))
    );
    assert!(
        app.status.is_none(),
        "no es un error global de la barra de estado"
    );
}

#[test]
fn renaming_the_open_file_updates_its_path() {
    // AC3
    let (mut app, _) = App::new(None);
    let from = load(&mut app, "a.md");
    type_char(&mut app, 'x');
    open_prompt(&mut app, PromptKind::Rename, "/proyecto/a.md", "b.md");
    let to = PathBuf::from("/proyecto/b.md");
    let _ = app.update(Message::OpDone(Ok(OpOutcome::Renamed {
        from,
        to: to.clone(),
    })));
    assert_eq!(app.doc.path.as_deref(), Some(to.as_path()));
    assert!(app.explorer.prompt.is_none());
    assert_eq!(
        app.title(),
        "● b.md — RustMDX",
        "conserva el estado sin guardar"
    );
}

#[test]
fn renaming_a_parent_folder_rebases_the_open_file() {
    // AC3
    let (mut app, _) = App::new(None);
    load(&mut app, "dir/a.md");
    let _ = app.update(Message::OpDone(Ok(OpOutcome::Renamed {
        from: PathBuf::from("/proyecto/dir"),
        to: PathBuf::from("/proyecto/nuevo"),
    })));
    assert_eq!(
        app.doc.path.as_deref(),
        Some(Path::new("/proyecto/nuevo/a.md"))
    );
}

#[test]
fn trashing_the_open_file_keeps_its_content_marked_deleted() {
    // AC2
    let (mut app, _) = App::new(None);
    let path = load(&mut app, "a.md");
    let _ = app.update(Message::OpDone(Ok(OpOutcome::Trashed(path))));
    assert_eq!(app.doc.disk_state, mdx_core::DiskState::Deleted);
    assert_eq!(app.content.text(), "# x");
}

#[test]
fn declining_the_trash_confirmation_does_nothing() {
    // AC2
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::DeleteAnswered(PathBuf::from("/p/a.md"), false));
    assert!(app.status.is_none());
}

/// Abre `/proyecto/a.md` con `text` en disco y devuelve su ruta.
fn open_with(app: &mut App, text: &str) -> PathBuf {
    let path = PathBuf::from("/proyecto/a.md");
    let doc = Document::from_bytes(&path, text.as_bytes(), None).unwrap();
    let _ = app.update(Message::DocumentLoaded(Ok(doc)));
    path
}

fn inspect(app: &mut App, path: &Path, result: Result<Document, CoreError>) {
    let _ = app.update(Message::DiskInspected(
        app.doc.id,
        path.to_path_buf(),
        result,
    ));
}

fn disk_version(path: &Path, text: &str) -> Result<Document, CoreError> {
    Document::from_bytes(path, text.as_bytes(), None)
}

#[test]
fn external_change_on_clean_document_reloads_silently() {
    // AC2
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "viejo");
    inspect(
        &mut app,
        &path,
        disk_version(&path, "nuevo desde otra herramienta"),
    );
    assert_eq!(app.content.text(), "nuevo desde otra herramienta");
    assert_eq!(app.doc.disk_state, DiskState::InSync);
    assert_eq!(app.title(), "a.md — RustMDX");
    assert!(app.external.is_none());
}

#[test]
fn external_change_on_dirty_document_shows_conflict_banner() {
    // AC3
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "base");
    type_char(&mut app, 'X');
    inspect(&mut app, &path, disk_version(&path, "otra versión"));
    assert_eq!(app.doc.disk_state, DiskState::ChangedExternally);
    assert!(app.external.is_some());
    assert_eq!(app.content.text(), "Xbase", "el texto local no se toca");
}

#[test]
fn conflict_reload_takes_the_disk_version() {
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "base");
    type_char(&mut app, 'X');
    inspect(&mut app, &path, disk_version(&path, "disco"));
    let _ = app.update(Message::ResolveConflict(ConflictChoice::Reload));
    assert_eq!(app.content.text(), "disco");
    assert_eq!(app.doc.disk_state, DiskState::InSync);
    assert_eq!(app.title(), "a.md — RustMDX");
}

#[test]
fn conflict_keep_mine_keeps_text_and_stays_dirty() {
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "base");
    type_char(&mut app, 'X');
    inspect(&mut app, &path, disk_version(&path, "disco"));
    let _ = app.update(Message::ResolveConflict(ConflictChoice::KeepMine));
    assert_eq!(app.content.text(), "Xbase");
    assert_eq!(app.doc.disk_state, DiskState::InSync);
    assert_eq!(app.title(), "● a.md — RustMDX");
    // Un nuevo aviso con el mismo contenido de disco ya no molesta.
    inspect(&mut app, &path, disk_version(&path, "disco"));
    assert_eq!(app.doc.disk_state, DiskState::InSync);
}

#[test]
fn own_save_never_triggers_the_banner() {
    // AC5 / INV-3: el disco coincide con `saved_hash`, aunque se haya seguido escribiendo.
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "base");
    type_char(&mut app, 'X');
    let saved = mdx_core::document::hash_text("Xbase");
    let info = SavedInfo {
        path: path.clone(),
        saved_hash: saved,
        mtime: None,
    };
    let _ = app.update(Message::Saved(app.doc.id, Ok(info)));
    type_char(&mut app, 'Y'); // sigue editando mientras llega el evento del guardado
    inspect(&mut app, &path, disk_version(&path, "Xbase"));
    assert_eq!(app.doc.disk_state, DiskState::InSync);
    assert!(app.external.is_none());
    assert_eq!(app.content.text(), "XYbase");
}

#[test]
fn deleted_on_disk_is_marked_and_keeps_content_and_can_be_resaved() {
    // AC4
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "contenido");
    inspect(&mut app, &path, Err(CoreError::NotFound(path.clone())));
    assert_eq!(app.doc.disk_state, DiskState::Deleted);
    assert_eq!(app.title(), "a.md (eliminado) — RustMDX");
    assert_eq!(app.content.text(), "contenido");
    // Volver a guardar restituye el archivo y quita la marca.
    let info = SavedInfo {
        path: path.clone(),
        saved_hash: mdx_core::document::hash_text("contenido"),
        mtime: None,
    };
    let _ = app.update(Message::Saved(app.doc.id, Ok(info)));
    assert_eq!(app.title(), "a.md — RustMDX");
}

#[test]
fn stale_inspection_results_are_ignored() {
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "base");
    let old_id = app.doc.id;
    // Se abre otro documento mientras se leía el disco.
    let _ = load(&mut app, "otro.md");
    let _ = app.update(Message::DiskInspected(
        old_id,
        path.clone(),
        disk_version(&path, "x"),
    ));
    assert_eq!(app.content.text(), "# x");
    assert_eq!(app.doc.disk_state, DiskState::InSync);
}

#[test]
fn fs_events_for_unrelated_files_do_nothing_to_the_document() {
    let (mut app, _) = App::new(None);
    let _ = open_with(&mut app, "base");
    let _ = app.update(Message::Fs(vec![FsEvent::Modified(PathBuf::from(
        "/otro/b.md",
    ))]));
    assert_eq!(app.content.text(), "base");
}

#[test]
fn cancelling_save_dialog_clears_pending_action() {
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let _ = app.update(Message::GuardAnswered(
        PendingAction::Close,
        GuardChoice::Save,
    ));
    assert_eq!(app.after_save, Some(PendingAction::Close));
    let _ = app.update(Message::SavePicked(None));
    assert_eq!(app.after_save, None);
}

// ---------------------------------------------------------------- US-3.1: pestañas

/// Abre `name` con `text` como pestaña fija (doble clic) y devuelve su id.
fn open_pinned_doc(app: &mut App, name: &str, text: &str) -> DocumentId {
    let path = PathBuf::from(format!("/proyecto/{name}"));
    let _ = app.update(Message::FilePinned(path.clone()));
    let doc = Document::from_bytes(&path, text.as_bytes(), None).unwrap();
    let _ = app.update(Message::DocumentLoaded(Ok(doc)));
    app.doc.id
}

fn tab_names(app: &App) -> Vec<String> {
    app.tabs
        .tabs()
        .iter()
        .filter_map(|t| app.tab_label(t.doc))
        .map(|l| l.name)
        .collect()
}

#[test]
fn opening_several_files_keeps_every_document_in_its_own_tab() {
    // AC1
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    let b = open_pinned_doc(&mut app, "b.md", "dos");
    assert_eq!(
        tab_names(&app),
        ["a.md", "b.md"],
        "el «Sin título» vacío se reemplazó"
    );
    assert_eq!(app.doc.id, b);
    assert!(app.parked.contains_key(&a));
    assert_eq!(app.content.text(), "dos");
}

#[test]
fn switching_tabs_preserves_each_documents_state() {
    // AC1
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    type_char(&mut app, 'X');
    let b = open_pinned_doc(&mut app, "b.md", "dos");
    assert_eq!(app.content.text(), "dos");
    assert!(!app.dirty);
    let _ = app.update(Message::SelectTab(a));
    assert_eq!(app.doc.id, a);
    assert_eq!(app.content.text(), "Xuno");
    assert_eq!(app.title(), "● a.md — RustMDX");
    assert_eq!(app.tabs.active(), Some(a));
    // El historial de deshacer viaja con cada documento.
    let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
    assert_eq!(app.content.text(), "uno");
    let _ = app.update(Message::SelectTab(b));
    assert_eq!(app.content.text(), "dos");
}

#[test]
fn opening_an_already_open_file_activates_its_tab_without_duplicating() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::FileClicked(PathBuf::from("/proyecto/a.md")));
    assert_eq!(app.doc.id, a);
    assert_eq!(app.tabs.tabs().len(), 2);
}

#[test]
fn editing_a_preview_tab_pins_it_so_it_is_never_replaced() {
    let (mut app, _) = App::new(None);
    load(&mut app, "a.md"); // clic simple: preview
    assert_eq!(unpinned_tabs(&app), 1);
    type_char(&mut app, 'x');
    assert_eq!(unpinned_tabs(&app), 0);
    load(&mut app, "b.md");
    assert_eq!(app.tabs.tabs().len(), 2, "la pestaña editada sobrevive");
}

#[test]
fn ctrl_w_closes_a_clean_tab_and_activates_the_previous_one() {
    // AC1
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::Shortcut(ShortcutAction::CloseTab));
    assert_eq!(tab_names(&app), ["a.md"]);
    assert_eq!(app.doc.id, a);
    assert_eq!(app.content.text(), "uno");
    assert!(app.parked.is_empty());
}

#[test]
fn closing_a_dirty_tab_asks_first_and_discard_or_cancel_resolve_it() {
    // AC1: con cambios pendientes no se cierra hasta responder.
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    type_char(&mut app, 'X');
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::CloseTab(a)); // clic en × de la pestaña sucia
    assert_eq!(
        tab_names(&app),
        ["a.md", "b.md"],
        "sigue abierta hasta responder"
    );
    let _ = app.update(Message::GuardAnswered(
        PendingAction::CloseTab(a),
        GuardChoice::Cancel,
    ));
    assert_eq!(app.tabs.tabs().len(), 2);
    let _ = app.update(Message::GuardAnswered(
        PendingAction::CloseTab(a),
        GuardChoice::Discard,
    ));
    assert_eq!(tab_names(&app), ["b.md"]);
}

#[test]
fn saving_from_the_guard_dialog_selects_the_tab_and_closes_it_after_saving() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    type_char(&mut app, 'X');
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::GuardAnswered(
        PendingAction::CloseTab(a),
        GuardChoice::Save,
    ));
    assert_eq!(app.doc.id, a, "se muestra la pestaña que se guarda");
    assert_eq!(app.after_save, Some(PendingAction::CloseTab(a)));
    let info = SavedInfo {
        path: PathBuf::from("/proyecto/a.md"),
        saved_hash: mdx_core::document::hash_text("Xuno"),
        mtime: None,
    };
    let _ = app.update(Message::Saved(a, Ok(info)));
    assert_eq!(tab_names(&app), ["b.md"]);
    assert!(app.after_save.is_none());
}

#[test]
fn a_save_result_for_a_background_tab_updates_that_tab_only() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    type_char(&mut app, 'X');
    let b = open_pinned_doc(&mut app, "b.md", "dos");
    let info = SavedInfo {
        path: PathBuf::from("/proyecto/a.md"),
        saved_hash: mdx_core::document::hash_text("Xuno"),
        mtime: None,
    };
    let _ = app.update(Message::Saved(a, Ok(info)));
    assert_eq!(app.doc.id, b, "la pestaña activa no cambia");
    assert!(app.first_dirty().is_none(), "a.md ya no está sucia");
}

#[test]
fn closing_the_window_reviews_every_open_document() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    type_char(&mut app, 'X');
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    assert_eq!(
        app.first_dirty(),
        Some(a),
        "hay un documento sucio en segundo plano"
    );
    // Descartar cierra esa pestaña y ya no queda nada pendiente.
    let _ = app.update(Message::GuardAnswered(
        PendingAction::Close,
        GuardChoice::Discard,
    ));
    assert!(app.first_dirty().is_none());
    assert_eq!(tab_names(&app), ["b.md"]);
}

#[test]
fn tab_shortcuts_cycle_and_jump() {
    // AC2
    let (mut app, _) = App::new(None);
    let ids: Vec<_> = ["a.md", "b.md", "c.md"]
        .iter()
        .map(|n| open_pinned_doc(&mut app, n, "x"))
        .collect();
    assert_eq!(app.doc.id, ids[2]);
    let _ = app.update(Message::Shortcut(ShortcutAction::NextTab));
    assert_eq!(app.doc.id, ids[0], "circular");
    let _ = app.update(Message::Shortcut(ShortcutAction::PrevTab));
    assert_eq!(app.doc.id, ids[2]);
    let _ = app.update(Message::Shortcut(ShortcutAction::GoToTab(2)));
    assert_eq!(app.doc.id, ids[1]);
    let _ = app.update(Message::Shortcut(ShortcutAction::GoToTab(9)));
    assert_eq!(app.doc.id, ids[2], "9 salta a la última");
    let _ = app.update(Message::Shortcut(ShortcutAction::GoToTab(7)));
    assert_eq!(app.doc.id, ids[2], "una posición inexistente no hace nada");
}

#[test]
fn tab_label_shows_dirty_mark_italic_state_and_full_path_tooltip() {
    // AC3
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    type_char(&mut app, 'X');
    let b = {
        let path = PathBuf::from("/otra/ruta/b.md");
        let _ = app.update(Message::FileClicked(path.clone())); // clic simple → preview
        let doc = Document::from_bytes(&path, b"dos", None).unwrap();
        let _ = app.update(Message::DocumentLoaded(Ok(doc)));
        app.doc.id
    };
    let la = app.tab_label(a).unwrap();
    assert!(la.dirty && la.pinned);
    assert_eq!(la.full_path, "/proyecto/a.md");
    let lb = app.tab_label(b).unwrap();
    assert!(!lb.dirty && !lb.pinned, "la preview se muestra en cursiva");
    assert_eq!(lb.full_path, "/otra/ruta/b.md");
}

#[test]
fn untitled_tab_has_a_tooltip_and_new_does_not_pile_up_empty_tabs() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::Shortcut(ShortcutAction::New));
    assert_eq!(app.tabs.tabs().len(), 1, "ya hay un «Sin título» vacío");
    open_pinned_doc(&mut app, "a.md", "uno");
    let _ = app.update(Message::Shortcut(ShortcutAction::New));
    assert_eq!(tab_names(&app), ["a.md", "Sin título"]);
    assert_eq!(app.tab_label(app.doc.id).unwrap().full_path, "Sin título");
}

#[test]
fn external_changes_reach_documents_in_background_tabs() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "viejo");
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    let path = PathBuf::from("/proyecto/a.md");
    let _ = app.update(Message::DiskInspected(
        a,
        path.clone(),
        Document::from_bytes(&path, b"nuevo", None),
    ));
    assert_ne!(app.doc.id, a, "no roba el foco");
    let _ = app.update(Message::SelectTab(a));
    assert_eq!(app.content.text(), "nuevo");
}

#[test]
fn renaming_updates_the_path_of_background_tabs_too() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::OpDone(Ok(OpOutcome::Renamed {
        from: PathBuf::from("/proyecto/a.md"),
        to: PathBuf::from("/proyecto/z.md"),
    })));
    assert_eq!(app.tab_label(a).unwrap().name, "z.md");
}

#[test]
fn deleting_a_file_marks_its_background_tab_deleted() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    let _ = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::OpDone(Ok(OpOutcome::Trashed(PathBuf::from(
        "/proyecto/a.md",
    )))));
    assert!(app.tab_label(a).unwrap().deleted);
}

// ---------------------------------------------------------------- US-3.2: esquema

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        if c == '\n' {
            let _ = app.update(Message::Edit(Action::Edit(Edit::Enter)));
        } else {
            type_char(app, c);
        }
    }
}

#[test]
fn outline_is_empty_until_the_debounced_parse_arrives() {
    // AC3: mismo debounce que la vista previa.
    let (mut app, _) = App::new(None);
    type_text(&mut app, "# Uno\n## Dos");
    assert_eq!(app.outline, Vec::new());
    let edited = app.preview_edit_at;
    let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(100)));
    assert!(app.preview_pending, "aún dentro del debounce");
    let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(150)));
    deliver_preview(&mut app);
    let shown: Vec<_> = app
        .outline
        .iter()
        .map(|h| (h.level, h.text.as_str(), h.line))
        .collect();
    assert_eq!(shown, [(1, "Uno", 0), (2, "Dos", 1)]);
}

#[test]
fn outline_keeps_updating_when_the_preview_is_hidden() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::Shortcut(ShortcutAction::TogglePreview));
    assert!(!app.preview_visible);
    type_text(&mut app, "# Solo esquema");
    let edited = app.preview_edit_at;
    let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(150)));
    assert!(
        !app.preview_pending,
        "se parsea aunque la vista previa esté oculta"
    );
    deliver_preview(&mut app);
    assert_eq!(app.outline.len(), 1);
}

#[test]
fn clicking_a_heading_moves_the_cursor_to_its_line() {
    // AC2
    let (mut app, _) = App::new(None);
    type_text(&mut app, "# Uno\n\ntexto\n\n## Dos\nfin");
    let line = 4;
    let _ = app.update(Message::GoToLine(line));
    assert_eq!(editor::cursor_pos(&app.content), (4, 0));
    // Una línea fuera de rango se acota a la última.
    let _ = app.update(Message::GoToLine(999));
    assert_eq!(editor::cursor_pos(&app.content).0, 5);
    // Mover el cursor no es una edición: el documento no queda sucio por ello.
    let before = app.dirty;
    let _ = app.update(Message::GoToLine(0));
    assert_eq!(app.dirty, before);
}

#[test]
fn sidebar_switches_between_files_and_outline() {
    // AC1
    let (mut app, _) = App::new(None);
    assert_eq!(app.sidebar, Sidebar::Files);
    let _ = app.update(Message::SetSidebar(Sidebar::Outline));
    assert_eq!(app.sidebar, Sidebar::Outline);
    let _ = app.update(Message::SetSidebar(Sidebar::Files));
    assert_eq!(app.sidebar, Sidebar::Files);
}

#[test]
fn each_tab_has_its_own_outline() {
    // AC1: el esquema es el del documento activo.
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "# De A");
    deliver_preview(&mut app);
    let b = open_pinned_doc(&mut app, "b.md", "# De B\n## Sub B");
    deliver_preview(&mut app);
    assert_eq!(app.outline.len(), 2);
    let _ = app.update(Message::SelectTab(a));
    assert_eq!(app.outline.len(), 1);
    assert_eq!(app.outline[0].text, "De A");
    let _ = app.update(Message::SelectTab(b));
    assert_eq!(app.outline[1].text, "Sub B");
}

#[test]
fn a_stale_outline_result_is_ignored() {
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let edited = app.preview_edit_at;
    let _ = app.update(Message::PreviewTick(edited + Duration::from_millis(150)));
    let stale = app.preview_gen - 1;
    let _ = app.update(Message::PreviewParsed(
        app.doc.id,
        stale,
        parsed_of("# viejo"),
    ));
    assert_eq!(app.outline, Vec::new());
}

// ------------------------------------------------------ US-3.3: scroll sincronizado

#[test]
fn source_blocks_match_the_preview_items_one_to_one_for_the_repo_docs() {
    // El mapeo línea → bloque supone que `top_level_blocks` y el visor de iced coinciden.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![
        root.join("README.md"),
        root.join("CLAUDE.md"),
        root.join("fixtures/gfm_showcase.md"),
    ];
    for entry in std::fs::read_dir(root.join("docs")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "md") {
            files.push(path);
        }
    }
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            markdown::parse(&text).count(),
            mdx_core::outline::top_level_blocks(&text).len(),
            "{}",
            file.display()
        );
    }
}

fn open_long_doc(app: &mut App) {
    let text: String = "línea\n".repeat(100);
    let path = PathBuf::from("/proyecto/largo.md");
    let doc = Document::from_bytes(&path, text.as_bytes(), None).unwrap();
    let _ = app.update(Message::DocumentLoaded(Ok(doc)));
}

#[test]
fn scrolling_the_editor_tracks_the_first_visible_line() {
    // AC1
    let (mut app, _) = App::new(None);
    open_long_doc(&mut app);
    let _ = app.update(Message::Edit(Action::Scroll { lines: 10 }));
    assert_eq!(app.view_top.0, 10);
    let _ = app.update(Message::Edit(Action::Scroll { lines: 25 }));
    assert_eq!(app.view_top.0, 35);
    // El cursor del usuario no se movió por el sondeo.
    assert_eq!(editor::cursor_pos(&app.content), (0, 0));
}

#[test]
fn view_top_travels_with_each_tab() {
    let (mut app, _) = App::new(None);
    open_long_doc(&mut app);
    let _ = app.update(Message::Edit(Action::Scroll { lines: 20 }));
    let a = app.doc.id;
    let _ = open_pinned_doc(&mut app, "b.md", "corto");
    assert_eq!(app.view_top.0, 0);
    let _ = app.update(Message::SelectTab(a));
    assert_eq!(app.view_top.0, 20);
}

#[test]
fn measured_layout_is_kept_only_for_the_active_document_and_reset_on_tab_switch() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    let layout = preview_sync::Layout {
        tops: vec![0.0],
        content_height: 100.0,
        viewport_height: 50.0,
    };
    let _ = app.update(Message::PreviewMeasured(a, layout.clone()));
    assert_eq!(app.preview_layout, layout);
    // Una medida de otro documento se ignora.
    let _ = app.update(Message::PreviewMeasured(
        DocumentId::next(),
        preview_sync::Layout::default(),
    ));
    assert_eq!(app.preview_layout, layout);
    let b = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::SelectTab(a));
    assert_eq!(
        app.preview_layout,
        preview_sync::Layout::default(),
        "se vuelve a medir"
    );
    let _ = b;
}

#[test]
fn sync_toggle_defaults_to_on_and_can_be_turned_off() {
    // AC2
    let (mut app, _) = App::new(None);
    assert_eq!(app.sync, SyncScroll::On);
    let _ = app.update(Message::SetSyncScroll(false));
    assert_eq!(app.sync, SyncScroll::Off);
    let _ = app.update(Message::SetSyncScroll(true));
    assert_eq!(app.sync, SyncScroll::On);
}

#[test]
fn parsed_blocks_are_stored_per_document() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "# Uno\n\ntexto");
    deliver_preview(&mut app);
    assert_eq!(app.blocks.len(), 2);
    let _ = open_pinned_doc(&mut app, "b.md", "solo un párrafo");
    deliver_preview(&mut app);
    assert_eq!(app.blocks.len(), 1);
    let _ = app.update(Message::SelectTab(a));
    assert_eq!(app.blocks.len(), 2);
}

// ------------------------------------------------- US-3.4: buscar y reemplazar

fn open_find_with(app: &mut App, text: &str, pattern: &str) {
    open_with(app, text);
    let _ = app.update(Message::Shortcut(ShortcutAction::Find));
    let _ = app.update(Message::FindQueryChanged(pattern.to_owned()));
}

fn selected_text(app: &App) -> Option<String> {
    app.content.selection()
}

#[test]
fn find_shortcut_opens_the_bar_and_selects_the_first_match() {
    // AC1
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "uno dos uno tres uno", "uno");
    assert!(app.find.open && !app.find.replace_mode);
    assert_eq!(app.find.matches.len(), 3);
    assert_eq!(app.find.current, Some(0));
    assert_eq!(selected_text(&app).as_deref(), Some("uno"));
    assert_eq!(app.find.summary(), "1 de 3");
}

#[test]
fn next_and_previous_wrap_around() {
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "a-a-a", "a");
    let _ = app.update(Message::FindNext);
    assert_eq!(app.find.current, Some(1));
    let _ = app.update(Message::FindNext);
    let _ = app.update(Message::FindNext);
    assert_eq!(app.find.current, Some(0), "vuelve al principio");
    let _ = app.update(Message::FindPrev);
    assert_eq!(app.find.current, Some(2), "y hacia atrás al final");
}

#[test]
fn options_change_the_matches() {
    // AC2
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "Casa casa casas", "casa");
    assert_eq!(app.find.matches.len(), 3);
    let _ = app.update(Message::FindOptionToggled(FindOption::CaseSensitive));
    assert_eq!(app.find.matches.len(), 2);
    let _ = app.update(Message::FindOptionToggled(FindOption::WholeWord));
    assert_eq!(app.find.matches.len(), 1);
    let _ = app.update(Message::FindOptionToggled(FindOption::CaseSensitive));
    let _ = app.update(Message::FindOptionToggled(FindOption::Regex));
    let _ = app.update(Message::FindQueryChanged(r"cas\w".to_owned()));
    // Con «palabra completa» todavía activa, `cas\w` solo encaja con «Casa» y «casa».
    assert_eq!(app.find.matches.len(), 2);
}

#[test]
fn invalid_regex_shows_an_error_and_no_matches() {
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "texto", "(x");
    let _ = app.update(Message::FindOptionToggled(FindOption::Regex));
    assert!(app.find.error.is_some());
    assert_eq!(app.find.matches, Vec::<std::ops::Range<usize>>::new());
    let _ = app.update(Message::FindQueryChanged("texto".to_owned()));
    assert!(app.find.error.is_none());
    assert_eq!(app.find.matches.len(), 1);
}

#[test]
fn replace_one_replaces_the_current_match_and_moves_to_the_next() {
    // AC1: reemplazar uno.
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "uno dos uno", "uno");
    let _ = app.update(Message::Shortcut(ShortcutAction::Replace));
    assert!(app.find.replace_mode);
    let _ = app.update(Message::ReplaceChanged("1".to_owned()));
    let _ = app.update(Message::ReplaceOne);
    assert_eq!(app.content.text(), "1 dos uno");
    assert_eq!(app.title(), "● a.md — RustMDX");
    assert_eq!(app.find.matches.len(), 1);
    assert_eq!(
        selected_text(&app).as_deref(),
        Some("uno"),
        "salta a la siguiente"
    );
    let _ = app.update(Message::ReplaceOne);
    assert_eq!(app.content.text(), "1 dos 1");
    assert_eq!(app.find.summary(), "Sin resultados");
}

#[test]
fn replace_all_is_a_single_undo_step_and_reports_the_count() {
    // AC1: reemplazar todos.
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "uno dos uno tres uno", "uno");
    let _ = app.update(Message::Shortcut(ShortcutAction::Replace));
    let _ = app.update(Message::ReplaceChanged("1".to_owned()));
    let _ = app.update(Message::ReplaceAll);
    assert_eq!(app.content.text(), "1 dos 1 tres 1");
    assert_eq!(app.find.notice.as_deref(), Some("3 reemplazos"));
    let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
    assert_eq!(
        app.content.text(),
        "uno dos uno tres uno",
        "un solo deshacer lo revierte"
    );
}

#[test]
fn regex_replacement_expands_capture_groups() {
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "ana@casa luis@mar", r"(\w+)@(\w+)");
    let _ = app.update(Message::FindOptionToggled(FindOption::Regex));
    let _ = app.update(Message::Shortcut(ShortcutAction::Replace));
    let _ = app.update(Message::ReplaceChanged("$2:$1".to_owned()));
    let _ = app.update(Message::ReplaceAll);
    assert_eq!(app.content.text(), "casa:ana mar:luis");
}

#[test]
fn replacing_with_an_empty_string_deletes_the_matches() {
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "a-b-a", "a");
    let _ = app.update(Message::Shortcut(ShortcutAction::Replace));
    let _ = app.update(Message::ReplaceAll);
    assert_eq!(app.content.text(), "-b-");
}

#[test]
fn matches_follow_the_text_while_the_bar_is_open() {
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "uno", "uno");
    assert_eq!(app.find.matches.len(), 1);
    // El primer «uno» quedó seleccionado: se lleva el cursor al final antes de escribir.
    let _ = app.update(Message::Edit(Action::Move(
        text_editor::Motion::DocumentEnd,
    )));
    type_text(&mut app, " uno");
    assert_eq!(app.find.matches.len(), 2);
}

#[test]
fn escape_closes_the_bar_and_the_find_shortcut_prefills_from_the_selection() {
    let (mut app, _) = App::new(None);
    open_find_with(&mut app, "uno dos", "uno");
    let _ = app.update(Message::EscapePressed);
    assert!(!app.find.open);
    assert_eq!(app.find.matches, Vec::<std::ops::Range<usize>>::new());
    // Con «dos» seleccionado, Ctrl+F lo usa como patrón.
    app.content.move_to(text_editor::Cursor {
        position: text_editor::Position { line: 0, column: 7 },
        selection: Some(text_editor::Position { line: 0, column: 4 }),
    });
    let _ = app.update(Message::Shortcut(ShortcutAction::Find));
    assert_eq!(app.find.query.pattern, "dos");
}

#[test]
fn find_matches_are_recomputed_per_tab() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "x x x");
    let _ = app.update(Message::Shortcut(ShortcutAction::Find));
    let _ = app.update(Message::FindQueryChanged("x".to_owned()));
    assert_eq!(app.find.matches.len(), 3);
    let _ = open_pinned_doc(&mut app, "b.md", "x");
    assert_eq!(
        app.find.matches.len(),
        1,
        "se recalculan con el documento nuevo"
    );
    let _ = app.update(Message::SelectTab(a));
    assert_eq!(app.find.matches.len(), 3);
}

fn ws_with_files() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.md"), "uno\ntema aquí\n").unwrap();
    std::fs::write(dir.path().join("b.md"), "tema\n").unwrap();
    let ws = Workspace::open(dir.path(), TreeFilters::default()).unwrap();
    (dir, ws)
}

#[test]
fn workspace_search_shortcut_shows_the_search_panel() {
    // AC3
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::Shortcut(ShortcutAction::FindInWorkspace));
    assert_eq!(app.sidebar, Sidebar::Search);
}

#[test]
fn workspace_search_needs_an_open_folder_and_ignores_empty_queries() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::WsQueryChanged("tema".to_owned()));
    let _ = app.update(Message::WsSearch);
    assert!(!app.ws_search.running, "sin carpeta no hay nada que buscar");
    let (_dir, ws) = ws_with_files();
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    let _ = app.update(Message::WsQueryChanged(String::new()));
    let _ = app.update(Message::WsSearch);
    assert!(!app.ws_search.running);
}

#[test]
fn workspace_search_runs_stores_results_and_discards_stale_ones() {
    let (mut app, _) = App::new(None);
    let (dir, ws) = ws_with_files();
    let root = ws.root.clone();
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    let _ = app.update(Message::WsQueryChanged("tema".to_owned()));
    let _ = app.update(Message::WsSearch);
    assert!(app.ws_search.running);
    let first_gen = app.ws_search.generation;
    // Una segunda búsqueda cancela la primera.
    let first_cancel = app.ws_search.cancel.clone().unwrap();
    let _ = app.update(Message::WsSearch);
    assert!(first_cancel.load(Ordering::Relaxed));
    assert_eq!(app.ws_search.generation, first_gen + 1);

    let found =
        mdx_core::search::search_workspace(&root, &app.ws_search.query, &AtomicBool::new(false))
            .unwrap();
    let _ = app.update(Message::WsDone(first_gen, Ok(found.clone())));
    assert!(
        app.ws_search.result.is_none(),
        "la respuesta vieja se descarta"
    );
    let _ = app.update(Message::WsDone(app.ws_search.generation, Ok(found)));
    assert!(!app.ws_search.running);
    let result = app.ws_search.result.as_ref().unwrap();
    assert_eq!(result.matches.len(), 2);
    assert_eq!(
        crate::search::result_summary(result),
        "2 resultados en 2 archivos"
    );
    drop(dir);
}

#[test]
fn cancelling_a_workspace_search_raises_the_flag() {
    // AC3: cancelable.
    let (mut app, _) = App::new(None);
    let (_dir, ws) = ws_with_files();
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    let _ = app.update(Message::WsQueryChanged("tema".to_owned()));
    let _ = app.update(Message::WsSearch);
    let flag = app.ws_search.cancel.clone().unwrap();
    let _ = app.update(Message::WsCancel);
    assert!(flag.load(Ordering::Relaxed));
}

#[test]
fn clicking_a_result_in_an_open_file_selects_the_match() {
    // AC3: clic abre y posiciona.
    let (mut app, _) = App::new(None);
    let path = open_with(&mut app, "uno\ntema aquí\nfin");
    let _ = app.update(Message::OpenResult(path, 1, 0..4));
    assert_eq!(selected_text(&app).as_deref(), Some("tema"));
    assert_eq!(editor::cursor_pos(&app.content), (1, 4));
}

#[test]
fn clicking_a_result_in_a_closed_file_opens_it_then_selects_the_match() {
    let (mut app, _) = App::new(None);
    let path = PathBuf::from("/proyecto/otro.md");
    let _ = app.update(Message::OpenResult(path.clone(), 1, 3..7));
    assert!(app.pending_goto.is_some());
    let doc = Document::from_bytes(&path, "uno\nxx tema yy".as_bytes(), None).unwrap();
    let _ = app.update(Message::DocumentLoaded(Ok(doc)));
    assert!(app.pending_goto.is_none());
    assert_eq!(app.doc.path.as_deref(), Some(path.as_path()));
    assert_eq!(selected_text(&app).as_deref(), Some("tema"));
}

#[test]
fn a_stale_pending_position_is_dropped_when_another_file_opens_or_loading_fails() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::OpenResult(
        PathBuf::from("/proyecto/x.md"),
        3,
        0..1,
    ));
    let _ = app.update(Message::DocumentLoaded(Err(CoreError::NotFound(
        PathBuf::from("/proyecto/x.md"),
    ))));
    assert!(app.pending_goto.is_none());
    // Coordenadas fuera de rango no provocan pánico (documento más corto que el resultado).
    let path = open_with(&mut app, "corto");
    let _ = app.update(Message::OpenResult(path, 99, 50..60));
    assert!(
        editor::cursor_pos(&app.content).0 == 0,
        "se acota a la última línea"
    );
}

// ------------------------------------------------ US-3.5: autoguardado y sesión

use mdx_core::config::{ConfigWarning, LoadedConfig, ThemeSetting};
use mdx_core::session::{Boot, RestorePlan, Session};

fn with_autosave(app: &mut App, on: bool) {
    app.config.files.autosave = on;
    app.config.files.autosave_delay_ms = 2000;
}

/// Abre `a.md` (con ruta) y lo deja con cambios sin guardar.
fn dirty_file_doc(app: &mut App, name: &str) -> DocumentId {
    let id = open_pinned_doc(app, name, "base");
    type_char(app, 'X');
    id
}

fn after(app: &App, ms: u64) -> Instant {
    app.last_change + Duration::from_millis(ms)
}

#[test]
fn autosave_is_off_by_default() {
    // AC1
    let (mut app, _) = App::new(None);
    assert!(!app.config.files.autosave);
    dirty_file_doc(&mut app, "a.md");
    assert!(app.autosave_candidates(after(&app, 60_000)).is_empty());
    assert!(!app.autosave_armed());
}

#[test]
fn autosave_waits_for_the_delay_after_the_last_change() {
    // AC1: 2 s de inactividad.
    let (mut app, _) = App::new(None);
    with_autosave(&mut app, true);
    let id = dirty_file_doc(&mut app, "a.md");
    assert!(app.autosave_armed());
    assert!(app.autosave_candidates(after(&app, 1999)).is_empty());
    assert_eq!(app.autosave_candidates(after(&app, 2000)), [id]);
    // Una nueva pulsación reinicia la espera.
    type_char(&mut app, 'Y');
    assert!(app.autosave_candidates(after(&app, 1500)).is_empty());
}

#[test]
fn autosave_only_applies_to_saved_dirty_documents_without_conflicts() {
    // AC1: solo con ruta; nunca pisa cambios externos ni resucita archivos borrados.
    let (mut app, _) = App::new(None);
    with_autosave(&mut app, true);
    // Sin título: sin ruta → nunca.
    type_char(&mut app, 'u');
    assert!(app.autosave_candidates(after(&app, 60_000)).is_empty());
    // Con ruta pero limpio.
    let clean = open_pinned_doc(&mut app, "limpio.md", "texto");
    assert!(
        !app.autosave_candidates(after(&app, 60_000))
            .contains(&clean)
    );
    // Con conflicto con el disco.
    let conflict = dirty_file_doc(&mut app, "conflicto.md");
    app.doc.disk_state = DiskState::ChangedExternally;
    assert!(app.autosave_candidates(after(&app, 60_000)).is_empty());
    app.doc.disk_state = DiskState::Deleted;
    assert!(app.autosave_candidates(after(&app, 60_000)).is_empty());
    app.doc.disk_state = DiskState::InSync;
    assert_eq!(app.autosave_candidates(after(&app, 60_000)), [conflict]);
}

#[test]
fn autosave_reaches_background_tabs_and_avoids_double_saves() {
    let (mut app, _) = App::new(None);
    with_autosave(&mut app, true);
    let a = dirty_file_doc(&mut app, "a.md");
    let b = open_pinned_doc(&mut app, "b.md", "dos");
    let now = app.last_change + Duration::from_secs(60);
    assert_eq!(
        app.autosave_candidates(now),
        [a],
        "a está en segundo plano y con cambios"
    );
    let _ = app.update(Message::AutosaveTick(now));
    assert_eq!(app.doc.id, b, "la pestaña activa no cambia");
    assert!(app.autosave_inflight.contains(&a));
    assert!(
        app.autosave_candidates(now).is_empty(),
        "ya hay un guardado en curso"
    );
    // Al terminar se libera.
    let info = SavedInfo {
        path: PathBuf::from("/proyecto/a.md"),
        saved_hash: mdx_core::document::hash_text("Xbase"),
        mtime: None,
    };
    let _ = app.update(Message::Saved(a, Ok(info)));
    assert!(!app.autosave_inflight.contains(&a));
    assert!(app.first_dirty().is_none());
}

#[test]
fn a_failed_autosave_is_not_retried_immediately() {
    let (mut app, _) = App::new(None);
    with_autosave(&mut app, true);
    let a = dirty_file_doc(&mut app, "a.md");
    let now = app.last_change + Duration::from_secs(60);
    let _ = app.update(Message::AutosaveTick(now));
    let _ = app.update(Message::Saved(
        a,
        Err(CoreError::PermissionDenied(PathBuf::from("/x"))),
    ));
    assert!(app.status.as_ref().is_some_and(|s| s.is_error));
    assert!(
        app.autosave_candidates(Instant::now()).is_empty(),
        "reinicia la espera"
    );
}

#[test]
fn config_loaded_enables_autosave_and_reports_warnings() {
    let (mut app, _) = App::new(None);
    let mut loaded = LoadedConfig::default();
    loaded.config.files.autosave = true;
    loaded.config.files.autosave_delay_ms = 1000;
    let _ = app.update(Message::ConfigLoaded(loaded));
    assert!(app.config.files.autosave && app.config.files.autosave_delay_ms == 1000);
    let bad = LoadedConfig {
        warnings: vec![ConfigWarning::Invalid("línea 3".to_owned())],
        ..LoadedConfig::default()
    };
    let _ = app.update(Message::ConfigLoaded(bad));
    assert!(
        !app.config.files.autosave,
        "se usan los valores por defecto"
    );
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.is_error && s.text.contains("línea 3"))
    );
}

#[test]
fn snapshot_captures_workspace_tabs_active_tab_and_layout() {
    // AC2
    let (mut app, _) = App::new(None);
    let (_dir, ws) = ws_with_files();
    let root = ws.root.clone();
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    open_pinned_doc(&mut app, "a.md", "uno");
    let b = open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::Shortcut(ShortcutAction::New)); // «Sin título»: no se persiste
    let _ = app.update(Message::SelectTab(b));
    let s = app.session_snapshot();
    assert_eq!(s.workspace, Some(root.clone()));
    assert_eq!(
        s.open_files,
        [
            PathBuf::from("/proyecto/a.md"),
            PathBuf::from("/proyecto/b.md")
        ]
    );
    assert_eq!(s.active_tab, 1);
    assert!(s.recent.folders.contains(&root));
    assert!((s.layout.explorer_ratio - 0.2).abs() < 1e-4);
    assert!((s.layout.editor_ratio - 0.4).abs() < 1e-4);
}

#[test]
fn signature_changes_with_tabs_and_recents_but_not_with_typing() {
    // AC2: se guarda al cambiar la sesión, no en cada pulsación.
    let (mut app, _) = App::new(None);
    let initial = app.session_signature();
    open_pinned_doc(&mut app, "a.md", "uno");
    let with_tab = app.session_signature();
    assert_ne!(initial, with_tab);
    type_char(&mut app, 'x');
    assert_eq!(app.session_signature(), with_tab);
    let id = app.doc.id;
    let _ = app.update(Message::GuardAnswered(
        PendingAction::CloseTab(id),
        GuardChoice::Discard,
    ));
    assert_ne!(app.session_signature(), with_tab);
}

#[test]
fn nothing_is_saved_before_the_previous_session_has_been_read() {
    let (mut app, _) = App::new(None);
    assert!(!app.can_save_session());
    let _ = app.update(Message::SessionBooted(Box::default()));
    assert_eq!(app.can_save_session(), app.sess.files.is_some());
}

fn boot_with(app: &mut App, session: Session, plan: RestorePlan, docs: Vec<Document>) {
    let boot = Boot {
        session,
        plan,
        documents: docs,
        unusable: false,
    };
    let _ = app.update(Message::SessionBooted(Box::new(boot)));
}

fn doc_at(path: &str, text: &str) -> Document {
    Document::from_bytes(Path::new(path), text.as_bytes(), None).unwrap()
}

#[test]
fn restoring_a_session_reopens_tabs_in_order_with_the_saved_active_tab_and_panes() {
    // AC3
    let (mut app, _) = App::new(None);
    let session = Session {
        layout: mdx_core::session::Layout {
            explorer_ratio: 0.3,
            editor_ratio: 0.35,
        },
        ..Session::default()
    };
    let plan = RestorePlan {
        files: vec![PathBuf::from("/p/a.md"), PathBuf::from("/p/b.md")],
        active: Some(PathBuf::from("/p/a.md")),
        ..RestorePlan::default()
    };
    boot_with(
        &mut app,
        session,
        plan,
        vec![doc_at("/p/a.md", "A"), doc_at("/p/b.md", "B")],
    );
    assert_eq!(tab_names(&app), ["a.md", "b.md"]);
    assert_eq!(
        app.doc.path.as_deref(),
        Some(Path::new("/p/a.md")),
        "pestaña activa"
    );
    assert_eq!(
        unpinned_tabs(&app),
        0,
        "las pestañas restauradas quedan fijas"
    );
    let (explorer, inner) = crate::layout::current_ratios(&app.panes).unwrap();
    assert!((explorer - 0.3).abs() < 1e-4);
    assert!((inner.unwrap() - 0.35 / 0.7).abs() < 1e-3);
}

fn saved_plan() -> RestorePlan {
    RestorePlan {
        files: vec![PathBuf::from("/p/a.md"), PathBuf::from("/p/b.md")],
        active: Some(PathBuf::from("/p/a.md")),
        ..RestorePlan::default()
    }
}

fn saved_docs() -> Vec<Document> {
    vec![doc_at("/p/a.md", "A"), doc_at("/p/b.md", "B")]
}

/// `App::new` con un argumento: solo difiere del arranque normal si hay directorio de estado.
fn app_with_cli(arg: &str) -> App {
    let (mut app, _) = App::new(Some(PathBuf::from(arg)));
    app.sess.files = Some(mdx_core::paths::AppFiles {
        config_file: PathBuf::from("/x/config.toml"),
        session_file: PathBuf::from("/x/session.toml"),
    });
    app.sess.cli = Some(CliTarget::File(PathBuf::from(arg)));
    app
}

#[test]
fn a_cli_file_is_added_on_top_of_the_restored_session_and_becomes_active() {
    // AC3 (decisión del propietario): con argumento también se restaura la sesión.
    let mut app = app_with_cli("/p/nuevo.md");
    boot_with(&mut app, Session::default(), saved_plan(), saved_docs());
    assert_eq!(tab_names(&app), ["a.md", "b.md"], "primero lo restaurado");
    assert!(app.sess.cli.is_none(), "el argumento ya se consumió");
    // El archivo del argumento llega después de leerse y queda activo.
    let _ = app.update(Message::DocumentLoaded(Ok(doc_at("/p/nuevo.md", "N"))));
    assert_eq!(tab_names(&app), ["a.md", "b.md", "nuevo.md"]);
    assert_eq!(app.doc.path.as_deref(), Some(Path::new("/p/nuevo.md")));
}

#[test]
fn a_cli_file_already_in_the_session_just_becomes_the_active_tab() {
    let mut app = app_with_cli("/p/b.md");
    boot_with(&mut app, Session::default(), saved_plan(), saved_docs());
    assert_eq!(tab_names(&app), ["a.md", "b.md"], "sin duplicados");
    assert_eq!(app.doc.path.as_deref(), Some(Path::new("/p/b.md")));
}

#[test]
fn a_cli_folder_replaces_the_session_workspace() {
    let (mut app, _) = App::new(None);
    app.sess.cli = Some(CliTarget::Folder(PathBuf::from("/otra")));
    let plan = RestorePlan {
        workspace: Some(PathBuf::from("/guardada")),
        expanded_dirs: vec![PathBuf::from("/guardada/docs")],
        ..RestorePlan::default()
    };
    boot_with(&mut app, Session::default(), plan, Vec::new());
    assert!(
        app.sess.pending_expand.is_empty(),
        "no se expanden carpetas de otra carpeta"
    );
    assert!(app.sess.cli.is_none());
}

#[test]
fn a_session_without_cli_still_restores_everything() {
    let (mut app, _) = App::new(None);
    let mut session = Session::default();
    session.recent.files = vec![PathBuf::from("/p/a.md")];
    boot_with(&mut app, session, saved_plan(), saved_docs());
    assert_eq!(tab_names(&app), ["a.md", "b.md"]);
    assert_eq!(app.sess.data.recent.files, [PathBuf::from("/p/a.md")]);
}

#[test]
fn an_unusable_previous_session_only_warns() {
    let (mut app, _) = App::new(None);
    let boot = Boot {
        unusable: true,
        ..Boot::default()
    };
    let _ = app.update(Message::SessionBooted(Box::new(boot)));
    assert!(app.status.as_ref().is_some_and(|s| s.is_error));
    assert_eq!(app.tabs.tabs().len(), 1);
}

#[test]
fn opening_files_and_folders_feeds_the_recents_list() {
    // AC4
    let (mut app, _) = App::new(None);
    open_pinned_doc(&mut app, "a.md", "uno");
    assert_eq!(
        app.sess.data.recent.files,
        [PathBuf::from("/proyecto/a.md")]
    );
    let (_dir, ws) = ws_with_files();
    let root = ws.root.clone();
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    assert_eq!(app.sess.data.recent.folders, [root]);
    // Reabrir un archivo ya reciente lo sube al principio sin duplicarlo.
    open_pinned_doc(&mut app, "b.md", "dos");
    let _ = app.update(Message::OpenRecentFile(PathBuf::from("/proyecto/a.md")));
    assert_eq!(
        app.sess.data.recent.files[0],
        PathBuf::from("/proyecto/b.md")
    );
}

#[test]
fn recents_are_capped_at_ten_files() {
    let (mut app, _) = App::new(None);
    for i in 0..14 {
        open_pinned_doc(&mut app, &format!("f{i}.md"), "x");
    }
    assert_eq!(app.sess.data.recent.files.len(), 10);
    assert_eq!(
        app.sess.data.recent.files[0],
        PathBuf::from("/proyecto/f13.md")
    );
}

#[test]
fn expanded_directories_from_the_session_open_as_their_parents_load() {
    // AC3: carpetas expandidas, de afuera hacia adentro.
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
    std::fs::write(dir.path().join("a/b/n.md"), "x").unwrap();
    let ws = Workspace::open(dir.path(), TreeFilters::default()).unwrap();
    let (a, b) = (ws.root.join("a"), ws.root.join("a/b"));
    let (mut app, _) = App::new(None);
    app.sess.pending_expand = vec![b.clone(), a.clone()];
    let _ = app.update(Message::WorkspaceOpened(Ok(ws)));
    let expanded = |app: &App, p: &Path| {
        matches!(
            app.explorer
                .workspace
                .as_ref()
                .unwrap()
                .node(p)
                .map(|n| &n.kind),
            Some(mdx_core::workspace::NodeKind::Dir { expanded: true, .. })
        )
    };
    assert!(expanded(&app, &a), "a no necesita a nadie");
    assert!(
        app.sess.pending_expand.contains(&b),
        "b espera a que cargue a"
    );
    let kids = workspace::load_children(&a, &TreeFilters::default());
    let _ = app.update(Message::DirLoaded(a.clone(), kids));
    assert!(expanded(&app, &b));
    assert!(app.sess.pending_expand.is_empty());
}

#[test]
fn autosave_due_is_a_simple_threshold() {
    let t0 = Instant::now();
    let d = Duration::from_millis(2000);
    assert!(!session_ops::autosave_due(
        t0,
        t0 + Duration::from_millis(1999),
        d
    ));
    assert!(session_ops::autosave_due(t0, t0 + d, d));
    assert!(
        !session_ops::autosave_due(t0 + d, t0, d),
        "un instante anterior no cuenta"
    );
}

// ----------------------------------------------------------- US-4.2: configuración

fn app_with_config_dir() -> (App, PathBuf) {
    let (mut app, _) = App::new(None);
    let config_file = PathBuf::from("/cfg/mdx/config.toml");
    app.sess.files = Some(mdx_core::paths::AppFiles {
        config_file: config_file.clone(),
        session_file: PathBuf::from("/st/session.toml"),
    });
    (app, config_file)
}

fn loaded(text: &str) -> LoadedConfig {
    mdx_core::config::Config::parse(text).unwrap()
}

#[test]
fn loaded_config_drives_the_explorer_filters_preview_and_editor_settings() {
    // AC2
    let (mut app, _) = App::new(None);
    let text = "[files]\nrespect_gitignore = false\nshow_hidden = true\nonly_markdown = false\n\
                [preview]\nsync_scroll = false\nvisible = false\n\
                [editor]\nfont_size = 20\nline_wrap = false\nfont_family = \"Fuente Que No Existe\"\n";
    let _ = app.update(Message::ConfigLoaded(loaded(text)));
    let f = app.explorer.filters;
    assert!(!f.respect_gitignore && f.show_hidden && !f.only_markdown);
    assert_eq!(app.sync, SyncScroll::Off);
    assert!(!app.preview_visible);
    assert_eq!(
        app.panes.len(),
        2,
        "sin vista previa solo quedan dos paneles"
    );
    assert_eq!(app.config.editor.font_size, 20);
    assert!(!app.config.editor.line_wrap);
    assert_eq!(
        app.editor_font,
        iced::Font::MONOSPACE,
        "fuente inexistente → monoespaciada"
    );
}

#[test]
fn the_default_config_changes_nothing_at_startup() {
    let (mut app, _) = App::new(None);
    let before = app.explorer.filters;
    let _ = app.update(Message::ConfigLoaded(LoadedConfig::default()));
    assert_eq!(app.explorer.filters, before);
    assert!(app.preview_visible && app.sync == SyncScroll::On);
    assert!(app.status.is_none(), "sin avisos");
}

#[test]
fn clamped_values_warn_unknown_keys_do_not() {
    // AC4
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::ConfigLoaded(loaded("futuro = 1\n")));
    assert!(
        app.status.is_none(),
        "una clave desconocida solo se registra"
    );
    let _ = app.update(Message::ConfigLoaded(loaded("[editor]\nfont_size = 999\n")));
    assert_eq!(app.config.editor.font_size, 48);
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.is_error && s.text.contains("font_size"))
    );
}

#[test]
fn saving_config_toml_reloads_it_and_other_files_do_not() {
    // AC3: «se aplica al guardar».
    let (app, config_file) = app_with_config_dir();
    assert!(app.is_config_file(&config_file));
    assert!(!app.is_config_file(Path::new("/cfg/mdx/otro.toml")));
    assert!(!app.is_config_file(Path::new("/proyecto/a.md")));
}

#[test]
fn a_reloaded_valid_config_is_confirmed_and_an_invalid_one_falls_back_to_defaults() {
    // AC3 + AC4
    let (mut app, _) = App::new(None);
    let mut custom = loaded("[files]\nautosave = true\n");
    custom.config.editor.font_size = 22;
    let _ = app.update(Message::ConfigReloaded(custom));
    assert!(app.config.files.autosave);
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Configuración aplicada")
    );
    // El archivo quedó inválido: valores por defecto y aviso; la app sigue viva.
    let bad = LoadedConfig {
        warnings: vec![ConfigWarning::Invalid("línea 2".to_owned())],
        ..LoadedConfig::default()
    };
    let _ = app.update(Message::ConfigReloaded(bad));
    assert!(!app.config.files.autosave && app.config.editor.font_size == 14);
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.is_error && s.text.contains("línea 2"))
    );
}

#[test]
fn the_config_shortcut_opens_the_file_in_a_pinned_tab() {
    // AC3
    let (mut app, config_file) = app_with_config_dir();
    let _ = app.update(Message::Shortcut(ShortcutAction::OpenConfig));
    let _ = app.update(Message::ConfigFileReady(Ok(config_file.clone())));
    assert_eq!(app.pin_on_load.as_deref(), Some(config_file.as_path()));
    let doc = Document::from_bytes(&config_file, b"[files]\nautosave = true\n", None).unwrap();
    let _ = app.update(Message::DocumentLoaded(Ok(doc)));
    assert_eq!(tab_names(&app), ["config.toml"]);
    assert_eq!(unpinned_tabs(&app), 0);
}

#[test]
fn the_config_shortcut_reports_when_there_is_no_config_directory() {
    let (mut app, _) = App::new(None);
    app.sess.files = None;
    let _ = app.update(Message::Shortcut(ShortcutAction::OpenConfig));
    assert!(app.status.as_ref().is_some_and(|s| s.is_error));
    let _ = app.update(Message::ConfigFileReady(Err(CoreError::PermissionDenied(
        PathBuf::from("/x"),
    ))));
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("Permiso"))
    );
}

// ------------------------------------------------------------ US-4.1: temas

#[test]
fn the_theme_defaults_to_the_system_mode_and_follows_its_changes() {
    // AC1
    let (mut app, _) = App::new(None);
    assert_eq!(app.config.appearance.theme, ThemeSetting::System);
    assert_eq!(app.theme(), Theme::Light, "sin información del SO: claro");
    let _ = app.update(Message::SystemTheme(iced::theme::Mode::Dark));
    assert_eq!(app.theme(), Theme::Dark);
    let _ = app.update(Message::SystemTheme(iced::theme::Mode::Light));
    assert_eq!(app.theme(), Theme::Light);
}

#[test]
fn an_explicit_theme_in_the_config_overrides_the_system() {
    // AC1: claro y oscuro fijos, aunque el SO diga otra cosa.
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::SystemTheme(iced::theme::Mode::Light));
    let _ = app.update(Message::ConfigLoaded(loaded(
        "[appearance]\ntheme = \"dark\"\n",
    )));
    assert_eq!(app.theme(), Theme::Dark);
    let _ = app.update(Message::SystemTheme(iced::theme::Mode::Dark));
    let _ = app.update(Message::ConfigReloaded(loaded(
        "[appearance]\ntheme = \"light\"\n",
    )));
    assert_eq!(
        app.theme(),
        Theme::Light,
        "cambia al guardar la configuración"
    );
    let _ = app.update(Message::ConfigReloaded(loaded(
        "[appearance]\ntheme = \"system\"\n",
    )));
    assert_eq!(app.theme(), Theme::Dark, "vuelve a seguir al sistema");
}

#[test]
fn an_invalid_theme_value_falls_back_to_the_system_default() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::SystemTheme(iced::theme::Mode::Dark));
    // TOML con un tema desconocido → configuración por defecto (INV-6) → sigue al sistema.
    assert!(mdx_core::config::Config::parse("[appearance]\ntheme = \"neón\"\n").is_err());
    let _ = app.update(Message::ConfigReloaded(LoadedConfig::default()));
    assert_eq!(app.theme(), Theme::Dark);
}

// ------------------------------------------------------------- US-4.5: zoom

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn percent(zoom: f32) -> u32 {
    (zoom * 100.0).round() as u32
}

fn zoom_percent(app: &App) -> u32 {
    percent(app.zoom)
}

#[test]
fn zoom_in_and_out_change_the_factor_in_tenths() {
    // AC1
    let (mut app, _) = App::new(None);
    assert_eq!(zoom_percent(&app), 100);
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    assert_eq!(zoom_percent(&app), 120);
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Zoom: 120 %")
    );
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomOut));
    assert_eq!(zoom_percent(&app), 110);
}

#[test]
fn zoom_stops_at_the_limits() {
    let (mut app, _) = App::new(None);
    for _ in 0..40 {
        let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    }
    assert_eq!(zoom_percent(&app), 300);
    for _ in 0..60 {
        let _ = app.update(Message::Shortcut(ShortcutAction::ZoomOut));
    }
    assert_eq!(zoom_percent(&app), 50);
}

#[test]
fn zoom_reset_returns_to_the_configured_zoom() {
    // AC1: Ctrl/Cmd+0.
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::ConfigLoaded(loaded("[appearance]\nzoom = 1.5\n")));
    assert_eq!(
        zoom_percent(&app),
        150,
        "el zoom inicial es el de config.toml"
    );
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomReset));
    assert_eq!(zoom_percent(&app), 150);
    let _ = app.update(Message::ConfigLoaded(LoadedConfig::default()));
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomReset));
    assert_eq!(zoom_percent(&app), 100);
}

#[test]
fn reloading_the_config_discards_the_keyboard_zoom() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    let _ = app.update(Message::ConfigReloaded(loaded(
        "[appearance]\nzoom = 2.0\n",
    )));
    assert_eq!(zoom_percent(&app), 200);
}

#[test]
fn the_zoom_is_saved_with_the_session_and_restored_over_the_config_value() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::ConfigLoaded(loaded("[appearance]\nzoom = 1.5\n")));
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    assert_eq!(app.session_snapshot().zoom.map(percent), Some(160));
    // Cambiar el zoom cuenta como cambio de sesión (se guarda en segundo plano).
    let before = app.session_signature();
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    assert_ne!(app.session_signature(), before);
    // Al arrancar de nuevo, la sesión prevalece sobre la configuración.
    let (mut next, _) = App::new(None);
    let _ = next.update(Message::ConfigLoaded(loaded("[appearance]\nzoom = 1.5\n")));
    let session = Session {
        zoom: Some(2.2),
        ..Session::default()
    };
    let _ = next.update(Message::SessionBooted(Box::new(Boot {
        session,
        ..Boot::default()
    })));
    assert_eq!(zoom_percent(&next), 220);
}

#[test]
fn zoom_changes_do_not_touch_the_document() {
    let (mut app, _) = App::new(None);
    open_with(&mut app, "texto");
    let _ = app.update(Message::Shortcut(ShortcutAction::ZoomIn));
    assert_eq!(app.content.text(), "texto");
    assert_eq!(app.title(), "a.md — RustMDX");
}

// ------------------------------------------------------- US-4.3: exportar HTML

#[test]
fn exporting_reports_the_written_path_or_the_error() {
    // AC1
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::HtmlExported(Ok(PathBuf::from("/tmp/doc.html"))));
    assert_eq!(
        app.status.as_ref().map(|s| (s.text.as_str(), s.is_error)),
        Some(("Exportado: /tmp/doc.html", false))
    );
    let _ = app.update(Message::HtmlExported(Err(CoreError::PermissionDenied(
        PathBuf::from("/x"),
    ))));
    assert!(app.status.as_ref().is_some_and(|s| s.is_error));
}

#[test]
fn cancelling_the_export_dialog_does_nothing_and_exporting_never_touches_the_document() {
    let (mut app, _) = App::new(None);
    open_with(&mut app, "# Título");
    type_char(&mut app, 'x');
    let before = (app.content.text(), app.dirty);
    let _ = app.update(Message::HtmlExportPicked(None));
    let _ = app.update(Message::ExportHtml);
    let _ = app.update(Message::Shortcut(ShortcutAction::ExportHtml));
    assert_eq!(
        (app.content.text(), app.dirty),
        before,
        "el documento sigue sin guardar y sin cambios"
    );
    assert!(app.status.as_ref().is_none_or(|s| !s.is_error));
}

// ----------------------------------------------------------- US-4.4: imágenes

#[test]
fn pasting_an_image_into_an_unsaved_document_asks_to_save_first() {
    // AC2: sin ruta no hay carpeta «junto al documento» donde guardar `assets/`.
    let (mut app, _) = App::new(None);
    let image = crate::clipboard::ClipboardImage {
        width: 1,
        height: 1,
        rgba: vec![0, 0, 0, 255],
    };
    let _ = app.update(Message::ClipboardImage(Some(image)));
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.is_error && s.text.contains("Guarde"))
    );
    assert_eq!(app.content.text(), "");
}

#[test]
fn a_saved_image_is_inserted_as_a_markdown_link_at_the_cursor() {
    // AC2
    let (mut app, _) = App::new(None);
    let _ = open_with(&mut app, "antes  después");
    app.content.move_to(text_editor::Cursor {
        position: text_editor::Position { line: 0, column: 6 },
        selection: None,
    });
    let saved = mdx_core::assets::SavedImage {
        path: PathBuf::from("/proyecto/assets/20261123-1.png"),
        relative: "assets/20261123-1.png".to_owned(),
    };
    let id = app.doc.id;
    let _ = app.update(Message::ImageSaved(id, Ok(saved)));
    assert_eq!(
        app.content.text(),
        "antes ![](assets/20261123-1.png) después"
    );
    assert_eq!(app.title(), "● a.md — RustMDX");
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("assets/20261123-1.png"))
    );
    // Y se deshace de un paso.
    let _ = app.update(Message::Shortcut(ShortcutAction::Undo));
    assert_eq!(app.content.text(), "antes  después");
}

#[test]
fn an_image_saved_for_a_background_tab_goes_into_that_tab() {
    let (mut app, _) = App::new(None);
    let a = open_pinned_doc(&mut app, "a.md", "uno");
    let b = open_pinned_doc(&mut app, "b.md", "dos");
    let saved = mdx_core::assets::SavedImage {
        path: PathBuf::from("/proyecto/assets/x.png"),
        relative: "assets/x.png".to_owned(),
    };
    let _ = app.update(Message::ImageSaved(a, Ok(saved)));
    assert_eq!(app.doc.id, b, "no cambia de pestaña");
    assert_eq!(app.content.text(), "dos");
    let _ = app.update(Message::SelectTab(a));
    assert!(app.content.text().contains("![](assets/x.png)"));
}

#[test]
fn a_failed_image_save_reports_the_error_and_leaves_the_text_alone() {
    let (mut app, _) = App::new(None);
    let _ = open_with(&mut app, "texto");
    let id = app.doc.id;
    let _ = app.update(Message::ImageSaved(
        id,
        Err(CoreError::PermissionDenied(PathBuf::from(
            "/proyecto/assets",
        ))),
    ));
    assert_eq!(app.content.text(), "texto");
    assert!(app.status.as_ref().is_some_and(|s| s.is_error));
}

#[test]
fn text_paste_normalises_line_endings_and_ignores_an_empty_clipboard() {
    let (mut app, _) = App::new(None);
    let _ = app.update(Message::PasteText(Some("uno\r\ndos\rtres".to_owned())));
    assert_eq!(app.content.text(), "uno\ndos\ntres");
    let before = app.content.text();
    let _ = app.update(Message::PasteText(None));
    let _ = app.update(Message::PasteText(Some(String::new())));
    assert_eq!(app.content.text(), before);
}
