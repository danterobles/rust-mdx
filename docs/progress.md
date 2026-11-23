# progress.md — Bitácora de Mdx

## US-0.1 Workspace y CI — ✅ completa salvo verificación de CI remota (rama feat/us-0.1-workspace-ci)
Plan: `Cargo.toml` (workspace), `crates/mdx-core`, `crates/mdx-app` (bin `mdx`), `xtask`, `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml`, `.gitignore`, `.github/workflows/ci.yml`, ADR 0001–0004.
- AC1 🧪 `cargo build --workspace` y `cargo test --workspace` (test `mdx_core::tests::version_is_semver_like`)
- AC2 🧪 `ci.yml` con matriz ubuntu/macos/windows (fmt, clippy, test); ~ el workflow no se pudo ejecutar sin push (requiere autorización); verificar en el primer push. Dependencias de sistema Linux de iced/rfd por confirmar en US-0.2
- AC3 🧪 `cargo clippy --workspace --all-targets -- -D warnings` limpio
- AC4 🧪 `deny.toml` + job `cargo deny` en CI
Decisiones:
- MSRV `1.92`: la exige `rfd 0.18` (iced 0.14 pide 1.88; `notify 9` pide 1.91).
- Versiones fijadas en `[workspace.dependencies]` (verificadas con `cargo info` el 2026-11-23); los crates solo declaran lo que usan hoy (`mdx-app`: `tracing`, `tracing-subscriber`, `mdx-core`). `iced` y el resto se activan en las historias que los necesitan.
- `license` y `repository` siguen en `TBD` (preguntas abiertas del PRD §12).
- AC4 🧪 `cargo deny check` → advisories/bans/licenses/sources ok (cargo-deny 0.22.0). Crates del workspace con `publish = false` y `licenses.private.ignore` mientras la licencia sea TBD.

## US-0.2 Ventana base — ✅ completa (rama feat/us-0.2-ventana-base)
Plan: `mdx-app/src/{main,app,layout,strings}.rs`; `iced::application` con `pane_grid` (Explorador | Editor | Preview) + barra de estado.
- AC1 👁 `cargo run -p mdx-app` arranca y crea la ventana (log de `iced_winit`, título "Mdx"). No pude capturar pantalla desde el agente → confirmación visual del usuario pendiente.
- AC2 🧪 `layout::tests::initial_layout_is_20_40_40` + 👁 pendiente.
- AC3 🧪 `layout::tests::dividers_can_be_resized` (resize por `State::resize`); 👁 arrastre real pendiente.
- AC4 👁 barra de estado vacía bajo el grid; pendiente de confirmar.
Decisiones:
- Raíz = split vertical 0.2; derecha = split vertical 0.5 (⇒ 20/40/40). Tema fijo `Light` hasta US-4.1.
- iced con `default-features = false` pierde `x11`/`wayland`/`crisp`: se añadieron a las features del workspace.
- `cargo deny`: se permitieron `BSL-1.0` (clipboard-win, error-code) y `NCSA` (libfuzzer-sys vía image→rav1e), ambas permisivas y solo transitivas. Cambio de política en STACK §8 → confirmar con el propietario.
- Textos en `strings.rs` (NFR-8). Dependencias de sistema Linux de iced/rfd siguen por confirmar en CI.
- Verificación 👁 AC1–AC4 confirmada manualmente por el propietario (macOS) el 2026-11-23.

## US-1.1 Nuevo / Abrir / Guardar — ✅ completa (rama feat/us-1.1-open-save)
Plan: `mdx-core` (`error.rs`, `document.rs`, `io.rs`), `mdx-app` (`shortcuts.rs`, `app.rs` con `text_editor` mínimo, `strings.rs`).
- AC1 👁 Ctrl/Cmd+N → documento «Sin título» (🧪 mapeo `shortcuts::tests::maps_file_shortcuts`)
- AC2 👁 Ctrl/Cmd+O → `rfd::AsyncFileDialog` con filtros «Markdown y texto» (`md`,`markdown`,`txt`) y «Todos» (`*`)
- AC3 👁 Ctrl/Cmd+S → guarda; sin ruta → Guardar como
- AC4 👁 Ctrl/Cmd+Shift+S → Guardar como
- AC5 🧪 `io::tests::atomic_failure_keeps_original` (fallo simulado antes del `rename`; original intacto y sin temporal)
- AC6 🧪 `io::tests::roundtrip_preserves_crlf_and_bom`, `edited_crlf_file_stays_crlf`, `document::tests::roundtrip_is_byte_identical`
- AC7 🧪 `io::tests::non_utf8_is_clear_error`; en la UI el error se muestra en la barra de estado y el documento actual no cambia.
Decisiones / limitaciones:
- `rfd 0.18` no tiene feature `tokio` (STACK.md decía lo contrario): se usan features por defecto (`xdg-portal`, `wayland`). Corregido en STACK.
- Un solo documento en memoria; `Message::Saved` lleva el texto guardado para que `is_dirty` sea correcto si se edita durante el guardado. DATA.md prevé `DocumentId` en los mensajes: llega con pestañas (US-3.1).
- **Limitación conocida (resuelta en US-1.2):** Nuevo/Abrir reemplazaban el documento sin preguntar.
- `text_editor` mínimo (sin fuente monoespaciada, resaltado ni conteos): lo completa US-1.3. Título de ventana «nombre — Mdx»; el `●` llega con US-1.2.
- `doc.text` solo se sincronizaba al abrir/guardar; desde US-1.2 se sincroniza en cada edición.
- Filtro «Todos» con extensión `*`: comportamiento a confirmar por plataforma (👁).
- Tests de `mdx-core` permiten `unwrap` vía `cfg_attr(test, ...)`; NFR-5 sigue vigente para código de producción.
- Guardado atómico: temporal `.<nombre>.mdx-tmp-<pid>` en el mismo directorio; no preserva permisos del original (mejora futura si hace falta).
- Verificación 👁 AC1–AC4 confirmada manualmente por el propietario el 2026-11-23.

