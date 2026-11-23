# Mdx — muestra CommonMark + GFM

Fixture para la verificación visual de la vista previa (US-1.4 AC2).

## Énfasis

Texto con *cursiva*, **negrita**, ***ambas***, ~~tachado~~ y `código en línea`.

## Encabezados

### Nivel 3
#### Nivel 4
##### Nivel 5
###### Nivel 6

## Listas

- Elemento uno
- Elemento dos
  - Anidado A
  - Anidado B
- Elemento tres

1. Primero
2. Segundo
3. Tercero

### Lista de tareas

- [x] Tarea completada
- [ ] Tarea pendiente
- [ ] Otra pendiente

## Cita

> Una cita de una línea.
>
> > Cita anidada con **énfasis**.

## Bloque de código

```rust
fn main() {
    println!("Hola, Mdx");
}
```

```toml
[package]
name = "mdx"
```

## Tabla

| Columna A | Columna B | Columna C |
|:----------|:---------:|----------:|
| izquierda | centro    | derecha   |
| 1         | 2         | 3         |

## Enlaces

- Externo: [Rust](https://www.rust-lang.org)
- Relativo a Markdown: [README del fixture](./gfm_showcase.md)
- Ancla (no soportada): [ir arriba](#mdx--muestra-commonmark--gfm)
- Esquema no soportado: [correo](mailto:alguien@example.com)

---

Fin del documento.
