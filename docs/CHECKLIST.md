# CHECKLIST.md — Seguimiento de Mdx

> Estado verificable de cada historia y criterio de aceptación del PRD.
> **Reglas:** marca `[x]` solo cuando el AC pasa y tiene test (automático o manual registrado). Usa `[~]` para cumplido con observación (explícala en `docs/progress.md`). No borres ítems: si uno queda fuera de alcance, márcalo `[-]` con motivo.
> Leyenda: `[ ]` pendiente · `[x]` hecho · `[~]` hecho con observación · `[-]` fuera de alcance · 🧪 test automático · 👁 verificación manual

---

## A. Checklist por commit (antes de cada `git commit`)

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] AC afectados marcados aquí
- [ ] `CHANGELOG.md` → entrada en `[Unreleased]` si el cambio es visible para el usuario
- [ ] `docs/DATA.md` / `docs/ARCHITECTURE.md` / `docs/STACK.md` actualizados si cambió lo que describen
- [ ] Mensaje Conventional Commit con `[US-x.y]`

## B. Checklist de cierre de fase

- [ ] Todas las historias de la fase en `[x]` o `[~]`/`[-]` justificados
- [ ] Hito de la fase verificado (👁)
- [ ] `CHANGELOG.md`: `[Unreleased]` movido a la nueva versión con fecha
- [ ] Versión subida en `[workspace.package]` (ver CLAUDE.md §Versionado)
- [ ] Commit `chore(release): vX.Y.Z`
- [ ] Tag `vX.Y.Z` **solo si el usuario lo autoriza**

---

## Fase 0 — Fundaciones → v0.1.0

### US-0.1 Workspace y CI
- [x] AC1 `cargo build` y `cargo test` pasan 🧪
- [x] AC2 CI en ubuntu/macos/windows 🧪 — GitHub Actions, run 45295816044 en `main` (2026-11-24): `fmt`, `clippy`, `test` y `cargo deny` en verde en ubuntu, macOS y windows
- [x] AC3 clippy sin warnings 🧪
- [x] AC4 `cargo deny check` configurado 🧪

### US-0.2 Ventana base
- [x] AC1 Ventana "Mdx" abre 👁
- [x] AC2 Layout 3 paneles 20/40/40 👁
- [x] AC3 Divisores arrastrables 👁
- [x] AC4 Barra de estado visible 👁

- [ ] **Hito F0**: CI verde en 3 SO y ventana con layout

## Fase 1 — MVP del editor → v0.2.0

### US-1.1 Nuevo / Abrir / Guardar
- [x] AC1 Ctrl/Cmd+N 👁
- [x] AC2 Ctrl/Cmd+O con filtro 👁
- [x] AC3 Ctrl/Cmd+S (y Guardar como si no hay ruta) 👁
- [x] AC4 Ctrl/Cmd+Shift+S 👁
- [x] AC5 Guardado atómico 🧪 (INV-2) — `io::tests::atomic_failure_keeps_original`
- [x] AC6 Preserva LF/CRLF 🧪 (INV-1) — `io::tests::roundtrip_preserves_crlf_and_bom`, `edited_crlf_file_stays_crlf`
- [x] AC7 Error claro con no-UTF-8 🧪 — `io::tests::non_utf8_is_clear_error`

### US-1.2 Estado "sin guardar"
- [x] AC1 `●` en título 👁
- [x] AC2 Diálogo al cerrar con cambios 👁
- [x] AC3 Deshacer hasta guardado limpia indicador 🧪 — `app::tests::returning_to_saved_state_clears_indicator`

### US-1.3 Editor
- [x] AC1 Fuente monoespaciada 👁
- [x] AC2 Resaltado Markdown 👁
- [x] AC3 Deshacer/rehacer 👁
- [x] AC4 Tab/Shift+Tab 👁
- [x] AC5 Barra de estado (línea:col, palabras, EOL, codificación) 🧪 conteo + 👁

### US-1.4 Vista previa
- [x] AC1 Debounce ≤ 150 ms 👁
- [x] AC2 Elementos CommonMark + GFM 👁 (fixture `fixtures/gfm_showcase.md`)
- [x] AC3 Enlaces externos y relativos 👁
- [x] AC4 Ctrl/Cmd+\ alterna preview 👁

- [~] **Hito F1**: editar el README del repo sin pérdida de datos 👁 (README creado en el cierre; verificación manual pendiente, ver progress.md)

