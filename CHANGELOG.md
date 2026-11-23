# Changelog

Todos los cambios notables de Mdx se documentan en este archivo.

El formato se basa en [Keep a Changelog 1.1.0](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado Semántico 2.0.0](https://semver.org/lang/es/).

Secciones permitidas por versión, en este orden: **Added**, **Changed**, **Deprecated**, **Removed**, **Fixed**, **Security**.
Cada entrada termina con la referencia a la historia del PRD, p. ej. `[US-1.1]`.

## [Unreleased]

### Added
- Panel «Esquema» (conmutador junto a «Archivos»): lista de encabezados H1–H6 del documento activo, indentados por nivel; un clic lleva el cursor a esa línea [US-3.2]
- Pestañas: varios documentos abiertos a la vez, con `●` si tienen cambios, cursiva en la pestaña de vista previa y la ruta completa al pasar el ratón [US-3.1]
- Ctrl/Cmd+W cierra la pestaña (con aviso si tiene cambios); Ctrl+Tab y Ctrl+Shift+Tab cambian de pestaña; Ctrl/Cmd+1…9 saltan a una pestaña [US-3.1]

### Changed
- Abrir un archivo, crear uno nuevo o abrir un enlace ya no reemplaza el documento actual: se abre en otra pestaña. El aviso de cambios sin guardar aparece al cerrar la pestaña o la aplicación (que revisa todas las pestañas) [US-3.1]
- Editar una pestaña de vista previa la fija automáticamente [US-3.1]

## [0.3.0] - 2026-11-23

### Added
- Cambios externos: el explorador refleja altas, bajas y renombrados; el archivo abierto se recarga solo si no tiene cambios, muestra el aviso «El archivo cambió en disco» con Recargar / Conservar mío si los tiene, y se marca «(eliminado)» conservando el contenido si se borra [US-2.4]
- Operaciones de archivo desde el explorador (clic derecho): nuevo archivo, nueva carpeta, renombrar, eliminar (a la papelera, con confirmación) y revelar en el sistema; los nombres inválidos o repetidos muestran el error en línea [US-2.3]
- Navegación del explorador: clic abre el archivo (pestaña preview reutilizable), doble clic lo fija, flechas y Enter para moverse, expandir/colapsar y abrir, y resalte del archivo abierto [US-2.2]
- Explorador de archivos: abrir carpeta con Ctrl/Cmd+Shift+O o `mdx <ruta>`, árbol con carga perezosa, orden carpetas primero, y filtros de archivos ocultos, `.gitignore` y solo Markdown (clic derecho en el panel) [US-2.1]
- Abrir un archivo desde la línea de comandos con `mdx <ruta>` [US-2.1]

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

[Unreleased]: https://github.com/OWNER/mdx/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/OWNER/mdx/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/OWNER/mdx/releases/tag/v0.2.0
