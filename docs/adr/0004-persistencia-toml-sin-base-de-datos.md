# ADR 0004 — Persistencia en TOML sin base de datos

**Estado:** Aceptado

## Contexto
Solo se persisten configuración y sesión: datos pequeños y legibles por humanos.

## Decisión
`config.toml` y `session.toml` en directorios del SO (`directories`), con `serde` + `toml`; validación y valores por defecto ante datos inválidos.

## Alternativas
SQLite/sled (sobredimensionado), JSON (menos cómodo de editar a mano).

## Consecuencias
Cambios de esquema se registran en CHANGELOG (**Changed**) y requieren migración en `mdx-core::config`.