## US-1.2 Estado "sin guardar" — ✅ completa (rama feat/us-1.2-sin-guardar)
Plan: `app.rs` (`dirty`, `PendingAction`, `GuardChoice`, `after_save`), `strings.rs`, `main.rs` (`exit_on_close_request(false)`).
- AC1 🧪 `app::tests::editing_marks_dirty_in_title` + 👁 título `● nombre — Mdx`
- AC2 👁 al cerrar con cambios aparece el diálogo Guardar / Descartar / Cancelar (`rfd::AsyncMessageDialog`, botones personalizados). Guardar → guarda y luego cierra (si el guardado falla o se cancela «Guardar como», no cierra). Descartar → cierra. Cancelar → sigue abierto.
- AC3 🧪 `app::tests::returning_to_saved_state_clears_indicator` (comparación por hash xxh3 contra `saved_hash`)
Decisiones:
- El mismo diálogo protege Nuevo (Ctrl/Cmd+N) y Abrir (Ctrl/Cmd+O) con cambios pendientes: cierra el riesgo de pérdida de datos anotado en US-1.1. Es una extensión mínima del AC2, no una funcionalidad nueva.
- `dirty` se cachea y se recalcula solo en acciones de edición (`Action::is_edit`), al guardar y al cargar. Cada edición materializa `content.text()` + hash: **medir en NFR-4 (1 MB)** al cierre de F1; si no es fluido, pasar a hash incremental/debounce.
- Si el usuario edita mientras se guarda, el documento queda sucio (se compara contra el contenido actual).
- Mensaje de aviso con nombre: «{name}» se sustituye en `strings::UNSAVED_BODY`.
- Con `exit_on_close_request(false)` el cierre lo decide la app (`iced::exit()`).
- Verificación 👁 AC1–AC2 confirmada manualmente por el propietario el 2026-11-23.

## US-1.3 Editor — ✅ completa (rama feat/us-1.3-editor)
Plan: `mdx-core` (`history.rs`, `document::count_words`), `mdx-app` (`editor.rs`, `statusbar.rs`, `app.rs`, `shortcuts.rs`).
- AC1 👁 `Font::MONOSPACE` en el `text_editor`.
- AC2 👁 `.highlight("markdown", InspiredGitHub)` (iced_highlighter/syntect; tema claro acorde al tema fijo `Light`).
- AC3 🧪 `history::tests::*` (round trip, ráfagas, redo descartado, multibyte, límite de memoria, texto divergente) y `app::tests::undo_and_redo_typing_burst`, `undo_restores_cursor_and_edit_after_undo_drops_redo`. Atajos: `shortcuts::tests::maps_undo_redo`.
- AC4 🧪 `app::tests::indent_and_unindent_selection`; `key_binding` mapea Tab/Shift+Tab a `Edit::Indent/Unindent`.
- AC5 🧪 `statusbar::tests::*` (formato, columna en caracteres), `document::tests::counts_words`, `app::tests::stats_follow_cursor_and_words`. Se muestra a la derecha de la barra de estado; los mensajes (abierto/guardado/errores) siguen a la izquierda.
Decisiones:
- **iced 0.14 no tiene deshacer/rehacer** en `text_editor`: se implementó `History` propio (ADR 0005), sin dependencias nuevas.
- El parche de deshacer se aplica seleccionando el rango y pegando/borrando para no reconstruir el `Content` (conserva scroll).
- La indentación con `Edit::Indent` usa el ancho de tabulación por defecto del motor (8 espacios, observado en test). Sin selección, inserta espacios en el cursor; con selección, indenta las líneas. No configurable en v1.
- Palabras = secuencias separadas por espacios en blanco; se recalculan en cada edición (O(n)). **Medir en NFR-4 (1 MB)** al cierre de F1.
- Columna mostrada en caracteres (no bytes).
- Verificación 👁 AC1–AC5 confirmada manualmente por el propietario el 2026-11-23.

## US-1.4 Vista previa en vivo — ✅ completa (rama feat/us-1.4-preview)
Plan: `mdx-core::link` (clasificación de enlaces), `mdx-app` (`app.rs`: preview, debounce, enlaces, toggle; `layout.rs`; `shortcuts.rs`), `fixtures/gfm_showcase.md`.
- AC1 🧪 `app::tests::preview_updates_only_after_debounce`, `loaded_document_renders_preview_immediately`; 👁 pendiente. Debounce: `time::every(25 ms)` solo mientras `preview_pending`; re-parsea cuando `now - última edición ≥ 150 ms` (ARCHITECTURE decía 50 ms de tick: se bajó a 25 ms para quedar más cerca de «≤ 150 ms»).
- AC2 👁 `iced::widget::markdown` (pulldown-cmark con tablas, tachado y listas de tareas activados por iced); código resaltado vía feature `highlighter`. Verificar con `fixtures/gfm_showcase.md`.
- AC3 🧪 `link::tests::*` (http/https externos, relativos con `../`, `#ancla`, `%20`, esquemas no soportados), `app::tests::unsupported_link_reports_error_and_keeps_document`; 👁 apertura real en navegador/editor. Los `.md` relativos pasan por el diálogo de cambios sin guardar (`PendingAction::OpenPath`).
- AC4 🧪 `shortcuts::tests::maps_toggle_preview_by_physical_key`, `app::tests::toggle_preview_hides_and_restores_pane`; 👁 pendiente.
Decisiones:
- **Tablas sí soportadas** por el widget `markdown` de iced 0.14: no hizo falta ADR ni widget propio.
- Ctrl/Cmd+\ se detecta por **tecla física** (`Code::Backslash`) porque en teclados como el español `\` requiere Alt.
- Ocultar la vista previa reconstruye el layout (Explorador 20 % · Editor 80 %); al mostrarla se restaura 20/40/40. Los anchos arrastrados por el usuario no se recuerdan (podría guardarse con la sesión en US-3.5).
- La preview se re-parsea completa en cada pausa (el parseo incremental de iced solo sirve para añadir al final). **Medir en NFR-4 (1 MB)**; si no es fluido, mover el parseo a un `Task`.
- `open::that_detached` se ejecuta dentro de un `Task` (no en `update`).
- Enlaces a `.md` relativos requieren que el documento tenga ruta; sin ruta se informa «Enlace no soportado».
- Imágenes: fuera de esta historia (US-4.4).
- Verificación 👁 AC1–AC4 confirmada manualmente por el propietario el 2026-11-23.

## Cierre de F1 — mediciones (rama chore/cierre-f1)
- `cargo xtask gen-fixtures` genera `fixtures/large_1mb.md` y `large_5mb.md` (ignorados por git); `cargo xtask size` verifica NFR-1.
- Test de rendimiento `app::tests::perf_edit_cost_on_large_documents` (`#[ignore]`): `cargo test --release -p mdx-app perf_ -- --ignored --nocapture`.
- **Hallazgo NFR-4:** el re-parseo de la vista previa en el hilo de UI costaba 258 ms (1 MB) y 3,5 s (5 MB) tras cada pausa. Se movió a `Task` + `spawn_blocking` con generación para descartar resultados viejos (`Message::PreviewParsed`); `preview` pasó de `markdown::Content` a `Vec<markdown::Item>`.
- **Observación abierta:** abrir un archivo grande bloquea la UI mientras `text_editor::Content::with_text` lo carga (0,7 s con 1 MB; 3,4 s con 5 MB). No se puede mover a otro hilo con la API actual de iced; aceptable para «usable con 5 MB» pero a vigilar. El coste de dibujar la preview de un documento enorme (iced no virtualiza) solo se puede ver a mano.
- **NFR-3:** la métrica fiable en macOS es el *physical footprint* (63 MB, pico 71 MB), no el RSS (102 MB). `tiny-skia` usa más memoria (RSS 153 MB) que `wgpu`.
- **NFR-2:** sin instrumentación de «primer frame»; ≈130 ms hasta que wgpu selecciona el adaptador. Confirmación subjetiva/👁 del propietario.
- Cobertura: `cargo-llvm-cov` 0.9.2 instalado; `mdx-core` ≈ 97 % por archivo.
- Hito F1: se añadió `README.md` en la raíz para poder editarlo de principio a fin (👁 del propietario).
- Mediciones hechas solo en macOS (arm64); Windows/Linux quedan para QA de F5.

