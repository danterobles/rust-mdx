# 0009 — Virtualización de la vista previa

**Estado:** Aceptada (2026-11-23)

## Contexto
NFR-3 (RAM < 80 MB con un archivo de 100 KB) no se cumplía: 176 MB. Una parte importante (≈33 MB) la ocupaban los glifos de `cosmic-text` de la vista previa, que construía todos los bloques en cada `view`.

## Decisión
Con más de 150 bloques (`preview_virtual::MIN_BLOCKS`) solo se construyen los bloques cercanos a lo visible (±800 px); el resto se sustituye por espaciadores con la altura estimada y, una vez dibujados, medida (`Measure::for_range`). Se compensa el desplazamiento cuando cambian las alturas de bloques por encima de la vista. El scroll sincronizado usa las alturas conocidas y mueve la ventana al destino. Lógica pura en `preview_virtual.rs`; integración en `app/virtualize.rs`.

## Alternativas
- Editor propio con viewport (`ropey`): atacaría los ≈61 MB del editor, pero es un widget propio grande; descartado por ahora.
- No hacer nada y redefinir NFR-3.

## Consecuencias
- Medido (macOS arm64, release): 100 KB pasa de 176 MB a **135 MB**; archivo pequeño ≈91 MB. NFR-3 sigue sin cumplir el umbral de 80 MB; se anota el valor medido y queda como límite conocido (el editor `text_editor` domina el resto).
- Las alturas de bloques fuera de pantalla son estimaciones hasta que se dibujan: la barra de desplazamiento puede variar ligeramente.
