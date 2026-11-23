# ADR 0008 — Imágenes del portapapeles: `arboard` y codificación PNG con `image`

**Estado:** Aceptado (dependencias previstas en STACK.md; `image` añadida para codificar)

## Contexto
US-4.4 AC2 exige pegar una imagen del portapapeles, guardarla como `assets/<fecha>-<n>.png` junto al documento e insertar `![](assets/…)`. iced 0.14 solo expone **texto** en su portapapeles.

## Decisión
- **Leer la imagen:** `arboard 3.7` con la feature `image-data` (ya listado en STACK.md §2.1), en un hilo aparte (`spawn_blocking`) para no bloquear la interfaz.
- **Codificar PNG:** `image 0.25` con `default-features = false, features = ["png"]` en `mdx-core` (`assets::encode_png`). `image` ya forma parte del árbol de dependencias de iced; no se añaden formatos adicionales.
- **Cuándo se intercepta Cmd/Ctrl+V:** solo con el editor enfocado (`text_editor::key_binding`); otros campos de texto (búsqueda, renombrar) conservan el pegado normal. Si el portapapeles no trae imagen, se pega el texto con el portapapeles de iced (`Task` `clipboard::read`).
- **Nombres:** `AAAAMMDD-N.png` con la fecha en **UTC** (algoritmo `civil_from_days`, sin biblioteca de fechas) y la primera `N` libre; nunca se sobrescribe (`create_new`).

## Alternativas
- Fecha local con `chrono`/`time`: más correcto para el usuario pero añade una dependencia solo para un nombre de archivo.
- Guardar el portapapeles con otro formato (BMP/JPEG): el PRD pide PNG.
- Portapapeles propio por plataforma: más código y `unsafe`/FFI (prohibido).

## Consecuencias
- Una captura grande se codifica en un hilo de fondo; el texto insertado llega cuando el archivo ya existe.
- En Linux `arboard` necesita el backend adecuado (X11/Wayland: feature `wayland-data-control`); queda para el QA por plataforma de la F5.
- Un documento sin guardar no puede recibir imágenes (no hay carpeta «junto al documento»): se pide guardarlo primero.
