use super::*;
use iced::widget::text_editor::{Action, Edit};

fn type_char(app: &mut App, c: char) {
    let _ = app.update(Message::Edit(Action::Edit(Edit::Insert(c))));
}

#[test]
fn starts_clean_and_untitled() {
    let (app, _) = App::new(None);
    assert_eq!(app.title(), "Sin título — Mdx");
}

#[test]
fn editing_marks_dirty_in_title() {
    // AC1
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    assert_eq!(app.title(), "● Sin título — Mdx");
}

#[test]
fn returning_to_saved_state_clears_indicator() {
    // AC3: borrar lo escrito devuelve el hash al del estado guardado.
    let (mut app, _) = App::new(None);
    type_char(&mut app, 'a');
    let _ = app.update(Message::Edit(Action::Edit(Edit::Backspace)));
    assert_eq!(app.title(), "Sin título — Mdx");
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
    assert_eq!(app.title(), "x.md — Mdx");
    type_char(&mut app, 'b');
    assert_eq!(app.title(), "● x.md — Mdx");
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
    assert_eq!(app.title(), "Sin título — Mdx");
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
    assert_eq!(app.title(), "Sin título — Mdx");
    let _ = app.update(Message::Shortcut(ShortcutAction::Redo));
    assert_eq!(app.content.text(), "hola");
    assert_eq!(app.title(), "● Sin título — Mdx");
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

/// Simula la llegada del resultado del parseo en segundo plano.
fn deliver_preview(app: &mut App) {
    let items = markdown::parse(&app.doc.text).collect();
    let outline = mdx_core::outline::extract(&app.doc.text);
    let _ = app.update(Message::PreviewParsed(
        app.doc.id,
        app.preview_gen,
        items,
        outline,
    ));
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
    let items = markdown::parse("# viejo").collect();
    let _ = app.update(Message::PreviewParsed(app.doc.id, stale, items, Vec::new()));
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
        "● b.md — Mdx",
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
    assert_eq!(app.title(), "a.md — Mdx");
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
    assert_eq!(app.title(), "a.md — Mdx");
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
    assert_eq!(app.title(), "● a.md — Mdx");
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
    assert_eq!(app.title(), "a.md (eliminado) — Mdx");
    assert_eq!(app.content.text(), "contenido");
    // Volver a guardar restituye el archivo y quita la marca.
    let info = SavedInfo {
        path: path.clone(),
        saved_hash: mdx_core::document::hash_text("contenido"),
        mtime: None,
    };
    let _ = app.update(Message::Saved(app.doc.id, Ok(info)));
    assert_eq!(app.title(), "a.md — Mdx");
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
    assert_eq!(app.title(), "● a.md — Mdx");
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
    let outline = mdx_core::outline::extract("# viejo");
    let _ = app.update(Message::PreviewParsed(
        app.doc.id,
        stale,
        Vec::new(),
        outline,
    ));
    assert_eq!(app.outline, Vec::new());
}
