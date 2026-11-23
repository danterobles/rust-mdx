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
- [ ] AC1 Ctrl/Cmd+N 👁
- [ ] AC2 Ctrl/Cmd+O con filtro 👁
- [ ] AC3 Ctrl/Cmd+S (y Guardar como si no hay ruta) 👁
- [ ] AC4 Ctrl/Cmd+Shift+S 👁
- [ ] AC5 Guardado atómico 🧪 (INV-2)
- [ ] AC6 Preserva LF/CRLF 🧪 (INV-1)
- [ ] AC7 Error claro con no-UTF-8 🧪

### US-1.2 Estado "sin guardar"
- [ ] AC1 `●` en título 👁
- [ ] AC2 Diálogo al cerrar con cambios 👁
- [ ] AC3 Deshacer hasta guardado limpia indicador 🧪

### US-1.3 Editor
- [ ] AC1 Fuente monoespaciada 👁
- [ ] AC2 Resaltado Markdown 👁
- [ ] AC3 Deshacer/rehacer 👁
- [ ] AC4 Tab/Shift+Tab 👁
- [ ] AC5 Barra de estado (línea:col, palabras, EOL, codificación) 🧪 conteo + 👁

### US-1.4 Vista previa
- [ ] AC1 Debounce ≤ 150 ms 👁
- [ ] AC2 Elementos CommonMark + GFM 👁 (fixture `fixtures/gfm_showcase.md`)
- [ ] AC3 Enlaces externos y relativos 👁
- [ ] AC4 Ctrl/Cmd+\ alterna preview 👁

- [ ] **Hito F1**: editar el README del repo sin pérdida de datos 👁

## Fase 2 — Explorador → v0.3.0

### US-2.1 Abrir carpeta
- [ ] AC1 Ctrl/Cmd+Shift+O y argumento CLI 👁
- [ ] AC2 Carga perezosa 🧪
- [ ] AC3 `.gitignore` y ocultos 🧪
- [ ] AC4 Filtro solo Markdown 🧪
- [ ] AC5 Orden dirs→archivos, alfabético 🧪 (INV-4)
- [ ] AC6 10 000 archivos sin bloquear UI 🧪 (ignored) + 👁

### US-2.2 Navegación
- [ ] AC1 Clic simple / doble clic 👁 + 🧪 (INV-7)
- [ ] AC2 Navegación por teclado 👁
- [ ] AC3 Resaltado del activo 👁

### US-2.3 Operaciones de archivo
- [ ] AC1 Menú contextual completo 👁
- [ ] AC2 Eliminar → papelera con confirmación 🧪 + 👁
- [ ] AC3 Renombrar actualiza pestaña 🧪
- [ ] AC4 Validación de nombres 🧪

### US-2.4 Cambios externos
- [ ] AC1 Árbol refleja cambios 🧪
- [ ] AC2 Recarga silenciosa sin cambios locales 🧪
- [ ] AC3 Banner de conflicto 🧪 + 👁
- [ ] AC4 Pestaña "(eliminado)" 🧪 + 👁
- [ ] AC5 Guardado propio no dispara banner 🧪 (INV-3)

- [ ] **Hito F2**: navegar y editar el repo; cambio externo manejado 👁

## Fase 3 — Productividad → v0.4.0

### US-3.1 Pestañas
- [ ] AC1 Ctrl/Cmd+W con diálogo 👁
- [ ] AC2 Ctrl+Tab y Ctrl/Cmd+1..9 👁
- [ ] AC3 `●` y tooltip 👁

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
| NFR-1 Binario < 15 MB | [ ] | [ ] | [ ] | |
| NFR-2 Arranque < 300 ms | [ ] | [ ] | [ ] | |
| NFR-3 RAM < 80 MB | [ ] | [ ] | [ ] | |
| NFR-4 Archivo 1 MB fluido / 5 MB usable | [ ] | [ ] | [ ] | |
| NFR-5 Sin unwrap en core | [ ] | [ ] | [ ] | |
| NFR-6 Sin unsafe | [ ] | [ ] | [ ] | |
| NFR-7 Todo por teclado | [ ] | [ ] | [ ] | |
| NFR-8 Strings centralizados | [ ] | [ ] | [ ] | |
| NFR-9 Cobertura core ≥ 70 % | [ ] | [ ] | [ ] | |

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
