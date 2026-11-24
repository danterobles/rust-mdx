# Publicación y firma de RustMDX

## 1. Cómo se publica una versión

1. Cierre de fase según `CLAUDE.md` §5: `CHANGELOG.md` con la sección `## [X.Y.Z]`, y la versión subida en **dos** lugares: `[workspace.package]` de `Cargo.toml` y `version` de `Packager.toml`.
2. Merge a `main` y, **solo con autorización del propietario**, tag anotado y push:
   `git tag -a vX.Y.Z -m "Mdx vX.Y.Z" && git push origin vX.Y.Z`.
3. El workflow `.github/workflows/release.yml`:
   - `verify`: el tag coincide con `Cargo.toml`, `Packager.toml` y existe la sección en `CHANGELOG.md`.
   - `package` (ubuntu / macOS / windows): `cargo packager --release --formats …`.
     Linux → `.AppImage` + `.deb`; macOS → `.dmg` universal (x86_64 + arm64, unidos con `lipo`); Windows → `.msi` (WiX).
   - `release`: crea el GitHub Release con los artefactos; el cuerpo es la sección del CHANGELOG de esa versión.

Prueba local: `cargo install cargo-packager --locked && cargo packager --release --formats dmg` (o `app`, `deb`, `appimage`, `wix` según el sistema). La salida queda en `target/packager/`.

## 2. Firma (US-5.2)

> El agente **no** gestiona certificados, claves ni contraseñas. Aquí solo se documenta qué hacer y qué *secrets* de GitHub configura el propietario. Sin ellos el workflow publica instaladores **sin firmar** (Gatekeeper y SmartScreen mostrarán avisos).

### 2.1 macOS (Apple Developer ID + notarización)

Requisitos: cuenta de Apple Developer, certificado «Developer ID Application», contraseña específica de app para notarizar.

| Secret de GitHub | Contenido |
|---|---|
| `APPLE_CERTIFICATE` | Certificado `.p12` en base64 |
| `APPLE_CERTIFICATE_PASSWORD` | Contraseña del `.p12` |
| `APPLE_ID` | Apple ID usado para notarizar |
| `APPLE_PASSWORD` | Contraseña específica de app |
| `APPLE_TEAM_ID` | Team ID |

Pasos que el propietario completa: (1) exportar el certificado y codificarlo (`base64 -i cert.p12`); (2) crear los secrets; (3) en `Packager.toml` añadir `signing-identity = "Developer ID Application: <Nombre> (<TEAM_ID>)"` en la sección `[macos]`; (4) importar el certificado en un llavero temporal en el job de macOS antes de «Empaquetar» y dejar que `cargo-packager` firme y notarice (consultar la documentación vigente de `cargo-packager` para las variables exactas de notarización). El job ya recibe los secrets como variables de entorno.

### 2.2 Windows

Requisitos: certificado de firma de código (OV/EV) o servicio de firma en la nube.

| Secret de GitHub | Contenido |
|---|---|
| `WINDOWS_CERTIFICATE` | `.pfx` en base64 |
| `WINDOWS_CERTIFICATE_PASSWORD` | Contraseña del `.pfx` |

Pasos: importar el `.pfx` en el almacén del runner y fijar en `Packager.toml`, sección `[windows]`, `certificate-thumbprint`, `digest-algorithm = "sha256"` y `timestamp-url`. Sin certificado, el `.msi` se publica sin firmar.

### 2.3 Linux

No requiere firma. Opcional: firmar el `.AppImage`/`.deb` con GPG (fuera del alcance de v1).

## 3. Limitaciones conocidas

- Asociación de archivos: en Windows y Linux el sistema pasa la ruta como argumento (`mdx <ruta>`, ya soportado). En macOS «Abrir con» entrega el archivo por evento de la aplicación, que `winit` 0.30 no expone: RustMDX se abre pero no carga el archivo (se documenta como observación de US-5.1 AC3).
- Los pasos de empaquetado de Windows y Linux solo se ejercitan en el workflow de release; localmente se verificó el `.app`/`.dmg` de macOS.