## Cierre de F1 → v0.2.0 (2026-11-23)
- Decisión del propietario: ir directo a 0.2.0 sin cerrar F0 (se omite 0.1.0). F0 sigue con AC2 de US-0.1 en `[~]` (CI remoto sin verificar por falta de `push`), licencia y URL del repositorio en `TBD`.
- Historias F1: US-1.1, US-1.2, US-1.3 y US-1.4 completas (AC verificados).
- **Pendientes de verificación 👁 al cerrar** (no bloquean según decisión del propietario, quedan abiertos): Hito F1 (editar README sin pérdida de datos), NFR-2 (primer frame), NFR-3 con archivo de 100 KB, NFR-4 visual con `large_1mb/5mb`.
- Tag `v0.2.0`: **no creado**; requiere autorización explícita.

## US-2.1 Abrir carpeta — ✅ completa (rama feat/us-2.1-abrir-carpeta)
Plan: `mdx-core::workspace` (árbol, filtros, `load_children` con `ignore::WalkBuilder` a profundidad 1), `mdx-app` (`explorer.rs`, `app.rs`, `shortcuts.rs`, CLI en `main.rs`).
- AC1 🧪/👁 Ctrl/Cmd+Shift+O abre el selector de carpeta; `mdx <ruta>`: carpeta → workspace, archivo → editor. Verificar a mano.
- AC2 🧪 solo se lee el primer nivel; cada carpeta se lee al expandirla (`Children::NotLoaded → Loading → Loaded`).
- AC3 🧪 `.gitignore` (incluye el de carpetas padre, también fuera de un repo git: `require_git(false)`) y ocultos, ambos por defecto. Toggle en un menú que se abre con **clic derecho en el panel** (casillas; no hay widget de menú contextual en iced 0.14, así que el menú se muestra en línea en la parte superior del panel, no junto al cursor).
- AC4 🧪 «Solo Markdown» por defecto; las carpetas nunca se ocultan.
- AC5 🧪 INV-4 en `workspace::compare_nodes`.
- AC6 🧪 `perf_ten_thousand_files`: 15 ms para 10 000 archivos + 100 carpetas (release); la carga va en `Task` + `spawn_blocking`. 👁 pendiente con una carpeta real grande.
Decisiones:
- Cambiar un filtro recarga la raíz y **colapsa el árbol** (se pierde lo expandido).
- Los archivos del árbol son solo texto por ahora; abrir con clic es US-2.2.
- `ExplorerState` agrupa workspace, filtros y menú (también evita `struct_excessive_bools`).
- El árbol se dibuja completo en cada `view` (iced no virtualiza): con un directorio de 10 000 entradas expandido puede notarse; vigilar en la verificación 👁 de AC6 y, si hace falta, paginar/virtualizar (ADR).
- Archivos y enlaces simbólicos: `follow_links(false)`.
- Con `mdx <ruta>` que no existe, se muestra el error de lectura en la barra de estado.
- Verificación 👁 AC1 y AC6 confirmada manualmente por el propietario el 2026-11-23.

## US-2.2 Navegación — ✅ completa (rama feat/us-2.2-navegacion)
Plan: `mdx-app/src/state/tabs.rs` (modelo INV-7), `explorer.rs` (vista, resalte, navegación pura), `app.rs` (clic, doble clic, teclas, `pin_on_load`), `mdx-core::workspace::visible_nodes`.
- AC1 🧪 `state::tabs::tests::*` (INV-7: nunca más de una pestaña sin fijar), `app::tests::single_clicks_reuse_one_preview_tab`, `double_click_pins_the_opened_file`, `double_click_on_open_preview_pins_it_without_reloading`, `clicking_a_file_with_unsaved_changes_does_not_discard_them`. 👁 pendiente.
- AC2 🧪 `explorer::tests::*` (↑/↓ con tope, → expande y entra, ← colapsa y sube al padre, Enter abre/alterna), `app::tests::keyboard_navigation_moves_selection_and_toggles`. 👁 pendiente.
- AC3 👁 el archivo abierto se resalta con el color primario del tema; la fila con foco de teclado, con un gris más sutil.
Decisiones:
- **Alcance acordado con el propietario:** solo el modelo de pestañas (INV-7); la barra de pestañas visible, varios documentos abiertos a la vez, Ctrl+W y Ctrl+Tab se refinan en US-3.1. Mientras tanto, abrir un archivo reemplaza el documento actual (con el diálogo de cambios sin guardar si hace falta) y la «fijada» no tiene efecto visible.
- `Ctrl/Cmd+O` y `mdx <archivo>` abren el archivo **fijado**; el clic simple, como preview; Nuevo, fijado.
- Doble clic: `mouse_area` con `on_press` + `on_double_click` (un `button` captura el clic y no deja pasar el doble clic). El primer clic ya abre el archivo; el doble clic lo fija (`pin_on_load` si aún se está cargando).
- Teclado del árbol: la suscripción global solo reenvía flechas/Enter si ningún widget los consumió (`status == Ignored`), así el editor enfocado conserva sus flechas. Sin selección: ↓ primera fila, ↑ última.
- Pendiente de verificar a mano: que tras hacer clic en el explorador el editor pierda el foco y las flechas muevan el árbol.
- `Tabs::{tabs, active, is_pinned}` llevan `#[allow(dead_code)]` hasta que la use US-3.1.
- Verificación 👁 AC1–AC3 confirmada manualmente por el propietario el 2026-11-23.

