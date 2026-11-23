# PRD — Mdx: Editor ligero de Markdown en Rust

| Campo | Valor |
|---|---|
| Producto | Mdx (nombre de trabajo) |
| Versión del documento | 1.0 |
| Fecha | 2026-11-23 |
| Autor | Dante Robles — DoIT Consulting |
| Estado | Aprobado para implementación |
| Metodología | Spec Driven Development (SDD) |

---

## 1. Resumen

Mdx es un editor de Markdown de escritorio, **ligero, nativo y multiplataforma** (Windows, macOS, Linux), escrito 100 % en Rust. Combina un **explorador de archivos** orientado a carpetas/repositorios, un **editor de texto** con resaltado y una **vista previa en vivo**. Es la alternativa rápida a editores basados en Electron (Typora, Obsidian, MarkText) para quien trabaja con documentación, notas y READMEs dentro de repositorios.

## 2. Problema

- Los editores Markdown populares usan Electron: arrancan lento, consumen 300–800 MB de RAM y pesan más de 100 MB.
- Los editores de código (VS Code, Zed) resuelven Markdown como un caso secundario: la vista previa es un panel extra y el flujo de "escribir documentación" no es el centro.
- Se necesita una herramienta que abra una carpeta en < 1 s, edite y previsualice sin fricción, y no compita por recursos con el resto del entorno de desarrollo.

## 3. Objetivos

| ID | Objetivo | Métrica |
|---|---|---|
| G1 | Ligero | Binario empaquetado < 15 MB; RAM en reposo < 80 MB |
| G2 | Rápido | Arranque en frío < 300 ms; escritura sin lag (< 16 ms/frame) en archivos de 1 MB |
| G3 | Multiplataforma real | Mismo binario funcional en Windows 10+, macOS 13+, Linux (X11 y Wayland) |
| G4 | Seguro con los datos | Cero pérdida de datos: guardado atómico, detección de cambios externos, papelera en lugar de borrado |
| G5 | Fiel al estándar | Vista previa CommonMark + GFM (tablas, tareas, tachado, autolinks) |

## 4. No-objetivos (v1)

- No es un editor WYSIWYG (estilo Typora). El modelo es **fuente + vista previa**.
- Sin sincronización en la nube, cuentas ni colaboración en tiempo real.
- Sin sistema de plugins.
- Sin multicursor, plegado de código ni LSP.
- Sin exportación a PDF/DOCX (se evalúa en v1.1).
- Sin soporte móvil ni web.

## 5. Usuarios objetivo

| Persona | Necesidad principal |
|---|---|
| **Desarrollador** | Editar README, docs/ y specs (PRD, CLAUDE.md) dentro de repos, respetando `.gitignore` |
| **Escritor técnico / consultor** | Escribir documentación y entregables en Markdown con vista previa fiel |
| **Usuario de notas** | Mantener una carpeta de notas `.md` y navegarla rápido |

## 6. Stack técnico (decisión cerrada)

| Área | Elección | Notas |
|---|---|---|
| Lenguaje | Rust stable (edition 2024) | MSRV = la que exija iced 0.14 |
| UI | `iced` 0.14 | `text_editor`, `markdown`, `pane_grid`, `scrollable` |
| Parseo para preview | `pulldown-cmark` (vía widget `markdown` de iced) | Parseo incremental |
| Exportación HTML | `comrak` | 100 % CommonMark + GFM |
| Resaltado de código | `iced_highlighter` (syntect) | En editor y bloques de código |
| Recorrido de carpetas | `ignore` | Respeta `.gitignore` |
| Watcher | `notify` + `notify-debouncer-full` | |
| Diálogos nativos | `rfd` | |
| Papelera | `trash` | |
| Rutas de config | `directories` | |
| Configuración | `serde` + `toml` | |
| Errores | `thiserror` (core), `anyhow` (app) | |
| Logs | `tracing` + `tracing-subscriber` | |
| Tests | `cargo test`, `insta` (snapshots), `tempfile` | |
| Empaquetado | `cargo-packager` | MSI, DMG, AppImage, DEB |

## 7. Arquitectura

