//! Operaciones del explorador: árbol, menú contextual, operaciones de archivo y filtros.

use super::{
    App, ContextAction, CoreError, ExplorerKey, FilterKind, Message, Nav, OpOutcome, PROMPT_ID,
    Path, PathBuf, Prompt, PromptKind, Task, explorer, fileops, strings, workspace,
};

impl App {
    /// Mensajes del explorador (árbol, menú contextual, operaciones de archivo).
    pub(super) fn update_explorer(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FolderPicked(Some(path)) => Task::perform(
                workspace::open_async(path, self.explorer.filters),
                Message::WorkspaceOpened,
            ),
            Message::WorkspaceOpened(result) => {
                match result {
                    Ok(ws) => {
                        let msg = format!("{}: {}", strings::STATUS_FOLDER_OPENED, ws.tree.name);
                        self.explorer.workspace = Some(ws);
                        self.explorer.selected = None;
                        self.set_status(msg, false);
                    }
                    Err(err) => self.set_status(err.to_string(), true),
                }
                Task::none()
            }
            Message::ToggleDir(path) => self.toggle_dir(&path),
            Message::DirLoaded(path, result) => {
                if let Some(ws) = &mut self.explorer.workspace {
                    ws.set_children(&path, result);
                }
                Task::none()
            }
            Message::FileClicked(path) => self.open_from_explorer(path, false),
            Message::FilePinned(path) => self.open_from_explorer(path, true),
            Message::ExplorerKey(key) => self.explorer_key(key),
            Message::OpenContext(target) => {
                self.explorer.context = Some(target);
                self.explorer.prompt = None;
                Task::none()
            }
            Message::CloseContext => {
                self.explorer.context = None;
                Task::none()
            }
            Message::ContextAction(action) => self.context_action(action),
            Message::PromptChanged(value) => {
                if let Some(p) = &mut self.explorer.prompt {
                    p.value = value;
                    p.error = None;
                }
                Task::none()
            }
            Message::PromptSubmit => self.submit_prompt(),
            Message::PromptCancel => {
                self.explorer.prompt = None;
                Task::none()
            }
            Message::DeleteAnswered(path, true) => Task::perform(
                async move {
                    fileops::move_to_trash_async(path.clone())
                        .await
                        .map(|()| OpOutcome::Trashed(path))
                },
                Message::OpDone,
            ),
            Message::OpDone(result) => self.handle_op_done(result),
            Message::ToggleExplorerMenu => {
                self.explorer.menu_open = !self.explorer.menu_open;
                Task::none()
            }
            Message::ToggleFilter(kind) => self.toggle_filter(kind),
            _ => Task::none(),
        }
    }

    pub(super) fn context_action(&mut self, action: ContextAction) -> Task<Message> {
        let Some(target) = self.explorer.context.take() else {
            return Task::none();
        };
        let new_prompt = |kind, dir: PathBuf, value: String| Prompt {
            kind,
            target: dir,
            value,
            error: None,
        };
        let focus = || iced::widget::operation::focus(iced::widget::Id::new(PROMPT_ID));
        match action {
            ContextAction::NewFile | ContextAction::NewFolder => {
                let dir = if target.is_dir {
                    target.path
                } else {
                    target
                        .path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or(target.path)
                };
                let kind = if action == ContextAction::NewFile {
                    PromptKind::NewFile
                } else {
                    PromptKind::NewFolder
                };
                self.explorer.prompt = Some(new_prompt(kind, dir, String::new()));
                focus()
            }
            ContextAction::Rename => {
                let name = target
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.explorer.prompt = Some(new_prompt(PromptKind::Rename, target.path, name));
                focus()
            }
            ContextAction::Delete => {
                let name = target
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let path = target.path;
                Task::perform(ask_delete(name), move |yes| {
                    Message::DeleteAnswered(path.clone(), yes)
                })
            }
            ContextAction::Reveal => Task::perform(
                async move {
                    fileops::reveal_async(target.path)
                        .await
                        .map(|()| OpOutcome::Revealed)
                },
                Message::OpDone,
            ),
        }
    }

    pub(super) fn submit_prompt(&mut self) -> Task<Message> {
        let Some(prompt) = &mut self.explorer.prompt else {
            return Task::none();
        };
        // Validación en línea, sin tocar el disco (AC4).
        if let Err(err) = fileops::validate_name(&prompt.value) {
            prompt.error = Some(err.to_string());
            return Task::none();
        }
        let (target, value) = (prompt.target.clone(), prompt.value.clone());
        match prompt.kind {
            PromptKind::NewFile => Task::perform(
                async move {
                    fileops::create_file_async(target, value)
                        .await
                        .map(OpOutcome::Created)
                },
                Message::OpDone,
            ),
            PromptKind::NewFolder => Task::perform(
                async move {
                    fileops::create_dir_async(target, value)
                        .await
                        .map(OpOutcome::Created)
                },
                Message::OpDone,
            ),
            PromptKind::Rename => Task::perform(
                async move {
                    fileops::rename_async(target.clone(), value)
                        .await
                        .map(|to| OpOutcome::Renamed { from: target, to })
                },
                Message::OpDone,
            ),
        }
    }

    pub(super) fn handle_op_done(&mut self, result: Result<OpOutcome, CoreError>) -> Task<Message> {
        match result {
            Ok(OpOutcome::Created(path)) => {
                self.explorer.prompt = None;
                self.explorer.selected = Some(path.clone());
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                self.set_status(
                    format!("{}: {}", strings::STATUS_CREATED, name.unwrap_or_default()),
                    false,
                );
                self.reload_parent(&path, true)
            }
            Ok(OpOutcome::Renamed { from, to }) => {
                self.explorer.prompt = None;
                // AC3: el documento abierto (o uno dentro de la carpeta renombrada) sigue su ruta.
                self.open_docs_mut(|doc, _| {
                    if let Some(new_path) = doc
                        .path
                        .as_deref()
                        .and_then(|p| fileops::rebase_path(p, &from, &to))
                    {
                        doc.path = Some(new_path);
                    }
                });
                self.explorer.selected = Some(to.clone());
                let name = to.file_name().map(|n| n.to_string_lossy().into_owned());
                self.set_status(
                    format!("{}: {}", strings::STATUS_RENAMED, name.unwrap_or_default()),
                    false,
                );
                self.reload_parent(&from, false)
            }
            Ok(OpOutcome::Trashed(path)) => {
                // El contenido se conserva en el editor para poder volver a guardarlo.
                self.open_docs_mut(|doc, _| {
                    if doc.path.as_deref().is_some_and(|p| p.starts_with(&path)) {
                        doc.disk_state = mdx_core::DiskState::Deleted;
                    }
                });
                if self.explorer.selected.as_deref() == Some(path.as_path()) {
                    self.explorer.selected = None;
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                self.set_status(
                    format!("{}: {}", strings::STATUS_TRASHED, name.unwrap_or_default()),
                    false,
                );
                self.reload_parent(&path, false)
            }
            Ok(OpOutcome::Revealed) => Task::none(),
            Err(err) => {
                // Con la entrada de nombre abierta el error se muestra en línea (AC4).
                if let Some(prompt) = &mut self.explorer.prompt {
                    prompt.error = Some(err.to_string());
                } else {
                    self.set_status(err.to_string(), true);
                }
                Task::none()
            }
        }
    }

    /// Recarga los hijos del directorio que contiene `path` (y lo expande si `expand`).
    pub(super) fn reload_parent(&mut self, path: &Path, expand: bool) -> Task<Message> {
        let filters = self.explorer.filters;
        let Some(parent) = path.parent().map(Path::to_path_buf) else {
            return Task::none();
        };
        if expand && let Some(ws) = &mut self.explorer.workspace {
            let _ = ws.begin_expand(&parent);
        }
        Task::perform(
            workspace::load_children_async(parent.clone(), filters),
            move |result| Message::DirLoaded(parent.clone(), result),
        )
    }

    /// Clic simple (pestaña preview) o doble clic (pestaña fija) sobre un archivo del árbol.
    pub(super) fn open_from_explorer(&mut self, path: PathBuf, pin: bool) -> Task<Message> {
        self.explorer.selected = Some(path.clone());
        self.open_path(path, pin)
    }

    pub(super) fn explorer_key(&mut self, key: ExplorerKey) -> Task<Message> {
        let Some(ws) = &self.explorer.workspace else {
            return Task::none();
        };
        let nav = explorer::navigate(&ws.visible_nodes(), self.explorer.selected.as_deref(), key);
        match nav {
            Nav::None => Task::none(),
            Nav::Select(path) => {
                self.explorer.selected = Some(path);
                Task::none()
            }
            Nav::Toggle(path) => self.toggle_dir(&path),
            Nav::Open(path) => self.open_from_explorer(path, false),
        }
    }

    pub(super) fn toggle_dir(&mut self, path: &Path) -> Task<Message> {
        let filters = self.explorer.filters;
        let Some(ws) = &mut self.explorer.workspace else {
            return Task::none();
        };
        let expanded = matches!(
            ws.node(path).map(|n| &n.kind),
            Some(mdx_core::workspace::NodeKind::Dir { expanded: true, .. })
        );
        if expanded {
            ws.collapse(path);
            return Task::none();
        }
        if ws.begin_expand(path) {
            let dir = path.to_path_buf();
            return Task::perform(
                workspace::load_children_async(dir.clone(), filters),
                move |result| Message::DirLoaded(dir.clone(), result),
            );
        }
        Task::none()
    }

    /// Cambia un filtro y recarga la raíz (el árbol vuelve a colapsarse).
    pub(super) fn toggle_filter(&mut self, kind: FilterKind) -> Task<Message> {
        let filters = &mut self.explorer.filters;
        match kind {
            FilterKind::Hidden => filters.show_hidden = !filters.show_hidden,
            FilterKind::Gitignore => filters.respect_gitignore = !filters.respect_gitignore,
            FilterKind::OnlyMarkdown => filters.only_markdown = !filters.only_markdown,
        }
        let Some(ws) = &self.explorer.workspace else {
            return Task::none();
        };
        Task::perform(
            workspace::open_async(ws.root.clone(), self.explorer.filters),
            Message::WorkspaceOpened,
        )
    }
}

pub(super) async fn ask_delete(name: String) -> bool {
    let result = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(strings::DELETE_TITLE)
        .set_description(strings::DELETE_BODY.replace("{name}", &name))
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            strings::DELETE_CONFIRM.to_owned(),
            strings::PROMPT_CANCEL.to_owned(),
        ))
        .show()
        .await;
    match result {
        rfd::MessageDialogResult::Ok => true,
        rfd::MessageDialogResult::Custom(label) => label == strings::DELETE_CONFIRM,
        _ => false,
    }
}

pub(super) async fn pick_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(strings::DIALOG_FOLDER_TITLE)
        .pick_folder()
        .await
        .map(|h| h.path().to_path_buf())
}