## US-2.3 Operaciones de archivo — ✅ completa (rama feat/us-2.3-operaciones)
Plan: `mdx-core::fileops` (validación, crear, renombrar, papelera, revelar, `rebase_path`), `mdx-app` (`explorer.rs` menú contextual y entrada de nombre en línea; `app.rs` `update_explorer`, `OpOutcome`).
- AC1 👁 clic derecho en una fila (archivo, carpeta) o en el nombre de la carpeta raíz abre el menú: Nuevo archivo, Nueva carpeta, Renombrar, Eliminar, Revelar en el sistema (la raíz no ofrece renombrar/eliminar). Clic derecho en espacio vacío abre el menú de filtros de US-2.1.
- AC2 🧪/👁 `fileops::move_to_trash` delega en `trash::delete`; no hay `remove_file`/`remove_dir*` sobre archivos del usuario (grep limpio salvo el temporal propio del guardado atómico). La prueba unitaria usa un «trasher» inyectado para no ensuciar la papelera real del desarrollador; el envío real a la papelera se verifica a mano. Confirmación con diálogo nativo «Mover a la papelera / Cancelar».
- AC3 🧪 el documento abierto sigue la ruta tras renombrar él o una carpeta padre (conserva contenido y estado «sin guardar»).
- AC4 🧪 validación sin tocar disco (vacío, `.`/`..`, `/ \ : * ? " < > |`, controles, punto o espacio final, nombres reservados de Windows, > 255 bytes) y duplicados rechazados con `create_new`/comprobación previa; el error se muestra en línea bajo el campo.
Decisiones:
- «Nuevo archivo» sin extensión añade `.md`.
- Tras crear/renombrar/eliminar se recarga el directorio padre (el watcher de US-2.4 lo hará solo más adelante); crear expande la carpeta y selecciona el elemento. El archivo nuevo no se abre automáticamente.
- Eliminar el archivo abierto conserva su contenido en el editor con `DiskState::Deleted`; la marca «(eliminado)» visible llega en US-2.4 AC4.
- Renombrar un archivo con cambios sin guardar es seguro: no se toca el contenido, solo la ruta.
- Renombrar solo mayúsculas/minúsculas se permite incluso en sistemas de archivos que no las distinguen.
- «Revelar»: `open -R` (macOS), `explorer /select,` (Windows), `xdg-open <carpeta>` (Linux).
- `update` se dividió: `update_explorer` agrupa los mensajes del explorador (límite de líneas de clippy).
- Verificación 👁 AC1 y AC2 confirmada manualmente por el propietario el 2026-11-23.

## US-2.4 Cambios externos — ✅ completa (rama feat/us-2.4-cambios-externos)
Plan: `mdx-core::watcher` (notify-debouncer-full 200 ms → `FsEvent`), `mdx-app/app.rs` (subscripciones `run_with`, `handle_fs`, `handle_disk_inspected`, banner), `explorer::dirs_to_reload`.
- AC1 🧪 `watcher::tests::*` + watcher real sobre `TempDir` (macOS). El árbol relee solo los directorios **ya cargados** que contienen el cambio.
- AC2 🧪 hash del disco ≠ `saved_hash` y documento limpio → se sustituye el contenido en silencio (se vacía el historial de deshacer, porque ya no corresponde al texto).
- AC3 🧪/👁 documento sucio → `DiskState::ChangedExternally` y banner con **Recargar** (descarta lo local) y **Conservar mío** (el disco pasa a ser la referencia de «guardado»; el texto sigue sin guardar y el siguiente guardado lo sobrescribe a conciencia). Un segundo aviso con el mismo contenido de disco no vuelve a molestar.
- AC4 🧪/👁 el archivo desaparece → `DiskState::Deleted`, título «a.md (eliminado) — Mdx» y aviso en la barra de estado; el contenido se conserva y guardar lo restituye. La marca va en el título porque aún no hay barra de pestañas (US-3.1).
- AC5 🧪 INV-3: se compara el hash del disco con `saved_hash` (cubre nuestros propios guardados atómicos y también ediciones posteriores al guardado).
Decisiones:
- Se vigila la carpeta abierta (recursivo) y, si el documento está **fuera** de ella (CLI/Ctrl+O), su directorio (no recursivo): sin esto los AC2–AC5 no cubrirían los archivos abiertos sin carpeta.
- Toda decisión sobre el documento se toma releyendo el disco en un `Task` (`DiskInspected`), no confiando en el tipo de evento: robusto ante las diferencias entre FSEvents/inotify/ReadDirectoryChanges y ante carreras con renombrados propios (US-2.3). Los resultados de lecturas viejas (otro documento o ruta) se ignoran.
- Se descartan eventos dentro de `.git`, accesos y metadatos.
- Un archivo borrado que reaparece con otro contenido se trata como cambio externo (recarga silenciosa o banner).
- Si el observador no puede iniciarse se informa en la barra de estado.
- Solo probado en macOS (FSEvents). Linux (inotify) y Windows quedan para el QA de F5 / CI.
- Verificación 👁 AC3 y AC4 confirmada manualmente por el propietario el 2026-11-23.

## Cierre de F2 → v0.3.0 (2026-11-23)
- Historias F2: US-2.1, US-2.2, US-2.3 y US-2.4 completas (AC verificados).
- Hito F2 verificado manualmente por el propietario (navegar y editar el repo; cambio externo manejado).
- F2 no exige NFR de cierre (la tabla de NFR se mide en F1, F3 y F5).
- Siguen abiertos: Hito F1 y comprobaciones 👁 de NFR-2/3/4 (F1); CI remoto (AC2 de US-0.1); licencia y URL del repositorio en `TBD`. Watcher, papelera y «revelar» solo probados en macOS.
- Tag `v0.3.0` y `v0.2.0`: **no creados**; requieren autorización explícita.