```
mdx/
├── Cargo.toml                 # workspace
├── crates/
│   ├── mdx-core/              # lógica pura, SIN dependencia de iced
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── document.rs    # buffer, dirty state, line endings, encoding
│   │       ├── io.rs          # lectura/escritura atómica
│   │       ├── workspace.rs   # árbol de archivos, filtros, búsqueda
│   │       ├── watcher.rs     # notify → FsEvent
│   │       ├── outline.rs     # extracción de encabezados (TOC)
│   │       ├── export.rs      # comrak → HTML
│   │       ├── session.rs     # restauración de sesión
│   │       └── config.rs      # settings, tema, recientes
│   └── mdx-app/               # UI con iced
│       └── src/
│           ├── main.rs
│           ├── app.rs         # State, Message, update, view, subscription
│           ├── views/{explorer,tabs,editor,preview,outline,statusbar,search}.rs
│           ├── shortcuts.rs
│           └── theme.rs
├── assets/                    # íconos, fuentes
├── fixtures/                  # .md de prueba (incluye archivo de 5 MB generado)
└── docs/                      # PRD.md, decisiones (ADR)
```

### 7.1 Principios de arquitectura

1. **Separación core/UI**: `mdx-core` no importa `iced`. Toda regla de negocio se prueba sin UI.
2. **Arquitectura Elm (iced)**: estado único, `Message` enum, `update` puro salvo `Task`s, `view` sin efectos.
3. **Nada de I/O bloqueante en `update`**: lectura, escritura, recorrido de carpetas y exportación van en `Task::perform` (async).
4. **Eventos externos por `Subscription`**: watcher de archivos, timer de autoguardado, eventos de teclado globales.
5. **Debounce de preview**: re-parsear 150 ms después de la última edición.

### 7.2 Modelo de datos principal

```rust
// mdx-core
pub struct DocumentId(u64);

pub struct Document {
    pub id: DocumentId,
    pub path: Option<PathBuf>,   // None = sin guardar
    pub text: String,
    pub saved_hash: u64,         // hash del contenido en disco
    pub line_ending: LineEnding, // Lf | CrLf, detectado al abrir
    pub disk_mtime: Option<SystemTime>,
}
impl Document { pub fn is_dirty(&self) -> bool; }

pub struct Workspace {
    pub root: PathBuf,
    pub tree: FileNode,          // carga perezosa por directorio
    pub show_hidden: bool,
    pub only_markdown: bool,
}

pub enum FsEvent { Created(PathBuf), Modified(PathBuf), Removed(PathBuf), Renamed(PathBuf, PathBuf) }
```

---

## 8. Requerimientos funcionales por fase

Convención: `US-<fase>.<n>` = historia de usuario; `AC` = criterio de aceptación verificable. Una historia está **terminada** solo cuando todos sus AC pasan y tienen test (automático cuando sea posible; manual documentado cuando dependa de UI).

### Fase 0 — Fundaciones

**US-0.1 Workspace y CI**
Como desarrollador, quiero un workspace Cargo con CI para que cada cambio compile y se pruebe en las 3 plataformas.
- AC1: `cargo build --workspace` y `cargo test --workspace` pasan localmente.
- AC2: GitHub Actions ejecuta build + test + clippy + fmt en `ubuntu-latest`, `macos-latest`, `windows-latest`.
- AC3: `cargo clippy --workspace --all-targets -- -D warnings` sin warnings.
- AC4: `cargo deny check` configurado (licencias permitidas: MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, MPL-2.0).

**US-0.2 Ventana base**
Como usuario, quiero que la app abra una ventana con el layout de 3 paneles.
- AC1: `cargo run -p mdx-app` abre una ventana titulada "Mdx".
- AC2: Layout con `pane_grid`: Explorador (izq., 20 %), Editor (centro, 40 %), Preview (der., 40 %).
- AC3: Los divisores se pueden arrastrar.
- AC4: Barra de estado inferior visible (vacía por ahora).

### Fase 1 — MVP del editor

