# CLAUDE.md — Mdx

Instrucciones para agentes de Claude Code que implementan este proyecto.

---

## 0. Documentos del proyecto (léelos en este orden al iniciar sesión)

| Archivo | Qué contiene | Autoridad |
|---|---|---|
| `docs/PRD.md` | Qué construir: historias, criterios de aceptación (AC), NFR, alcance | **Máxima** — fuente de verdad del *qué* |
| `docs/ARCHITECTURE.md` | Cómo está construido: capas, módulos, flujos, concurrencia, errores | Fuente de verdad del *cómo* |
| `docs/DATA.md` | Tipos, máquinas de estado, archivos persistidos, invariantes (INV-x) | Fuente de verdad de los *datos* |
| `docs/STACK.md` | Toolchain, dependencias permitidas, perfiles, CI, licencias | Fuente de verdad de las *herramientas* |
| `docs/CHECKLIST.md` | Estado de cada AC, NFR y QA | Estado del avance |
| `docs/progress.md` | Bitácora: planes, decisiones, observaciones por historia | Contexto histórico |
| `docs/adr/` | Decisiones de arquitectura | Justificación de cambios |
| `CHANGELOG.md` | Cambios visibles por versión | Registro público |

**Jerarquía ante conflictos:** PRD > DATA > ARCHITECTURE > STACK > CLAUDE.md. Si encuentras una contradicción, **detente y repórtala**; no la resuelvas en silencio.

Al empezar cada sesión: lee `docs/CHECKLIST.md` para ubicar la siguiente historia pendiente y la última entrada de `docs/progress.md`.

## 1. Qué es este proyecto

Mdx: editor de Markdown de escritorio, ligero y multiplataforma (Windows, macOS, Linux), 100 % Rust, con explorador de archivos, editor y vista previa en vivo. UI con **iced 0.14**. Workspace con `mdx-core` (lógica pura) y `mdx-app` (UI).

## 2. Comandos

```bash
cargo build --workspace                                   # compilar
cargo run -p mdx-app                                      # ejecutar
cargo run -p mdx-app -- ./docs                            # abrir carpeta
cargo test --workspace                                    # tests
cargo test --workspace -- --ignored                       # tests de rendimiento
cargo clippy --workspace --all-targets -- -D warnings     # lint (obligatorio)
cargo fmt --all                                           # formato (obligatorio)
cargo deny check                                          # licencias/advisories
cargo insta review                                        # snapshots
cargo xtask gen-fixtures                                  # genera fixtures grandes
cargo xtask size                                          # verifica NFR-1
cargo build --release -p mdx-app                          # release
```