## US-3.1 Pestañas — ✅ completa (rama feat/us-3.1-pestanas)
Plan: dividir `app.rs` (commit de refactor aparte), `mdx-app/src/app/documents.rs` (activo + aparcados), `state/tabs.rs` (orden, vecinos, `nth`), barra de pestañas en `app/mod.rs`, atajos en `shortcuts.rs`; ADR 0006.
- AC1 🧪 varios documentos en pestañas con estado independiente (texto, historial, dirty, conflicto, vista previa); Ctrl/Cmd+W y botón × cierran la pestaña, con diálogo Guardar/Descartar/Cancelar si tiene cambios; «Guardar» muestra esa pestaña, guarda y la cierra.
- AC2 🧪 Ctrl+Tab / Ctrl+Shift+Tab (Control también en macOS) cíclico; Ctrl/Cmd+1..8 salta a esa posición y **9 salta a la última** (como los navegadores). Los dígitos también se reconocen por tecla física (teclados donde la fila numérica exige Shift).
- AC3 🧪 `●` en la pestaña sucia, «(eliminado)» si se borró en disco, cursiva para la pestaña «preview» y tooltip con la ruta completa (o «Sin título»).
Decisiones:
- **Abrir ya no reemplaza ni pregunta:** el aviso de cambios sin guardar pasa a Cerrar pestaña / Cerrar aplicación. Cerrar la aplicación revisa todas las pestañas: «Descartar» cierra esa pestaña y vuelve a preguntar por la siguiente (si luego se cancela, las ya descartadas no vuelven).
- Un «Sin título» vacío y sin tocar se reemplaza al abrir algo; Ctrl+N no duplica «Sin título» vacíos.
- Editar fija la pestaña preview (nunca se reemplaza una con cambios).
- Al cerrar la última pestaña queda un «Sin título» vacío.
- Eventos de guardado, disco y vista previa llevan `DocumentId` y funcionan sobre pestañas en segundo plano (`with_doc`); los renombrados, borrados y cambios externos afectan a todas las pestañas.
- Vigilancia: se vigilan los directorios de **todos** los documentos abiertos que están fuera de la carpeta.
- La barra de pestañas hace scroll horizontal si no caben.
- ADR 0006 documenta el diseño «activo + aparcados» frente a `HashMap<DocumentId, Content>` de ARCHITECTURE.
- `app.rs` (1 800 líneas) → `app/{mod,documents,editing,disk,explorer_ops,preview,tests}.rs`, sin cambios de comportamiento (commit `refactor(app)`).
- Verificación 👁 AC1–AC3 confirmada manualmente por el propietario el 2026-11-23.

## US-3.2 Outline — ✅ completa (rama feat/us-3.2-outline)
Plan: `mdx-core::outline` (`pulldown-cmark` + `into_offset_iter`), `mdx-app` (`outline.rs` panel, conmutador en `app/mod.rs`, `GoToLine` en `app/editing.rs`, cálculo junto al parseo de la vista previa).
- AC1 👁 conmutador «Archivos | Esquema» en la cabecera del panel izquierdo (acordado con el propietario); lista de H1–H6 indentada 12 px por nivel; el esquema es el del documento activo (viaja con cada pestaña).
- AC2 🧪/👁 clic → `Content::move_to` a (línea, 0) + un `Motion::Home` para que el editor desplace la vista hasta el cursor. Verificar a mano que el desplazamiento funciona en documentos largos.
- AC3 🧪 extracción probada con tests y un snapshot `insta` (`snapshots/…realistic_document.snap`); se actualiza con el mismo debounce de la vista previa (150 ms).
Decisiones:
- Reconoce ATX y Setext; ignora los encabezados dentro de bloques de código y del bloque de metadatos YAML/TOML inicial; texto plano (negrita, énfasis, enlaces y código en línea sin marcas).
- El esquema se calcula **siempre** en el mismo `spawn_blocking` del parseo (aunque la vista previa esté oculta); los elementos de la vista previa solo si está visible. La suscripción del debounce ya no depende de que la preview esté visible.
- Un encabezado vacío se muestra como «(sin texto)».
- Pendiente de decidir más adelante: marcar en el esquema el encabezado actual según el cursor (no está en el PRD).
- `insta` entra en uso (versión 1.49 de STACK.md); `cargo insta review` no está instalado, los tests no lo necesitan.
- Verificación 👁 AC1 y AC2 confirmada manualmente por el propietario el 2026-11-23.

## US-3.3 Scroll sincronizado — ✅ completa (rama feat/us-3.3-scroll-sincronizado)
Plan: investigación previa (informada al propietario) → ADR 0007 (feature `advanced`, aprobada el 2026-11-23); `mdx-core::outline::top_level_blocks`, `mdx-app/src/preview_sync.rs`, `app/sync.rs`, `editor::top_visible_line`.
- AC1 cómo funciona: (1) cada `Scroll`/`Move`/edición sondea la primera fila visible del editor (clic virtual en (0,0) + restaurar cursor); (2) tras cada parseo, cambio de pestaña o de tamaño se mide con una `Operation` dónde quedó cada bloque renderizado; (3) `target_offset` ubica la línea en su bloque (interpolando dentro) y se hace `scroll_to` en la vista previa. Sin medidas coincidentes se usa una proporción por líneas.
- AC2 casilla «Sincronizar scroll con el editor» en la cabecera de la vista previa (activada por defecto). Sin atajo de teclado (no está en el PRD); la casilla no es accesible por teclado (NFR-7, a revisar en F3).
Evidencia de la investigación: bloques `pulldown-cmark` vs. `Item` de iced coinciden 1:1 en los 9 documentos `.md` del repo (ahora test de regresión `source_blocks_match_the_preview_items_one_to_one_for_the_repo_docs`); el prototipo midió 25 bloques en `gfm_showcase.md` con posiciones crecientes (contenido 1706 px, vista 705 px); el sondeo devuelve la línea 10 tras `Scroll{10}` y la 35 tras 25 más.
Decisiones y límites:
- Unidireccional (editor → vista previa).
- La medición se repite al cambiar el tamaño de la ventana o de los paneles (`window::resize_events`, `PaneResized`).
- Si la sincronía está desactivada o la vista previa oculta no se mide ni se desplaza.
- Con documentos muy grandes (`large_5mb.md`) el recorrido del árbol tras cada parseo puede notarse: medir en el cierre de F3 (NFR-4).
- `PreviewViewer` mínimo propio (solo enlace → mensaje); el resto del aspecto es el predeterminado de iced.
- Verificación 👁 AC1 y AC2 confirmada manualmente por el propietario el 2026-11-23.