## Fase 2 — Explorador → v0.3.0

### US-2.1 Abrir carpeta
- [x] AC1 Ctrl/Cmd+Shift+O y argumento CLI 👁
- [x] AC2 Carga perezosa 🧪 — `workspace::tests::loading_is_lazy_one_level_only`, `app::tests::toggle_dir_loads_lazily_then_collapses`
- [x] AC3 `.gitignore` y ocultos 🧪 — `workspace::tests::gitignore_is_respected_and_togglable`, `hidden_files_toggle` (toggle en menú de clic derecho del panel, 👁 en progress.md)
- [x] AC4 Filtro solo Markdown 🧪 — `workspace::tests::only_markdown_filter_keeps_all_dirs`
- [x] AC5 Orden dirs→archivos, alfabético 🧪 (INV-4) — `workspace::tests::sort_order_dirs_first_then_case_insensitive`
- [x] AC6 10 000 archivos sin bloquear UI 🧪 (ignored) + 👁 — `workspace::tests::perf_ten_thousand_files` (15 ms en release)

### US-2.2 Navegación
- [x] AC1 Clic simple / doble clic 👁 + 🧪 (INV-7) — `state::tabs::tests::*`, `app::tests::single_clicks_reuse_one_preview_tab` (la barra visible de pestañas llega en US-3.1)
- [x] AC2 Navegación por teclado 👁
- [x] AC3 Resaltado del activo 👁

### US-2.3 Operaciones de archivo
- [x] AC1 Menú contextual completo 👁
- [x] AC2 Eliminar → papelera con confirmación 🧪 + 👁 — `fileops::tests::trash_goes_through_the_injected_trasher_and_reports_errors`; envío real a la papelera verificado a mano
- [x] AC3 Renombrar actualiza pestaña 🧪 — `app::tests::renaming_the_open_file_updates_its_path`, `renaming_a_parent_folder_rebases_the_open_file`, `fileops::tests::rebase_path_for_open_documents`
- [x] AC4 Validación de nombres 🧪 — `fileops::tests::validates_names`, `invalid_name_leaves_disk_untouched`, `create_file_adds_md_and_never_overwrites`, `rename_moves_file_and_refuses_duplicates`, `app::tests::invalid_name_shows_inline_error_*`, `duplicate_name_error_from_disk_*`

### US-2.4 Cambios externos
- [x] AC1 Árbol refleja cambios 🧪 — `watcher::tests::*` (incl. `real_watcher_reports_creation_and_modification`), `explorer::tests::reloads_only_loaded_parent_dirs_without_duplicates`
- [x] AC2 Recarga silenciosa sin cambios locales 🧪 — `app::tests::external_change_on_clean_document_reloads_silently`
- [x] AC3 Banner de conflicto 🧪 + 👁 — `app::tests::external_change_on_dirty_document_shows_conflict_banner`, `conflict_reload_*`, `conflict_keep_mine_*`
- [x] AC4 Pestaña "(eliminado)" 🧪 + 👁 — `app::tests::deleted_on_disk_is_marked_and_keeps_content_and_can_be_resaved` (marcador en el título de la ventana; la barra de pestañas llega en US-3.1)
- [x] AC5 Guardado propio no dispara banner 🧪 (INV-3) — `app::tests::own_save_never_triggers_the_banner`

- [x] **Hito F2**: navegar y editar el repo; cambio externo manejado 👁 (verificado por el propietario el 2026-11-23)

## Fase 3 — Productividad → v0.4.0

### US-3.1 Pestañas
- [x] AC1 Ctrl/Cmd+W con diálogo 👁
- [x] AC2 Ctrl+Tab y Ctrl/Cmd+1..9 👁
- [x] AC3 `●` y tooltip 👁

### US-3.2 Outline
- [x] AC1 Lista H1–H6 indentada 👁
- [x] AC2 Clic posiciona cursor 👁
- [x] AC3 Extracción con snapshots 🧪 — `outline::tests::*` (snapshot `insta`), `app::tests::outline_is_empty_until_the_debounced_parse_arrives`, `outline_keeps_updating_when_the_preview_is_hidden`

### US-3.3 Scroll sincronizado
- [x] AC1 Sincronía editor→preview 👁
- [x] AC2 Toggle 👁