**Gate de calidad** (antes de cualquier commit, en este orden; las tres deben pasar):
`cargo fmt --all` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`

## 3. Flujo Spec Driven (OBLIGATORIO)

Una historia (`US-x.y`) a la vez, en orden de fase. No empieces una fase sin cerrar la anterior.

1. **Ubicar**: siguiente ítem `[ ]` en `docs/CHECKLIST.md`.
2. **Leer**: la historia y sus AC en `PRD.md`; los tipos en `DATA.md`; el módulo responsable en `ARCHITECTURE.md`.
3. **Rama**: `git switch -c feat/us-x.y-descripcion-corta` desde `main` actualizada.
4. **Plan**: en `docs/progress.md`, archivos a tocar y test por AC. Si un AC es ambiguo, elige la interpretación más simple que lo cumpla y anótala.
5. **Tests primero** para todo AC de lógica e invariantes (`INV-x` de DATA.md). AC visuales → verificación manual 👁 descrita en progress.md.
6. **Implementar** lo mínimo para cumplir los AC. Nada fuera del PRD.
7. **Gate de calidad** (§2).
8. **Actualizar documentación** (ver §6): CHECKLIST, CHANGELOG, progress, y DATA/ARCHITECTURE/STACK si cambió lo que describen.
9. **Commit(s)** según §4.
10. **Merge** a `main` con `git merge --no-ff` cuando la historia esté completa y el gate pase. No hagas `push` salvo que el usuario lo pida.

Al completar la última historia de una fase → **cierre de fase** (§5).

## 4. Control de versiones (Git)

### 4.1 Ramas
| Rama | Uso |
|---|---|
| `main` | Siempre compila y pasa el gate. Solo recibe merges. |
| `feat/us-x.y-<slug>` | Una historia de usuario |
| `fix/<slug>` | Corrección de bug fuera de una historia en curso |
| `chore/<slug>` | Dependencias, CI, tooling, docs sin cambio funcional |
| `migrate/iced-<ver>` | Migración de versión de iced (historia propia) |

Nunca trabajes directo en `main`. Nunca reescribas historia publicada (`push --force`, `rebase` de ramas ya empujadas).

### 4.2 Commits — Conventional Commits
Formato: `tipo(ámbito): descripción en imperativo, minúsculas [US-x.y]`

| Tipo | Cuándo | Efecto en versión |
|---|---|---|
| `feat` | Funcionalidad nueva visible | minor (tras 1.0) |
| `fix` | Corrección de bug | patch |
| `perf` | Mejora de rendimiento | patch |
| `refactor` | Sin cambio de comportamiento | — |
| `test` | Solo tests | — |
| `docs` | Solo documentación | — |
| `chore` | Tooling, deps, CI | — |
| `build` / `ci` | Build o pipelines | — |

Ámbitos: `core`, `io`, `workspace`, `watcher`, `outline`, `search`, `export`, `config`, `session`, `app`, `editor`, `preview`, `explorer`, `tabs`, `ui`, `ci`, `deps`, `docs`, `release`.

Cambio incompatible (formato de config/sesión, atajo eliminado): `feat(config)!: ...` y pie `BREAKING CHANGE: <explicación>`.

Reglas:
- Commits atómicos: un propósito por commit; cada commit pasa el gate.
- Cuerpo opcional explicando el *por qué*, no el *qué*.
- Commits generados por el agente terminan con el trailer de coautoría que indique la sesión (si aplica).
- No incluyas archivos generados (`target/`, `fixtures/large_*.md`, snapshots `.snap.new`).

### 4.3 Versionado — SemVer 2.0
- **Única fuente de versión**: `[workspace.package] version` en el `Cargo.toml` raíz. Los crates heredan con `version.workspace = true`.
- Antes de 1.0, cada **fase cerrada** sube la versión menor:

| Fase cerrada | Versión |
|---|---|
| F0 | `0.1.0` |
| F1 | `0.2.0` |
| F2 | `0.3.0` |
| F3 | `0.4.0` |
| F4 | `0.5.0` |
| F5 | `1.0.0` |

- Correcciones entre fases: patch (`0.2.1`).
- Tras 1.0: `fix`/`perf` → patch; `feat` → minor; `!`/`BREAKING CHANGE` → major.
- **Tags** `vX.Y.Z` (anotados: `git tag -a vX.Y.Z -m "Mdx vX.Y.Z"`) **solo con autorización explícita del usuario**, porque disparan el workflow de release.

## 5. Cierre de fase

1. Verificar que todas las historias de la fase estén `[x]` (o `[~]`/`[-]` justificados) en `CHECKLIST.md`.
2. Verificar el **Hito** de la fase y los NFR que correspondan (F1, F3, F5) registrando valores medidos.
3. En `CHANGELOG.md`: renombrar `## [Unreleased]` a `## [X.Y.Z] - AAAA-MM-DD`, crear un `## [Unreleased]` vacío arriba y actualizar los enlaces de comparación al pie.
4. Subir `version` en `[workspace.package]`; correr `cargo build` para actualizar `Cargo.lock`.
5. Commit: `chore(release): vX.Y.Z`.
6. Merge a `main`. Preguntar al usuario si desea crear el tag.
7. Resumen al usuario: historias completadas, AC con observación, NFR medidos, siguiente fase.

## 6. Reglas de documentación

### 6.1 CHANGELOG.md (Keep a Changelog)
- Toda historia o fix **visible para el usuario** agrega una línea en `## [Unreleased]` en el mismo commit (o el último commit de la rama).
- Secciones en orden: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`. Omite las vacías.
- Redacta para el usuario final, en español, en pasado o sustantivo: "Guardado atómico que evita archivos truncados [US-1.1]".
- No registres refactors internos, tests ni cambios de CI (salvo que afecten al usuario).
- Cambios de esquema de `config.toml`/`session.toml` y atajos de teclado siempre van en **Changed** (o **Removed**).
- Actualizaciones de dependencias por seguridad van en **Security**.

### 6.2 CHECKLIST.md
- Marca cada AC en el mismo commit que lo cumple: `[x]` hecho, `[~]` con observación (explica en progress.md), `[-]` fuera de alcance (con motivo).
- Nunca marques `[x]` un AC sin test 🧪 pasando o verificación 👁 registrada.
- Nunca borres ítems. Si el PRD agrega AC, agrégalos aquí.
- Usa las checklists A (por commit) y B (cierre de fase) del propio archivo.

### 6.3 DATA.md / ARCHITECTURE.md / STACK.md
- Si cambias un tipo, estado, archivo persistido o invariante → actualiza `DATA.md` en el mismo commit.
- Si agregas un módulo, cambias un flujo o una regla de concurrencia → `ARCHITECTURE.md`.
- Si agregas/cambias una dependencia o herramienta → `STACK.md` + ADR. Reemplaza los 🔎 por la versión exacta cuando la fijes.

### 6.4 progress.md
```markdown
## US-1.1 Nuevo / Abrir / Guardar — ✅ completa (rama feat/us-1.1-open-save)
Plan: core/io.rs (write_atomic, detect_line_ending), app.rs (handle_file)
- AC5 🧪 core::io::tests::atomic_failure_keeps_original
- AC7 ~ solo detecta UTF-8 inválido; reapertura con otra codificación fuera de alcance v1
Decisiones: ...
```

### 6.5 ADR
`docs/adr/NNNN-titulo.md` con: Contexto, Decisión, Alternativas, Consecuencias, Estado. Obligatorio para: nueva dependencia, cambio de stack, widget propio, cambio de arquitectura, excepción a una regla de este archivo.

## 7. Reglas de arquitectura (resumen; detalle en ARCHITECTURE.md)

- `mdx-core` **nunca** importa `iced`.
- Arquitectura Elm en `mdx-app`; `update` rápido, `view` pura, efectos solo vía `Task` y `Subscription`.
- Cero I/O bloqueante en `update`/`view`.
- Guardado siempre con `mdx_core::io::write_atomic`.
- Eliminar archivos del usuario = papelera (`trash`). Prohibido `remove_file`/`remove_dir_all` sobre archivos del usuario.
- Preservar finales de línea y BOM.
- Textos de UI en `crates/mdx-app/src/strings.rs` (español).

## 8. Convenciones de código

- Edition 2024; `unsafe_code = "forbid"` en el workspace.
- `mdx-core`: `#![deny(clippy::unwrap_used, clippy::expect_used)]`; errores con `CoreError` (`thiserror`); sin `panic!` por input externo.
- `mdx-app`: `anyhow` solo en `main`; los errores de operación se muestran al usuario.
- Logs con `tracing`; nada de `println!`.
- Identificadores en inglés; comentarios/docs pueden ir en español; strings de UI en español.
- `///` en toda función pública de `mdx-core` (contrato + errores).
- Tests que tocan disco: siempre en `tempfile::TempDir`. Snapshots con `insta`.

