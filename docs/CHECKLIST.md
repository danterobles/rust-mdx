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
- [~] AC2 CI en ubuntu/macos/windows 🧪 (workflow escrito; verificación pendiente del primer push, ver progress.md)
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
- [ ] AC1 Lista H1–H6 indentada 👁
- [ ] AC2 Clic posiciona cursor 👁
- [ ] AC3 Extracción con snapshots 🧪

### US-3.3 Scroll sincronizado
- [ ] AC1 Sincronía editor→preview 👁
- [ ] AC2 Toggle 👁

### US-3.4 Buscar y reemplazar
- [ ] AC1 Buscar / reemplazar en documento 🧪 + 👁
- [ ] AC2 Opciones (mayúsculas, palabra, regex) 🧪
- [ ] AC3 Búsqueda en workspace cancelable 🧪 + 👁

### US-3.5 Autoguardado y sesión
- [ ] AC1 Autoguardado opcional 🧪
- [ ] AC2 Sesión guardada al cerrar 🧪
- [ ] AC3 Restauración con rutas faltantes 🧪 (INV-5)
- [ ] AC4 Recientes (máx. 10) 🧪

- [ ] **Hito F3**: sesión de trabajo completa con varias pestañas y búsqueda 👁

## Fase 4 — Pulido → v0.5.0

### US-4.1 Temas
- [ ] AC1 Claro / oscuro / sistema 👁
- [ ] AC2 Preview con paleta del tema 👁

### US-4.2 Configuración
- [ ] AC1 `config.toml` en dir de SO 🧪
- [ ] AC2 Ajustes soportados 🧪
- [ ] AC3 Ctrl/Cmd+, y recarga al guardar 👁
- [ ] AC4 Config inválida → defaults, sin crash 🧪 (INV-6)

### US-4.3 Exportar HTML
- [ ] AC1 HTML autocontenido 👁
- [ ] AC2 Snapshot test 🧪

### US-4.4 Imágenes
- [ ] AC1 Imágenes relativas en preview 👁
- [ ] AC2 Pegar imagen → `./assets/` 🧪 + 👁

### US-4.5 Zoom
- [ ] AC1 Ctrl/Cmd +/-/0 👁

- [ ] **Hito F4**: app usable a diario con config personalizada 👁

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
| NFR-1 Binario < 15 MB | [x] | [ ] | [ ] | 11,51 MB (binario release macOS arm64, `cargo xtask size`). Instaladores: medir en F5 |
| NFR-2 Arranque < 300 ms | [~] | [ ] | [ ] | ≈130 ms desde `main` hasta adaptador wgpu listo (log); primer frame sin instrumentar → 👁 pendiente |
| NFR-3 RAM < 80 MB | [~] | [ ] | [ ] | Footprint físico 63 MB (pico 71 MB) en reposo sin archivo (macOS `footprint`); RSS 102 MB incluye librerías compartidas. Con archivo de 100 KB: pendiente 👁 |
| NFR-4 Archivo 1 MB fluido / 5 MB usable | [~] | [ ] | [ ] | Release, `cargo test --release -- --ignored`: 1 MB → edición 3 ms, deshacer 2 ms, carga 0,7 s, parseo preview 0,28 s (hilo aparte); 5 MB → edición 17 ms, deshacer 11 ms, carga 3,4 s, parseo 3,5 s. Render de preview/editor con fixtures: 👁 pendiente |
| NFR-5 Sin unwrap en core | [x] | [ ] | [ ] | clippy `unwrap_used`/`expect_used` = deny en producción |
| NFR-6 Sin unsafe | [x] | [ ] | [ ] | `unsafe_code = "forbid"` en el workspace |
| NFR-7 Todo por teclado | [x] | [ ] | [ ] | F1: nuevo/abrir/guardar/deshacer/preview por atajos (el arrastre de divisores es solo ratón) |
| NFR-8 Strings centralizados | [x] | [ ] | [ ] | Textos de UI en `strings.rs`; solo quedan separadores de formato |
| NFR-9 Cobertura core ≥ 70 % | [x] | [ ] | [ ] | `cargo llvm-cov`: mdx-core ≈ 97 % de líneas por archivo (workspace 78 %) |

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