## US-3.4 Buscar y reemplazar — ✅ completa (rama feat/us-3.4-buscar-reemplazar)
Plan: `mdx-core::search` (`SearchQuery`, `find_all`, `replace_all`, `replacement_for`, `search_workspace`), `mdx-app` (`search.rs` vistas, `app/search_ops.rs`, atajos), `history::pos_to_offset`, `CoreError::InvalidPattern`.
- AC1 🧪/👁 Ctrl/Cmd+F abre la barra sobre el editor (campo enfocado, prellena con la selección de una línea); Ctrl+H abre también el campo de reemplazo (**en macOS Cmd+Opt+F**, porque Cmd+H es «ocultar» del sistema, según PRD §10). Buscar selecciona la coincidencia en el editor; ↑/↓ (o Enter) navegan con vuelta al principio/final; contador «2 de 5»; Esc cierra. «Reemplazar» sustituye la actual y salta a la siguiente; «Reemplazar todo» lo hace en **un solo paso de deshacer**. Reemplazos como ediciones normales (historial, `●`, vista previa).
- AC2 🧪 distinguir mayúsculas, palabra completa (`\b`) y regex; en modo regex el reemplazo admite `$1`/`$nombre`; en modo literal `$` no se interpreta. Un regex inválido muestra el motivo en rojo y no hay coincidencias. Las coincidencias vacías (p. ej. `x*`) se ignoran.
- AC3 🧪/👁 Ctrl/Cmd+Shift+F muestra la pestaña «Buscar» del panel izquierdo; se lanza con Enter o el botón, en `spawn_blocking` y es **cancelable** (flag atómico que se comprueba entre archivos y cada 1 024 líneas); una búsqueda nueva cancela la anterior y se descartan las respuestas viejas por generación. Resultados agrupados por archivo (ruta relativa) con «línea: extracto»; clic → abre el archivo (o activa su pestaña) y selecciona la coincidencia.
Decisiones y límites:
- Se buscan archivos Markdown (`md`, `markdown`, `mdown`, `mkd`) y `txt`; se respeta `.gitignore` y se omiten ocultos, no-UTF-8 y archivos > 8 MB. Máximo 5 000 coincidencias (aviso «hay más»).
- La búsqueda en carpeta lee **del disco**, no de los cambios sin guardar de las pestañas.
- En el documento el regex puede abarcar varias líneas (`(?m)`); en la carpeta se busca línea a línea.
- Las coincidencias del documento se recalculan al escribir, cambiar de pestaña o abrir/recargar un documento mientras la barra está abierta (O(n) por pulsación; medir en NFR-4).
- Sin resaltado de **todas** las coincidencias en el texto (el `text_editor` de iced no permite pintar rangos arbitrarios): solo se selecciona la actual y la barra muestra el contador.
- Reemplazar todo pega el texto completo en una operación (`Edit::Paste`): con documentos de varios MB puede tardar.
- Verificación 👁 AC1 y AC3 confirmada manualmente por el propietario el 2026-11-23.

