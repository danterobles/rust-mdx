# ADR 0005 — Historial de deshacer/rehacer propio (`mdx-core::history`)

**Estado:** Aceptado

## Contexto
`iced 0.14` no ofrece deshacer/rehacer en `text_editor` (ni en `Content`), pero US-1.3 AC3 lo exige. Lo detectamos al iniciar la US-1.3.

## Decisión
Implementar `mdx_core::history::History`, lógica pura sin dependencias nuevas:
- Cada entrada guarda solo el fragmento que cambió (offset, texto quitado, texto insertado, cursor antes/después), calculado por prefijo/sufijo común entre el texto anterior y el nuevo.
- El tecleo y el borrado contiguos dentro de 1 s se fusionan en un solo paso.
- `undo`/`redo` devuelven un `Patch` que la UI aplica seleccionando el rango y pegando/borrando, sin reconstruir el `Content` (conserva el scroll).
- Presupuesto de memoria de 8 MB; si el texto diverge del esperado, el historial se vacía en vez de corromper.
- Atajos propios: Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z y Ctrl+Y (Windows/Linux).

## Alternativas
- Snapshots completos por edición: memoria proporcional al tamaño del archivo × pasos; inviable con 1–5 MB (NFR-4).
- Widget de editor propio: fuera de alcance y requeriría ADR mayor.
- Esperar a una versión de iced con historial: bloquea F1.

## Consecuencias
Coste O(n) por edición para calcular el diff (el mismo orden que materializar el texto para el hash de «sin guardar»); medir en NFR-4. Si iced agrega historial nativo, se puede retirar este módulo.