**US-1.1 Nuevo / Abrir / Guardar**
Como usuario, quiero crear, abrir y guardar archivos `.md`.
- AC1: `Ctrl/Cmd+N` crea documento vacío "Sin título".
- AC2: `Ctrl/Cmd+O` abre diálogo nativo (`rfd`) filtrado a `.md`, `.markdown`, `.txt` (con opción "Todos").
- AC3: `Ctrl/Cmd+S` guarda; si no tiene ruta, abre "Guardar como".
- AC4: `Ctrl/Cmd+Shift+S` = Guardar como.
- AC5: **Guardado atómico**: se escribe a archivo temporal en el mismo directorio y se renombra. Test en `mdx-core::io`.
- AC6: Se preserva el final de línea original (LF/CRLF). Test con fixtures de ambos tipos.
- AC7: Archivos no UTF-8 muestran error claro y no se abren en modo editable (v1).

**US-1.2 Estado "sin guardar"**
- AC1: El título de la ventana muestra `● nombre.md — Mdx` cuando hay cambios.
- AC2: Al cerrar la ventana con cambios pendientes aparece diálogo: Guardar / Descartar / Cancelar.
- AC3: Deshacer cambios hasta el estado guardado elimina el indicador (comparación por hash).

**US-1.3 Editor**
- AC1: Edición con `text_editor` de iced, fuente monoespaciada.
- AC2: Resaltado de sintaxis Markdown.
- AC3: Deshacer/rehacer con `Ctrl/Cmd+Z` y `Ctrl/Cmd+Shift+Z` (y `Ctrl+Y` en Windows/Linux).
- AC4: `Tab`/`Shift+Tab` indentan/desindentan la selección.
- AC5: Barra de estado: línea:columna, conteo de palabras, final de línea (LF/CRLF), codificación.