## 9. Notas sobre iced 0.14

- API pre-1.0 que cambia entre versiones. **No uses memoria de 0.12/0.13.** Verifica firmas en docs.rs (`iced 0.14.x`) o con Context7, y en `examples/` del repo de iced en el tag `0.14.0`.
- Widgets clave: `text_editor`, `markdown` (parseo incremental), `pane_grid`, `scrollable`, `mouse_area`, `tooltip`.
- No forkees ni parchees iced; widgets propios en `mdx-app/src/widgets/` con ADR.

## 10. Prohibido

- Implementar No-objetivos del PRD (WYSIWYG, plugins, LSP, multicursor, nube, PDF).
- Cambiar stack sin ADR aprobado por el propietario.
- Desactivar lints, tests o checks de CI para hacer pasar algo.
- Commits en `main`, `push --force`, `git push`, tags o releases sin autorización del usuario.
- Gestionar certificados, secrets o credenciales (US-5.2: solo documentar y parametrizar).
- Borrar/sobrescribir archivos fuera de tempdirs en tests.
- Marcar AC como hechos sin evidencia.

## 11. Cuándo detenerte y preguntar

- Contradicción entre documentos o entre AC.
- Se necesita una dependencia fuera de `STACK.md` o romper la separación core/UI.
- Un NFR no se puede cumplir sin cambiar arquitectura.
- Antes de crear un tag o cualquier operación remota de Git.
- Cualquier operación destructiva fuera del flujo especificado.

## 12. Definición de Terminado (por historia)

- [ ] Todos los AC `[x]` (o `[~]`/`[-]` justificados) en `docs/CHECKLIST.md`
- [ ] Tests 🧪 para cada AC de lógica e invariante aplicable
- [ ] Gate de calidad en verde
- [ ] Sin `unwrap`/`expect` nuevos en `mdx-core`
- [ ] `CHANGELOG.md` → `[Unreleased]` actualizado (si es visible al usuario)
- [ ] `DATA.md` / `ARCHITECTURE.md` / `STACK.md` al día
- [ ] `docs/progress.md` con plan, evidencia y decisiones
- [ ] Commits convencionales con `[US-x.y]`, rama mergeada a `main` con `--no-ff`

## 13. Punto de partida (repo vacío → US-0.1)

1. `git init -b main`; commit inicial con los documentos de `docs/`, `CLAUDE.md` y `CHANGELOG.md`: `docs: especificación inicial`.
2. Rama `feat/us-0.1-workspace-ci`.
3. Workspace según `STACK.md` §5: `crates/mdx-core` (lib), `crates/mdx-app` (bin `mdx`), `xtask`. Versión `0.0.0`.
4. Verificar en crates.io las versiones 🔎 de `STACK.md`, fijarlas en `[workspace.dependencies]` y reemplazar los 🔎 en `STACK.md`.
5. `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml`, `.gitignore` (incluye `target/`, `fixtures/large_*`, `*.snap.new`).
6. `.github/workflows/ci.yml` según `STACK.md` §6.
7. `docs/progress.md` y ADR 0001–0004 listados en `ARCHITECTURE.md` §12.
8. Gate de calidad, marcar AC en `CHECKLIST.md`, entrada en `CHANGELOG.md`, commit `chore(ci): workspace, lints y pipeline multiplataforma [US-0.1]`.
