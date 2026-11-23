# ADR 0006 — Documentos abiertos: uno activo y el resto aparcados

**Estado:** Aceptado

## Contexto
Hasta la US-2.4 la app mantenía un único documento en memoria. US-3.1 exige varias pestañas con estado independiente (texto, historial de deshacer, vista previa, conflicto con el disco). ARCHITECTURE preveía un `HashMap<DocumentId, text_editor::Content>`, pero eso obligaba a reescribir todo el código y los tests que asumen «el documento».

## Decisión
- El **documento activo** conserva su estado en los campos de `App` (`doc`, `content`, `history`, `dirty`, `preview`, …), de modo que todo el código existente sigue operando sobre «el documento actual».
- Los demás documentos viven **aparcados** en `App::parked: HashMap<DocumentId, OpenDoc>`, con exactamente el mismo estado agrupado en un `OpenDoc`.
- Cambiar de pestaña **intercambia** ambos (`swap_active`: mueve, no copia). El orden y el estado fijada/preview lo lleva el modelo puro `state::tabs::Tabs` (INV-7).
- Los eventos asíncronos dirigidos a un documento concreto (guardado, lectura de disco, parseo de la vista previa) llevan su `DocumentId` y se aplican con `App::with_doc`, que activa el documento un instante y restaura el anterior.
- Editar fija la pestaña «preview» (nunca se reemplaza una pestaña con cambios); abrir un archivo ya abierto activa su pestaña; un «Sin título» vacío se reemplaza en vez de acumularse.
- `app.rs` se dividió en `app/{mod,documents,editing,disk,explorer_ops,preview,tests}.rs`.

## Alternativas
- `Vec<OpenDoc>` + índice activo y acceso `self.current()` en todas partes: más «limpio» pero reescribe ~180 referencias y todos los tests.
- `HashMap<DocumentId, Content>` separado del resto del estado: duplica la gestión de historial, dirty y vista previa por documento.

## Consecuencias
- Cambiar de pestaña es O(1) (movimiento de punteros), sin reparsear.
- Hay que recordar que `with_doc` no debe cerrar ni cambiar la pestaña activa dentro de su closure; las acciones que sí lo hacen (cerrar pestaña tras guardar) se ejecutan **después**, fuera de `with_doc`.
- Si en el futuro el número de campos por documento crece, conviene migrar a la alternativa 1.