**US-1.4 Vista previa en vivo**
- AC1: La preview se actualiza ≤ 150 ms después de dejar de escribir (debounce).
- AC2: Renderiza: encabezados, énfasis, listas, listas de tareas, citas, código inline y en bloque con resaltado, tablas, enlaces, reglas horizontales.
- AC3: Clic en enlace `http(s)` lo abre en el navegador del sistema (crate `open`); enlaces relativos a `.md` abren ese archivo en el editor.
- AC4: `Ctrl/Cmd+\` alterna la visibilidad de la preview.

**Hito F1**: Se puede editar el README de este mismo repo de principio a fin sin pérdida de datos.

### Fase 2 — Explorador de archivos

**US-2.1 Abrir carpeta**
- AC1: `Ctrl/Cmd+Shift+O` abre diálogo de carpeta; también por argumento CLI: `mdx <ruta>` (archivo o carpeta).
- AC2: El árbol carga **perezosamente**: solo se lee un directorio al expandirlo.
- AC3: Por defecto respeta `.gitignore` y oculta archivos ocultos (toggle en menú contextual del panel).
- AC4: Filtro "solo Markdown" activado por defecto (toggle disponible); carpetas sin `.md` descendientes no se ocultan en v1 (evita recorrido completo).
- AC5: Orden: carpetas primero, luego archivos, alfabético sin distinguir mayúsculas.
- AC6: Abrir un repo con 10 000 archivos no bloquea la UI (recorrido en `Task`). Test de rendimiento en `mdx-core` con `tempfile`.

**US-2.2 Navegación**
- AC1: Clic simple en archivo lo abre en el editor (pestaña preview reutilizable); doble clic lo fija.
- AC2: Teclado: ↑/↓ navegar, →/← expandir/colapsar, Enter abrir.
- AC3: El archivo activo se resalta en el árbol.

**US-2.3 Operaciones de archivo**
- AC1: Menú contextual: Nuevo archivo, Nueva carpeta, Renombrar, Eliminar, Revelar en el sistema (Finder/Explorer/gestor).
- AC2: **Eliminar envía a la papelera** del sistema (`trash`), con confirmación. Nunca `remove_file` directo.
- AC3: Renombrar un archivo abierto actualiza la ruta de su pestaña.
- AC4: Nombres inválidos o duplicados muestran error en línea, sin tocar disco.

**US-2.4 Cambios externos**
- AC1: El watcher (`notify`, debounce 200 ms) refleja altas/bajas/renombres en el árbol.
- AC2: Si un archivo abierto **sin cambios locales** se modifica en disco → se recarga en silencio.
- AC3: Si un archivo abierto **con cambios locales** se modifica en disco → banner: "El archivo cambió en disco. [Recargar] [Conservar mío]".
- AC4: Si un archivo abierto se elimina en disco → la pestaña se marca "(eliminado)" y conserva el contenido para poder re-guardarlo.
- AC5: Los propios guardados de Mdx no disparan el banner (se ignora el evento si el hash coincide con `saved_hash`).

**Hito F2**: Abrir este repo, navegar y editar varios archivos; modificar uno desde otra herramienta y ver la reacción correcta.

### Fase 3 — Productividad

**US-3.1 Pestañas**
- AC1: Múltiples documentos en pestañas; `Ctrl/Cmd+W` cierra (con diálogo si hay cambios).
- AC2: `Ctrl+Tab` / `Ctrl+Shift+Tab` cicla pestañas; `Ctrl/Cmd+1..9` salta.
- AC3: Pestaña muestra `●` si está sucia; tooltip con ruta completa.

**US-3.2 Esquema (outline)**
- AC1: Panel/lista con los encabezados H1–H6 del documento activo, indentados por nivel.
- AC2: Clic en un encabezado mueve el cursor del editor a esa línea.
- AC3: Se actualiza con el mismo debounce de la preview. Extracción probada en `mdx-core::outline` con snapshots (`insta`).

**US-3.3 Scroll sincronizado**
- AC1: Al hacer scroll en el editor, la preview se posiciona en el bloque correspondiente (mapeo por línea de origen de bloques de nivel superior).
- AC2: Toggle para desactivarlo.

**US-3.4 Buscar y reemplazar**
- AC1: `Ctrl/Cmd+F` buscar en documento; `Ctrl/Cmd+H` reemplazar (uno / todos).
- AC2: Opciones: distinguir mayúsculas, palabra completa, regex (`regex` crate).
- AC3: `Ctrl/Cmd+Shift+F` busca en todo el workspace (respeta `.gitignore`), resultados agrupados por archivo con número de línea; clic abre y posiciona. Búsqueda en `Task`, cancelable.

**US-3.5 Autoguardado y sesión**
- AC1: Autoguardado opcional (off por defecto) tras 2 s de inactividad, solo para documentos con ruta.
- AC2: Al cerrar, se guarda la sesión (workspace, pestañas abiertas, pestaña activa, tamaños de paneles) en el directorio de config del SO.
- AC3: Al abrir sin argumentos, se restaura la última sesión. Rutas inexistentes se omiten sin error.
- AC4: Lista de "Recientes" (máx. 10 carpetas y 10 archivos).

### Fase 4 — Pulido

**US-4.1 Temas**
- AC1: Temas claro y oscuro; modo "Sistema" sigue al SO (default).
- AC2: La preview usa la paleta del tema activo.

**US-4.2 Configuración**
- AC1: Archivo `config.toml` en el dir de config del SO (`directories`).
- AC2: Ajustes: fuente y tamaño del editor, ajuste de línea (wrap), autoguardado, tema, mostrar ocultos, respetar `.gitignore`.
- AC3: `Ctrl/Cmd+,` abre el archivo de configuración en una pestaña; se aplica al guardar.
- AC4: Config inválida → se usan defaults y se muestra aviso; nunca crashea.

**US-4.3 Exportar HTML**
- AC1: Menú "Exportar → HTML" genera un `.html` autocontenido (CSS embebido) con `comrak` (GFM habilitado).
- AC2: El HTML resultante se valida con snapshot test.

**US-4.4 Imágenes**
- AC1: Imágenes con ruta relativa al documento se muestran en la preview.
- AC2: Pegar una imagen del portapapeles guarda `./assets/<fecha>-<n>.png` junto al documento e inserta `![](assets/...)`.

**US-4.5 Zoom**
- AC1: `Ctrl/Cmd +/-/0` ajusta tamaño de fuente de editor y preview.

### Fase 5 — Distribución

**US-5.1 Paquetes**
- AC1: `cargo packager --release` genera: `.msi` (Windows), `.dmg` (macOS universal x86_64+arm64), `.AppImage` y `.deb` (Linux x86_64).
- AC2: Workflow de release en GitHub Actions al crear tag `v*`, adjuntando artefactos al Release.
- AC3: Asociación de archivos `.md` / `.markdown` en los instaladores.
- AC4: Ícono de la app en los 3 SO.

**US-5.2 Firma** (requiere credenciales del propietario; el agente NO las gestiona)
- AC1: Pasos de firma (Apple Developer ID + notarización; certificado Windows) documentados en `docs/release.md` y parametrizados por secrets de CI.

---

## 9. Requerimientos no funcionales

| ID | Requerimiento | Verificación |
|---|---|---|
| NFR-1 | Binario release empaquetado < 15 MB | Script `xtask size` en CI, falla si excede |
| NFR-2 | Arranque en frío < 300 ms hasta primer frame | Medición manual documentada por plataforma |
| NFR-3 | RAM en reposo < 80 MB con un archivo de 100 KB abierto | Medición manual documentada |
| NFR-4 | Escritura fluida en archivo de 1 MB; usable con 5 MB | Fixture generado + prueba manual |
| NFR-5 | Ningún `unwrap()`/`expect()` en rutas de I/O o input de usuario | clippy `unwrap_used` = deny en `mdx-core` |
| NFR-6 | Sin `unsafe` en código propio | `#![forbid(unsafe_code)]` en ambos crates |
| NFR-7 | Accesibilidad básica: todo accionable por teclado | Checklist manual por fase |
| NFR-8 | i18n preparada: strings de UI centralizados en un módulo (`strings.rs`), español por defecto | Revisión de código |
| NFR-9 | Cobertura de tests de `mdx-core` ≥ 70 % | `cargo llvm-cov` en CI (informativo) |

