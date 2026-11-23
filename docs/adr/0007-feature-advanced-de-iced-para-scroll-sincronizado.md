# ADR 0007 — Feature `advanced` de iced para el scroll sincronizado

**Estado:** Aceptado (aprobado por el propietario el 2026-11-23)

## Contexto
US-3.3 exige que la vista previa se posicione en el bloque que corresponde a lo que se ve en el editor. Para ello hacen falta dos datos que iced 0.14 no entrega por la API pública:
1. **Primera línea visible del editor.** `text_editor` no expone su scroll.
2. **Dónde quedó cada bloque renderizado en la vista previa.** `markdown::view` no devuelve posiciones.

## Decisión
- Activar la feature **`advanced`** de `iced` (ya permitida en STACK.md; solo cambia sus features) para poder implementar una `Operation` propia que recorre el árbol de widgets y mide los contenedores de bloque (`app/sync.rs`, `preview_sync.rs`).
- Cada bloque de la vista previa se dibuja con `markdown::item` y un visor mínimo propio (`PreviewViewer`), envuelto en un `container` con `Id`.
- La primera línea visible del editor se obtiene «sondeando»: clic virtual en la esquina superior izquierda, lectura del cursor resultante y restauración del cursor/selección originales (`editor::top_visible_line`). No requiere `advanced`.
- El mapeo línea → bloque usa los bloques de nivel superior de `pulldown-cmark` (`outline::top_level_blocks`), que coinciden uno a uno con los `Item` del visor en todos los documentos del repo (test de regresión). Si no coinciden, se usa una proporción por líneas.

## Alternativas
- **Proporción por líneas de origen sin `advanced`:** se descuadra con tablas, bloques de código e imágenes (AC1 quedaría `[~]`).
- **Widget de editor/preview propio:** coste y riesgo mucho mayores; requeriría un ADR de mayor alcance.

## Consecuencias
- La API `advanced` de iced es menos estable entre versiones menores: una migración de iced (`migrate/iced-<ver>`) debe revisar `Measure`.
- Cada vez que cambia el contenido de la vista previa, su tamaño o la pestaña activa se recorre el árbol de widgets una vez (coste proporcional al número de bloques; medir con `large_1mb.md`).
- La sincronía es unidireccional (editor → vista previa); desplazar la vista previa a mano no mueve el editor.