### US-3.4 Buscar y reemplazar
- [x] AC1 Buscar / reemplazar en documento 🧪 + 👁
- [x] AC2 Opciones (mayúsculas, palabra, regex) 🧪 — `search::tests::options_case_sensitive_whole_word_and_regex`, `invalid_regex_*`, `app::tests::options_change_the_matches`, `regex_replacement_expands_capture_groups`
- [x] AC3 Búsqueda en workspace cancelable 🧪 + 👁

### US-3.5 Autoguardado y sesión
- [x] AC1 Autoguardado opcional 🧪 — `app::tests::autosave_*`, `config::tests::*`
- [x] AC2 Sesión guardada al cerrar 🧪 — `app::tests::snapshot_captures_*`, `signature_changes_*`, `session::tests::toml_roundtrip_*`, `save_and_load_roundtrip_*` (+ prueba de extremo a extremo manual del archivo escrito)
- [x] AC3 Restauración con rutas faltantes 🧪 (INV-5), también al abrir con argumento (decisión del propietario) — `session::tests::missing_paths_skipped`, `boot_restores_existing_files_in_order_*`, `app::tests::restoring_a_session_*`, `expanded_directories_from_the_session_*`
- [x] AC4 Recientes (máx. 10) 🧪 — `session::tests::recents_are_most_recent_first_*`, `app::tests::opening_files_and_folders_feeds_the_recents_list`, `recents_are_capped_at_ten_files`

- [~] **Hito F3**: sesión de trabajo completa con varias pestañas y búsqueda 👁 (cada historia US-3.1…3.5 verificada a mano por el propietario; el recorrido completo de una sola sesión queda pendiente)

## Fase 4 — Pulido → v0.5.0

### US-4.1 Temas
- [x] AC1 Claro / oscuro / sistema 👁
- [x] AC2 Preview con paleta del tema 👁

### US-4.2 Configuración
- [x] AC1 `config.toml` en dir de SO 🧪 — `paths::tests::*`, `config::tests::ensure_file_creates_the_template_once_and_never_overwrites`
- [x] AC2 Ajustes soportados 🧪 — `config::tests::every_supported_setting_can_be_set`, `out_of_range_values_are_clamped_*`, `app::tests::loaded_config_drives_the_explorer_filters_preview_and_editor_settings`, `editor::tests::wrap_and_size_mappings` (tema y zoom se leen y validan; se aplican en US-4.1 y US-4.5)
- [x] AC3 Ctrl/Cmd+, y recarga al guardar 👁
- [x] AC4 Config inválida → defaults, sin crash 🧪 (INV-6) — `config::tests::invalid_not_overwritten`, `invalid_toml_wrong_types_unknown_theme_and_newer_schema_are_errors`, `app::tests::a_reloaded_valid_config_is_confirmed_and_an_invalid_one_falls_back_to_defaults`; arranque real con config válida e inválida sin errores y sin tocar el archivo

### US-4.3 Exportar HTML
- [x] AC1 HTML autocontenido 👁
- [x] AC2 Snapshot test 🧪 — `export::tests::snapshot_of_the_exported_body`, `snapshot_of_the_whole_document` (`insta`)

### US-4.4 Imágenes
- [x] AC1 Imágenes relativas en preview 👁 (fixture `fixtures/imagenes.md`)
- [x] AC2 Pegar imagen → `./assets/` 🧪 + 👁 — `assets::tests::pasted_images_are_saved_*`, `app::tests::a_saved_image_is_inserted_*`

### US-4.5 Zoom
- [x] AC1 Ctrl/Cmd +/-/0 👁

- [~] **Hito F4**: app usable a diario con config personalizada 👁 (cada historia US-4.1…4.5 verificada a mano por el propietario; el uso diario con una configuración propia queda pendiente)

## Fase 5 — Distribución → v1.0.0

### US-5.1 Paquetes
- [ ] AC1 MSI / DMG universal / AppImage / DEB 🧪 (CI)
- [ ] AC2 Workflow de release por tag 🧪
- [ ] AC3 Asociación de `.md` 👁
- [ ] AC4 Ícono en 3 SO 👁

### US-5.2 Firma
- [ ] AC1 Documentado y parametrizado por secrets 👁

- [ ] **Hito F5**: instaladores probados en Windows, macOS y Linux 👁

---

## Requerimientos no funcionales (verificar al cierre de F1, F3 y F5)