## US-3.5 Autoguardado y sesión — ✅ completa (rama feat/us-3.5-autoguardado-sesion)
Plan acordado con el propietario: `mdx-core::config` **mínimo** (solo `[files]`) ahora y completo en US-4.2; el autoguardado se activa editando `config.toml` a mano hasta que US-4.2 añada `Ctrl/Cmd+,`.
Implementación: `mdx-core::{paths, config, session}`, `mdx-app/src/app/session_ops.rs`, `SessionState` en `App`, `layout::{current_ratios, apply_ratios}`, «Recientes» en `explorer.rs`.
- AC1 🧪 off por defecto; `autosave_delay_ms` 500..=10000 (2000 por defecto, fuera de rango se acota con aviso). Un temporizador de 250 ms solo corre si hay documentos con ruta y cambios; guarda los que llevan ≥ retraso sin cambios, **incluidas pestañas en segundo plano**. Nunca guarda: sin título, limpios, con conflicto (`ChangedExternally`) ni borrados en disco (no pisa cambios externos ni resucita archivos), ni con un guardado en curso. Si falla, reinicia la espera (no reintenta en bucle) y muestra el error.
- AC2 🧪 `session.toml` (DATA §4.2) en el directorio de estado del SO, escrito con `write_atomic`. Se guarda en segundo plano cuando cambia la carpeta, las pestañas/activa, las carpetas expandidas o los recientes (no en cada pulsación ni mientras se arrastra un divisor) y **siempre al cerrar** (`run(Close)` espera a que termine antes de salir). No se guarda hasta haber leído la sesión anterior.
- AC3 🧪 sin argumento: restaura carpeta, pestañas en su orden (todas fijas), pestaña activa, carpetas expandidas (de fuera hacia dentro a medida que cargan) y proporciones de paneles. INV-5: lo que no existe o no se lee se omite sin error; `active_tab` se acota. Un `session.toml` corrupto o de esquema más nuevo se ignora con un aviso y **no se sobrescribe al arrancar** (sí al siguiente guardado normal).
- AC4 🧪 Recientes: máx. 10 carpetas y 10 archivos, más reciente primero, sin duplicados, podados al arrancar; se muestran (con la ruta en tooltip) en el panel Archivos mientras no haya carpeta abierta.
Prueba de extremo a extremo (manual, con `MDX_STATE_DIR`/`MDX_CONFIG_DIR` temporales): abrir un archivo por CLI escribe `session.toml` con pestañas y recientes; arrancar sin argumento reabre la sesión sin errores.
Decisiones y puntos a revisar:
- **Decisión del propietario (2026-11-23): con argumento CLI también se restaura la sesión** (cambio respecto al literal «sin argumentos» del AC3, para no perder pestañas al abrir un `.md` con doble clic). El archivo del argumento se abre al final, fijado y activo (sin duplicarlo si ya estaba en la sesión); una carpeta del argumento sustituye a la de la sesión y a sus carpetas expandidas. Sin directorio de estado (sin sesión) el argumento se abre de inmediato. Prueba de extremo a extremo con directorios temporales: tres arranques con `a.md`, `b.md` y `c.md` acumulan las tres pestañas y un cuarto sin argumento las restaura.
- Directorios: macOS `~/Library/Application Support/mx.doitconsulting.mdx/`; Linux `~/.config/mdx/` y `~/.local/state/mdx/`; Windows `%APPDATA%\doitconsulting\mdx\config\`. Variables `MDX_CONFIG_DIR`/`MDX_STATE_DIR` para sustituirlos.
- Los recientes no se muestran con una carpeta abierta (se llega a ellos al cerrar la carpeta); un menú «Recientes» global queda para más adelante si hace falta.
- `clippy::assert_is_empty` permitido solo en los tests (`cfg_attr(test)`) de `mdx-core` y `mdx-app`.
- Verificación manual de sesión, recientes y autoguardado confirmada por el propietario el 2026-11-23.

## Cierre de F3 — mediciones y hallazgos (rama chore/cierre-f3)
- **Corregido: «Reemplazar todo» tardaba 11 s con 1 MB y 261 s con 5 MB** (pegar megabytes en el `text_editor` es cuadrático en `cosmic-text`). Ahora el reemplazo masivo y los parches grandes de deshacer/rehacer (> 64 KB) **reconstruyen el editor** (`apply_bulk_edit`, `rebuild_editor`): 0,76 s / 3,5 s. También `History` ya no descarta la última edición aunque supere el presupuesto de 8 MB (a 5 MB «reemplazar todo» no se podía deshacer).
- Medición NFR-4 reproducible: `cargo xtask gen-fixtures` y `cargo test --release -p mdx-app perf_ -- --ignored --nocapture --test-threads=1` (en paralelo se estorban por el bloqueo global del sistema de fuentes).
- **NFR-3 NO se cumple con un archivo de 100 KB (176 MB frente al objetivo < 80 MB).** Desglose medido con `footprint` (macOS, release): reposo 63 MB → archivo mínimo 82 MB (el resaltador carga ≈19 MB) → **archivo de 100 KB 176 MB**: ≈61 MB el buffer del editor (`cosmic-text` guarda glifos y capas de todo el texto) y ≈33 MB la vista previa (un `rich_text` por bloque, todos construidos aunque no se vean). La medición de F1 (63 MB) era solo en reposo, sin archivo. Conseguirlo exige virtualizar editor y vista previa (renderizar solo lo visible), o un `ropey` + viewport (riesgo ya previsto en PRD §11), o redefinir el NFR: **decisión del propietario** (CLAUDE.md §11).
- NFR-1: 12,39 MB (límite 15 MB; creció 0,9 MB desde F1). NFR-2: ≈145–180 ms hasta wgpu listo. NFR-9: mdx-core ≥ 79 % por archivo (workspace 78 %).
- Las medidas de memoria se hicieron con `MDX_CONFIG_DIR`/`MDX_STATE_DIR` temporales: al restaurar una sesión previa (con otro archivo abierto) el footprint sube, así que hay que medir con sesión vacía.

## Cierre de F3 → v0.4.0 (2026-11-23)
- Historias F3: US-3.1 a US-3.5 completas (AC verificados).
- **NFR-3 incumplido (176 MB con un archivo de 100 KB frente a < 80 MB)**: decisión del propietario (opción 3): cerrar la F3 con el NFR marcado como no cumplido y valor medido, y decidir antes de la F4 entre virtualizar editor y vista previa (ADR) o redefinir el NFR en el PRD. La F5 vuelve a exigirlo.
- Otros NFR de F3: ver tabla de CHECKLIST (NFR-2, NFR-4 y NFR-7 en `[~]`).
- Hito F3 en `[~]`: verificadas las cinco historias por separado; falta un recorrido único de sesión.
- Siguen abiertos de fases anteriores: Hito F1 y comprobaciones 👁 de NFR-2/3/4; CI remoto (AC2 de US-0.1); licencia y URL del repositorio en `TBD`.
- Tags `v0.2.0`, `v0.3.0` y `v0.4.0`: **no creados**; requieren autorización explícita.

## US-4.2 Configuración — ✅ completa (rama feat/us-4.2-configuracion)
Plan: ampliar `mdx-core::config` al esquema completo de DATA §4.1, `Config::ensure_file` + plantilla, `Ctrl/Cmd+,`, recarga al guardar, aplicar los ajustes.
- AC1 🧪 `paths::app_files()` (directories) + `MDX_CONFIG_DIR`/`MDX_STATE_DIR`.
- AC2 🧪 ajustes: se aplican `font_family` (si no está instalada → monoespaciada del sistema, comprobado contra el sistema de fuentes de iced), `font_size`, `line_wrap` (`Wrapping::Word`/`None`), `autosave`, `autosave_delay_ms`, filtros del explorador (si cambian y hay carpeta abierta se relee), `preview.visible` y `preview.sync_scroll`. **Tema y zoom** se leen/validan pero se aplican en US-4.1/US-4.5; **`tab_width`/`insert_spaces`** quedan reservados (iced 0.14 no deja configurarlos).
- AC3 `Ctrl/Cmd+,` crea la plantilla comentada si el archivo no existe (nunca sobrescribe) y lo abre fijado; al **guardar ese archivo** (a mano o por autoguardado) se recarga y se aplica, con «Configuración aplicada» o el aviso correspondiente. Un cambio de `preview.visible`/`sync_scroll` al recargar sustituye lo que el usuario hubiera alternado a mano.
- AC4 🧪 INV-6 (`invalid_not_overwritten`): TOML inválido, tipos erróneos, tema desconocido o esquema más nuevo → valores por defecto + aviso, sin tocar el archivo; fuera de rango → se acota (aviso por cada campo). Claves desconocidas: se ignoran y solo se registran con `tracing::warn!`.
Decisiones:
- La configuración se lee antes que la sesión (`Task::chain`) para que los filtros del explorador ya estén aplicados al restaurar la carpeta.
- `font_family` por defecto pasa de «JetBrains Mono» (DATA) a `monospace`: no se distribuye ninguna fuente y una fuente ausente caería en una proporcional. Actualizado DATA §4.1.
- `FilesConfig` lleva `#[allow(clippy::struct_excessive_bools)]`: refleja el esquema fijado en DATA.
- No hay recarga por cambios externos de `config.toml` (solo al guardarlo desde Mdx); no está en el PRD.
- Verificación 👁 AC3 confirmada manualmente por el propietario el 2026-11-23.