## 10. Atajos de teclado (resumen)

| Acción | Windows/Linux | macOS |
|---|---|---|
| Nuevo | Ctrl+N | Cmd+N |
| Abrir archivo | Ctrl+O | Cmd+O |
| Abrir carpeta | Ctrl+Shift+O | Cmd+Shift+O |
| Guardar / Guardar como | Ctrl+S / Ctrl+Shift+S | Cmd+S / Cmd+Shift+S |
| Cerrar pestaña | Ctrl+W | Cmd+W |
| Buscar / Reemplazar | Ctrl+F / Ctrl+H | Cmd+F / Cmd+Opt+F |
| Buscar en workspace | Ctrl+Shift+F | Cmd+Shift+F |
| Alternar preview | Ctrl+\ | Cmd+\ |
| Configuración | Ctrl+, | Cmd+, |
| Zoom | Ctrl +/-/0 | Cmd +/-/0 |

## 11. Riesgos

| Riesgo | Impacto | Mitigación |
|---|---|---|
| `text_editor` de iced lento con archivos grandes | Alto | Fixture de 5 MB desde F1; si falla NFR-4, ADR para evaluar `ropey` + viewport |
| Breaking changes de iced (pre-1.0) | Medio | Fijar `iced = "0.14"`; migraciones como tarea aparte |
| Diferencias Wayland/X11 | Medio | CI Linux + prueba manual en GNOME y KDE |
| Mapeo de scroll impreciso | Bajo | Mapeo por bloque de nivel superior; aceptable en v1 |
| Watcher con eventos duplicados por SO | Medio | Debouncer + comparación de hash |

## 12. Preguntas abiertas

1. Nombre definitivo del producto y bundle ID (`mx.doitconsulting.mdx` propuesto).
2. Licencia: ¿MIT/Apache-2.0 open source o propietaria?
3. ¿Se incluye actualización automática en v1 o v1.1?
4. ¿Idioma de UI solo español en v1, o español + inglés?

## 13. Plan de entregas

| Fase | Duración estimada | Entregable |
|---|---|---|
| F0 | 2–3 días | Workspace, CI, ventana con layout |
| F1 | 1–1.5 semanas | Editor + preview funcional |
| F2 | 1 semana | Explorador + watcher |
| F3 | 1–1.5 semanas | Pestañas, outline, búsqueda, sesión |
| F4 | 1 semana | Temas, config, exportar, imágenes |
| F5 | 3–5 días | Instaladores y release |
| **Total** | **5–7 semanas** | **Mdx v1.0** |