| NFR | F1 | F3 | F5 | Valor medido |
|---|---|---|---|---|
| NFR-1 Binario < 15 MB | [x] | [x] | [ ] | 11,51 MB (binario release macOS arm64, `cargo xtask size`). Instaladores: medir en F5 · **F3:** 12,39 MB (binario release macOS arm64, `cargo xtask size`) |
| NFR-2 Arranque < 300 ms | [~] | [~] | [ ] | ≈130 ms desde `main` hasta adaptador wgpu listo (log); primer frame sin instrumentar → 👁 pendiente · **F3:** ≈145–180 ms desde `main` hasta adaptador wgpu listo (con sesión restaurada); primer frame sin instrumentar → 👁 |
| NFR-3 RAM < 80 MB | [~] | [ ] | [ ] | Footprint físico 63 MB (pico 71 MB) en reposo sin archivo (macOS `footprint`); RSS 102 MB incluye librerías compartidas. Con archivo de 100 KB: pendiente 👁 · **F3 ✗ NO CUMPLIDO:** footprint físico (macOS `footprint`) 82 MB con un archivo de unas líneas y **176 MB con un archivo de 100 KB** (143 MB con la vista previa oculta; ≈19 MB menos sin resaltado). Reposo sin archivo: 63 MB. Causa: `cosmic-text` guarda glifos de todo el texto en el editor y en la vista previa. Ver progress.md |
| NFR-4 Archivo 1 MB fluido / 5 MB usable | [~] | [~] | [ ] | Release, `cargo test --release -- --ignored`: 1 MB → edición 3 ms, deshacer 2 ms, carga 0,7 s, parseo preview 0,28 s (hilo aparte); 5 MB → edición 17 ms, deshacer 11 ms, carga 3,4 s, parseo 3,5 s. Render de preview/editor con fixtures: 👁 pendiente · **F3** (release): 1 MB → pulsación 3–4 ms (4 ms con la barra de búsqueda abierta), buscar 4 348 coincidencias 1 ms, sondeo de scroll < 1 ms, reemplazar todo 0,76 s y deshacerlo 0,72 s, esquema/bloques 5 ms (en hilo aparte); 5 MB → pulsación 17–21 ms, buscar 21 534 coincidencias 2 ms, reemplazar todo 3,5 s y deshacer 3,5 s. Abrir bloquea la UI 0,7 s (1 MB) / 3,4 s (5 MB). 👁 pendiente |
| NFR-5 Sin unwrap en core | [x] | [x] | [ ] | clippy `unwrap_used`/`expect_used` = deny en producción · **F3:** clippy `unwrap_used`/`expect_used` = deny en producción |
| NFR-6 Sin unsafe | [x] | [x] | [ ] | `unsafe_code = "forbid"` en el workspace · **F3:** `unsafe_code = "forbid"` en el workspace |
| NFR-7 Todo por teclado | [x] | [~] | [ ] | F1: nuevo/abrir/guardar/deshacer/preview por atajos (el arrastre de divisores es solo ratón) · **F3:** atajos para abrir/guardar/pestañas/buscar/deshacer/preview; solo con ratón: cambio de panel lateral (Archivos/Esquema), esquema, casilla de scroll sincronizado, menús contextuales y botones de la barra de búsqueda |
| NFR-8 Strings centralizados | [x] | [x] | [ ] | Textos de UI en `strings.rs`; solo quedan separadores de formato · **F3:** textos de UI en `strings.rs` |
| NFR-9 Cobertura core ≥ 70 % | [x] | [x] | [ ] | `cargo llvm-cov`: mdx-core ≈ 97 % de líneas por archivo (workspace 78 %) · **F3:** `cargo llvm-cov`: todos los archivos de mdx-core ≥ 79 % de líneas (workspace 78 %) |

## QA manual por plataforma (antes de v1.0.0)

| Escenario | Windows | macOS | Linux X11 | Linux Wayland |
|---|---|---|---|---|
| Instalar y abrir `.md` con doble clic | [ ] | [ ] | [ ] | [ ] |
| Atajos con Ctrl/Cmd correctos | [ ] | [ ] | [ ] | [ ] |
| Diálogos nativos | [ ] | [ ] | [ ] | [ ] |
| Papelera del sistema | [ ] | [ ] | [ ] | [ ] |
| Watcher (editar desde otro programa) | [ ] | [ ] | [ ] | [ ] |
| HiDPI / escalado | [ ] | [ ] | [ ] | [ ] |
| Tema del sistema | [ ] | [ ] | [ ] | [ ] |
| CRLF en Windows preservado | [ ] | — | — | — |
