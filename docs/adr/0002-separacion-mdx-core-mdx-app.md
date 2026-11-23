# ADR 0002 — Separación `mdx-core` / `mdx-app`

**Estado:** Aceptado

## Contexto
La lógica de negocio debe poder probarse sin UI y sobrevivir a cambios de framework.

## Decisión
`mdx-core` (lógica pura, jamás importa `iced`) y `mdx-app` (UI). Dependencia únicamente `mdx-app → mdx-core`. `xtask` para herramientas.

## Alternativas
Un solo crate (acopla lógica y UI; tests más lentos y difíciles).

## Consecuencias
Core con `deny(unwrap_used, expect_used)` y `CoreError` tipado; la UI orquesta con `Task`s.
