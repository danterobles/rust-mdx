# Changelog

Todos los cambios notables de Mdx se documentan en este archivo.

El formato se basa en [Keep a Changelog 1.1.0](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado Semántico 2.0.0](https://semver.org/lang/es/).

Secciones permitidas por versión, en este orden: **Added**, **Changed**, **Deprecated**, **Removed**, **Fixed**, **Security**.
Cada entrada termina con la referencia a la historia del PRD, p. ej. `[US-1.1]`.

## [Unreleased]

## [0.2.0] - 2026-11-23

### Added
- Vista previa en vivo con CommonMark y GFM (encabezados, listas, tareas, tablas, código resaltado), actualizada tras 150 ms sin escribir [US-1.4]
- Enlaces de la vista previa: `http(s)` se abren en el navegador y los `.md` relativos se abren en el editor [US-1.4]
- Ctrl/Cmd+\ muestra u oculta la vista previa [US-1.4]
- Editor con fuente monoespaciada y resaltado de sintaxis Markdown [US-1.3]
- Deshacer y rehacer (Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl+Y en Windows/Linux) [US-1.3]
- Tab y Shift+Tab para indentar y desindentar [US-1.3]
- Barra de estado con línea:columna, conteo de palabras, final de línea y codificación [US-1.3]
- Indicador `●` en el título cuando hay cambios sin guardar [US-1.2]
- Diálogo Guardar / Descartar / Cancelar al cerrar la ventana, o al usar Nuevo y Abrir, con cambios pendientes [US-1.2]
- Nuevo, Abrir, Guardar y Guardar como (Ctrl/Cmd+N, O, S, Shift+S) con diálogos nativos [US-1.1]
- Guardado atómico que evita archivos truncados y conserva finales de línea (LF/CRLF) y BOM [US-1.1]
- Aviso claro al abrir archivos que no son UTF-8 válido [US-1.1]
- Ventana principal «Mdx» con explorador, editor y vista previa (20/40/40), divisores arrastrables y barra de estado [US-0.2]
- Especificación inicial: PRD, DATA, ARCHITECTURE, STACK, CHECKLIST y CLAUDE.md.

[Unreleased]: https://github.com/OWNER/mdx/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/OWNER/mdx/releases/tag/v0.2.0