## US-4.1 Temas — ✅ completa (rama feat/us-4.1-temas)
Plan: `theme.rs` (resolución del tema y del tema de resaltado), modo del sistema vía `iced::system::{theme, theme_changes}`, ajuste `appearance.theme` de US-4.2.
- AC1 👁 `system` (por defecto) sigue al SO —consulta el modo al arrancar y se suscribe a sus cambios, sin reiniciar—; `light`/`dark` lo fijan. Si el SO no informa su modo se usa el claro. Cambiar el ajuste y guardar `config.toml` lo aplica al instante (US-4.2 AC3). En este Mac el SO informa `Light`.
- AC2 👁 toda la interfaz sale del `Theme` de iced (`markdown::Settings::from(&theme)` para la vista previa) y el resaltado del editor cambia con él (`InspiredGitHub` en claro, `Base16Mocha` en oscuro).
Decisiones:
- Solo dos temas (claro/oscuro) como pide el PRD; sin selector en la interfaz: se cambia en `config.toml`.
- Pendiente de comprobar a mano: contraste de los colores propios (resaltado de elementos del explorador, banners, casillas) en oscuro, y el resaltado de bloques de código de la vista previa en oscuro.
- Verificación 👁 AC1 y AC2 confirmada manualmente por el propietario el 2026-11-23.

## US-4.5 Zoom — ✅ completa (rama feat/us-4.5-zoom)
Plan: `app/zoom.rs` (pasos y límites), atajos en `shortcuts.rs`, tamaño efectivo en el editor y la vista previa, persistencia en `session.toml`.
- AC1 👁 `Ctrl/Cmd +` (también `=` sin Shift y el `+` del teclado numérico), `-` y `0`. Cada paso es del 10 % (se redondea a décimas para no acumular error), con tope 50 %–300 % (rango de `appearance.zoom`). `0` restablece al zoom de `config.toml`. Se escala el tamaño de fuente del editor (`font_size × zoom`) y el texto de la vista previa (16 px × zoom, con los encabezados proporcionales); el resto de la interfaz (explorador, barra de estado) no cambia, como pide el AC. Tras cada cambio se vuelve a medir la vista previa para mantener el scroll sincronizado.
Decisiones:
- **Persistencia:** el zoom elegido con el teclado se guarda en `session.toml` (campo opcional `zoom`, compatible con sesiones antiguas) y al arrancar prevalece sobre `appearance.zoom`; recargar `config.toml` (guardarlo) lo descarta y vuelve al valor del archivo. Es una ampliación menor de DATA §4.2 (actualizada).
- `Ctrl/Cmd+0` ya no es «ir a la pestaña 0» (nunca lo fue: las pestañas son 1..9).
- Mdx nunca escribe `appearance.zoom` en `config.toml` (INV-6).
- Verificación 👁 AC1 confirmada manualmente por el propietario el 2026-11-23.

## US-4.3 Exportar HTML — ✅ completa (rama feat/us-4.3-exportar-html)
Plan: `mdx-core::export` (`comrak` con GFM, CSS embebido), botón y atajo en la app, guardado atómico en un `Task`.
- AC1 👁 «Exportar HTML» en la cabecera de la vista previa o `Ctrl/Cmd+Shift+E`: diálogo «Guardar como» (sugiere `<nombre>.html` junto al documento) y exporta el **texto actual** (incluidos los cambios sin guardar). El HTML es autocontenido: un solo archivo con `<style>` embebido (GitHub-like, claro/oscuro según `prefers-color-scheme`), sin `<link>`, scripts ni fuentes externas. Soporta tablas, tachado, listas de tareas, autoenlaces, notas al pie, bloques de código con `class="language-…"` y ids de encabezado. El bloque YAML inicial (`---`) no se emite.
- AC2 🧪 dos snapshots `insta` (cuerpo y documento completo) + pruebas de seguridad (el `<script>` y los enlaces `javascript:` no salen: modo seguro de comrak, `render.unsafe = false`).
Decisiones y límites:
- Se interpretó «Menú Exportar → HTML» como botón + atajo: la aplicación no tiene barra de menús. El atajo `Ctrl/Cmd+Shift+E` no está en el PRD §10 (se añade por NFR-7).
- **Sin resaltado de sintaxis** en el HTML exportado (comrak sin `syntect`, para no sumar dependencias pesadas); los bloques de código salen monoespaciados con la clase `language-…` por si el usuario añade un resaltador.
- Las **imágenes** con ruta relativa se exportan como enlaces (no se incrustan); incrustarlas sería una mejora posterior. `aria-label` de los anclajes de encabezado sale en inglés (texto fijo de comrak).
- Nueva dependencia `comrak 1.2` (ya prevista en STACK, `default-features = false`) → licencia transitiva `Unicode-DFS-2016` (de `finl_unicode`) añadida a `deny.toml` y STACK §8, equivalente a `Unicode-3.0`.
- `app/mod.rs`: `view` se dividió en `editor_pane`/`preview_pane` por límite de líneas de clippy.
- Verificación 👁 AC1 confirmada manualmente por el propietario el 2026-11-23.

## US-4.4 Imágenes — ✅ completa (rama feat/us-4.4-imagenes)
Plan: `mdx-core::assets` (rutas, nombres, PNG, guardado), visor de imágenes en la vista previa, `Cmd/Ctrl+V` inteligente en el editor, `arboard` en un hilo aparte; ADR 0008; fixtures `fixtures/imagenes.md` y `fixtures/assets/`.
- AC1 👁 el visor de la vista previa (`PreviewViewer::image`) resuelve `ruta/relativa.png` contra la carpeta del documento (decodifica `%20`, quita `?` y `#`), dibuja `png/jpg/gif/webp/bmp` con el widget de imagen (escalado hacia abajo al ancho del panel) y `svg` con el widget vectorial. Remotas (`http`, `https`, `data:`…), inexistentes o de otro tipo muestran «Imagen no disponible: …». La existencia se comprueba al parsear (en el hilo de fondo), no en `view`. Si el documento se guarda en otra carpeta se vuelve a parsear.
- AC2 🧪/👁 con el editor enfocado, `Cmd/Ctrl+V` consulta el portapapeles (hilo aparte): con imagen → `assets/AAAAMMDD-N.png` (N = primer número libre del día, sin sobrescribir) y se inserta `![](assets/…)` en el cursor (una sola edición deshacible); sin imagen → pegado de texto normal con saltos de línea normalizados. Otros campos de texto no se ven afectados. Documento sin guardar → aviso para guardarlo primero.
Decisiones y límites:
- Fecha de los nombres en **UTC** (sin dependencia de fechas); cerca de la medianoche local puede diferir del día local.
- Remotas no se descargan (sin red en la app). No se incrustan imágenes en el HTML exportado (US-4.3).
- Linux: `arboard` puede requerir su backend Wayland/X11; a verificar en el QA de F5.
- Solo se pega la **primera** imagen/captura del portapapeles; archivos copiados desde el Finder (rutas) no se interpretan como imagen.
- Verificación 👁 AC1 y AC2 confirmada manualmente por el propietario el 2026-11-23.
