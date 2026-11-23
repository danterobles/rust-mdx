# ADR 0003 — pulldown-cmark para preview, comrak para exportación

**Estado:** Aceptado

## Contexto
El widget `markdown` de iced ya usa `pulldown-cmark` con parseo incremental. La exportación HTML requiere GFM completo.

## Decisión
`pulldown-cmark` para preview y outline (mapeo de líneas con `into_offset_iter`); `comrak` (features mínimas) para exportar HTML GFM.

## Alternativas
Un único parser (iced no genera HTML; `pulldown-cmark` no cubre todo GFM en HTML con la misma fidelidad).

## Consecuencias
Dos parsers en el árbol de dependencias; posibles diferencias menores de render entre preview y HTML exportado.
