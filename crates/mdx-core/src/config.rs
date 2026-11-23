//! `config.toml`: ajustes que edita el usuario (solo lectura para Mdx).
//!
//! El archivo del usuario **nunca se sobrescribe**: si no se puede interpretar se usan los
//! valores por defecto y se avisa (INV-6). Lo único que Mdx escribe es la plantilla inicial, y
//! solo si el archivo todavía no existe ([`Config::ensure_file`]).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Versión del esquema que entiende esta versión de Mdx.
pub const SCHEMA_VERSION: u32 = 1;

const FONT_SIZE: (u32, u32) = (8, 48);
const TAB_WIDTH: (u8, u8) = (2, 8);
const AUTOSAVE_MS: (u64, u64) = (500, 10_000);
const ZOOM: (f32, f32) = (0.5, 3.0);

/// Plantilla que se escribe la primera vez que se abre la configuración (todo en sus valores
/// por defecto, con comentarios).
pub const TEMPLATE: &str = r#"# Configuración de Mdx. Los cambios se aplican al guardar este archivo.
# Un valor fuera de rango se acota; un archivo con errores se ignora (se usan los valores por
# defecto) y Mdx muestra un aviso sin modificarlo.
schema_version = 1

[editor]
font_family = "monospace"   # nombre de una fuente instalada; si no existe, monoespaciada del sistema
font_size = 14              # 8..=48
line_wrap = true            # ajustar las líneas largas al ancho del editor
tab_width = 4               # 2..=8 (reservado: aún no se aplica)
insert_spaces = true        # (reservado: aún no se aplica)

[preview]
visible = true              # mostrar la vista previa al iniciar
sync_scroll = true          # sincronizar el scroll del editor con la vista previa

[files]
autosave = false            # guardar solo tras un tiempo sin escribir
autosave_delay_ms = 2000    # 500..=10000
respect_gitignore = true    # el explorador respeta .gitignore
show_hidden = false         # el explorador muestra archivos ocultos
only_markdown = true        # el explorador solo muestra archivos Markdown

[appearance]
theme = "system"            # "system" | "light" | "dark" (se aplica en una versión posterior)
zoom = 1.0                  # 0.5..=3.0 (se aplica en una versión posterior)
"#;

/// Ajustes de `[editor]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub font_family: String,
    pub font_size: u32,
    pub line_wrap: bool,
    pub tab_width: u8,
    pub insert_spaces: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            font_family: "monospace".to_owned(),
            font_size: 14,
            line_wrap: true,
            tab_width: 4,
            insert_spaces: true,
        }
    }
}

/// Ajustes de `[preview]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PreviewConfig {
    pub visible: bool,
    pub sync_scroll: bool,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            visible: true,
            sync_scroll: true,
        }
    }
}

/// Ajustes de `[files]`.
// Refleja tal cual el esquema de `config.toml` (DATA §4.1): son interruptores independientes.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesConfig {
    pub autosave: bool,
    pub autosave_delay_ms: u64,
    pub respect_gitignore: bool,
    pub show_hidden: bool,
    pub only_markdown: bool,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            autosave: false,
            autosave_delay_ms: 2000,
            respect_gitignore: true,
            show_hidden: false,
            only_markdown: true,
        }
    }
}

/// Tema de la interfaz.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSetting {
    /// Sigue al sistema operativo.
    #[default]
    System,
    Light,
    Dark,
}

/// Ajustes de `[appearance]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub theme: ThemeSetting,
    pub zoom: f32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: ThemeSetting::System,
            zoom: 1.0,
        }
    }
}

/// Configuración de Mdx.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema_version: u32,
    pub editor: EditorConfig,
    pub preview: PreviewConfig,
    pub files: FilesConfig,
    pub appearance: AppearanceConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            editor: EditorConfig::default(),
            preview: PreviewConfig::default(),
            files: FilesConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

/// Avisos que la interfaz debe mostrar (o registrar) al cargar la configuración.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigWarning {
    /// El TOML no se pudo interpretar; se usan los valores por defecto.
    Invalid(String),
    /// El archivo es de una versión de Mdx más nueva; se usan los valores por defecto.
    NewerSchema(u32),
    /// Un valor estaba fuera de rango y se acotó.
    Clamped(&'static str),
    /// Una clave desconocida se ignoró (solo se registra, no se muestra).
    UnknownKey(String),
}

/// Resultado de cargar la configuración: siempre hay una `Config` utilizable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadedConfig {
    pub config: Config,
    pub warnings: Vec<ConfigWarning>,
}

/// Claves que conoce esta versión, por sección (`""` = nivel superior).
const KNOWN_KEYS: [(&str, &[&str]); 5] = [
    (
        "",
        &["schema_version", "editor", "preview", "files", "appearance"],
    ),
    (
        "editor",
        &[
            "font_family",
            "font_size",
            "line_wrap",
            "tab_width",
            "insert_spaces",
        ],
    ),
    ("preview", &["visible", "sync_scroll"]),
    (
        "files",
        &[
            "autosave",
            "autosave_delay_ms",
            "respect_gitignore",
            "show_hidden",
            "only_markdown",
        ],
    ),
    ("appearance", &["theme", "zoom"]),
];

fn unknown_keys(text: &str) -> Vec<String> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut unknown = Vec::new();
    for (section, known) in KNOWN_KEYS {
        let scope = if section.is_empty() {
            Some(&table)
        } else {
            table.get(section).and_then(toml::Value::as_table)
        };
        for key in scope.into_iter().flat_map(toml::map::Map::keys) {
            if !known.contains(&key.as_str()) {
                unknown.push(if section.is_empty() {
                    key.clone()
                } else {
                    format!("{section}.{key}")
                });
            }
        }
    }
    unknown
}

fn clamp_in<T: PartialOrd + Copy>(value: &mut T, (min, max): (T, T)) -> bool {
    let clamped = if *value < min {
        min
    } else if *value > max {
        max
    } else {
        return false;
    };
    *value = clamped;
    true
}

impl Config {
    /// Acota los valores fuera de rango y devuelve un aviso por cada uno.
    fn sanitize(&mut self) -> Vec<ConfigWarning> {
        let mut warnings = Vec::new();
        if clamp_in(&mut self.editor.font_size, FONT_SIZE) {
            warnings.push(ConfigWarning::Clamped("editor.font_size"));
        }
        if clamp_in(&mut self.editor.tab_width, TAB_WIDTH) {
            warnings.push(ConfigWarning::Clamped("editor.tab_width"));
        }
        if clamp_in(&mut self.files.autosave_delay_ms, AUTOSAVE_MS) {
            warnings.push(ConfigWarning::Clamped("files.autosave_delay_ms"));
        }
        if !self.appearance.zoom.is_finite() {
            self.appearance.zoom = 1.0;
            warnings.push(ConfigWarning::Clamped("appearance.zoom"));
        } else if clamp_in(&mut self.appearance.zoom, ZOOM) {
            warnings.push(ConfigWarning::Clamped("appearance.zoom"));
        }
        if self.editor.font_family.trim().is_empty() {
            self.editor.font_family = EditorConfig::default().font_family;
        }
        warnings
    }

    /// Interpreta el contenido de un `config.toml`.
    ///
    /// # Errors
    /// [`CoreError::Config`] si el TOML no es válido o es de un esquema más nuevo.
    pub fn parse(text: &str) -> Result<LoadedConfig, CoreError> {
        let mut config: Config =
            toml::from_str(text).map_err(|e| CoreError::Config(e.message().to_owned()))?;
        if config.schema_version > SCHEMA_VERSION {
            return Err(CoreError::Config(format!(
                "esquema {} más nuevo que el soportado ({SCHEMA_VERSION})",
                config.schema_version
            )));
        }
        let mut warnings = config.sanitize();
        warnings.extend(
            unknown_keys(text)
                .into_iter()
                .map(ConfigWarning::UnknownKey),
        );
        Ok(LoadedConfig { config, warnings })
    }

    /// Lee `path`. Un archivo ausente da los valores por defecto sin avisos; uno inválido, los
    /// valores por defecto con un aviso. **Nunca escribe** en `path`.
    pub async fn load(path: &Path) -> LoadedConfig {
        let text = match tokio::fs::read_to_string(path).await {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadedConfig::default(),
            Err(e) => {
                return LoadedConfig {
                    config: Config::default(),
                    warnings: vec![ConfigWarning::Invalid(e.to_string())],
                };
            }
        };
        match Self::parse(&text) {
            Ok(loaded) => loaded,
            Err(err) => {
                let warning = match err {
                    CoreError::Config(m) if m.starts_with("esquema") => {
                        let version =
                            toml::from_str::<Config>(&text).map_or(0, |c| c.schema_version);
                        ConfigWarning::NewerSchema(version)
                    }
                    other => ConfigWarning::Invalid(other.to_string()),
                };
                LoadedConfig {
                    config: Config::default(),
                    warnings: vec![warning],
                }
            }
        }
    }

    /// Crea `path` con la [`TEMPLATE`] si todavía no existe (y su directorio). Si ya existe no
    /// se toca. Devuelve `true` si se creó.
    ///
    /// # Errors
    /// [`CoreError`] si no se puede crear el directorio o escribir el archivo.
    pub async fn ensure_file(path: &Path) -> Result<bool, CoreError> {
        if tokio::fs::try_exists(path)
            .await
            .map_err(|e| CoreError::from_io(&e, path))?
        {
            return Ok(false);
        }
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| CoreError::from_io(&e, dir))?;
        }
        crate::io::write_atomic(path, TEMPLATE.as_bytes().to_vec()).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_match_the_documented_schema() {
        let c = Config::default();
        assert!(!c.files.autosave);
        assert_eq!(c.files.autosave_delay_ms, 2000);
        assert_eq!((c.editor.font_size, c.editor.tab_width), (14, 4));
        assert!(c.editor.line_wrap && c.editor.insert_spaces);
        assert!(c.preview.visible && c.preview.sync_scroll);
        assert!(c.files.respect_gitignore && !c.files.show_hidden && c.files.only_markdown);
        assert_eq!(c.appearance.theme, ThemeSetting::System);
        assert!((c.appearance.zoom - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_template_is_valid_and_equals_the_defaults() {
        let loaded = Config::parse(TEMPLATE).unwrap();
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.warnings, Vec::new());
    }

    #[test]
    fn every_supported_setting_can_be_set() {
        // AC2
        let text = r#"
            [editor]
            font_family = "Fira Code"
            font_size = 18
            line_wrap = false
            [files]
            autosave = true
            autosave_delay_ms = 1500
            respect_gitignore = false
            show_hidden = true
            only_markdown = false
            [preview]
            visible = false
            sync_scroll = false
            [appearance]
            theme = "dark"
            zoom = 1.5
        "#;
        let c = Config::parse(text).unwrap().config;
        assert_eq!(c.editor.font_family, "Fira Code");
        assert_eq!(c.editor.font_size, 18);
        assert!(!c.editor.line_wrap);
        assert!(c.files.autosave && c.files.show_hidden);
        assert!(!c.files.respect_gitignore && !c.files.only_markdown);
        assert_eq!(c.files.autosave_delay_ms, 1500);
        assert!(!c.preview.visible && !c.preview.sync_scroll);
        assert_eq!(c.appearance.theme, ThemeSetting::Dark);
        assert!((c.appearance.zoom - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn partial_files_and_missing_sections_use_defaults() {
        let loaded = Config::parse("[files]\nautosave = true\n").unwrap();
        assert!(loaded.config.files.autosave);
        assert_eq!(loaded.config.files.autosave_delay_ms, 2000);
        assert_eq!(loaded.config.editor, EditorConfig::default());
        assert_eq!(Config::parse("").unwrap().config, Config::default());
    }

    #[test]
    fn out_of_range_values_are_clamped_with_one_warning_each() {
        let text = "[editor]\nfont_size = 400\ntab_width = 1\n[files]\nautosave_delay_ms = 10\n\
                    [appearance]\nzoom = 9.0\n";
        let loaded = Config::parse(text).unwrap();
        let c = &loaded.config;
        assert_eq!((c.editor.font_size, c.editor.tab_width), (48, 2));
        assert_eq!(c.files.autosave_delay_ms, 500);
        assert!((c.appearance.zoom - 3.0).abs() < f32::EPSILON);
        assert_eq!(
            loaded.warnings,
            [
                ConfigWarning::Clamped("editor.font_size"),
                ConfigWarning::Clamped("editor.tab_width"),
                ConfigWarning::Clamped("files.autosave_delay_ms"),
                ConfigWarning::Clamped("appearance.zoom"),
            ]
        );
        let low = Config::parse("[editor]\nfont_size = 2\n").unwrap();
        assert_eq!(low.config.editor.font_size, 8);
        let nan = Config::parse("[appearance]\nzoom = nan\n").unwrap();
        assert!((nan.config.appearance.zoom - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_blank_font_family_falls_back_to_the_default() {
        let c = Config::parse("[editor]\nfont_family = \"  \"\n")
            .unwrap()
            .config;
        assert_eq!(c.editor.font_family, "monospace");
    }

    #[test]
    fn unknown_keys_are_ignored_but_reported_for_logging() {
        let text = "schema_version = 1\nfuturo = 1\n[editor]\nfont_size = 15\ncolor = \"x\"\n\
                    [extra]\na = 1\n";
        let loaded = Config::parse(text).unwrap();
        assert_eq!(loaded.config.editor.font_size, 15);
        let mut unknown: Vec<_> = loaded
            .warnings
            .iter()
            .filter_map(|w| match w {
                ConfigWarning::UnknownKey(k) => Some(k.as_str()),
                _ => None,
            })
            .collect();
        unknown.sort_unstable();
        assert_eq!(unknown, ["editor.color", "extra", "futuro"]);
    }

    #[test]
    fn invalid_toml_wrong_types_unknown_theme_and_newer_schema_are_errors() {
        assert!(Config::parse("esto no es [toml").is_err());
        assert!(Config::parse("schema_version = 99\n").is_err());
        assert!(Config::parse("[files]\nautosave = \"sí\"\n").is_err());
        assert!(Config::parse("[appearance]\ntheme = \"neón\"\n").is_err());
        assert!(Config::parse("[editor]\nfont_size = \"grande\"\n").is_err());
    }

    #[tokio::test]
    async fn invalid_not_overwritten() {
        // INV-6 / AC4: un archivo inválido da valores por defecto, un aviso y no se toca.
        let dir = TempDir::new().unwrap();
        let missing = Config::load(&dir.path().join("config.toml")).await;
        assert_eq!(missing, LoadedConfig::default());
        let path = dir.path().join("malo.toml");
        std::fs::write(&path, "[files\nautosave = true").unwrap();
        let bad = Config::load(&path).await;
        assert_eq!(bad.config, Config::default());
        assert!(matches!(bad.warnings[0], ConfigWarning::Invalid(_)));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[files\nautosave = true"
        );
        let newer = dir.path().join("nuevo.toml");
        std::fs::write(&newer, "schema_version = 7\n[files]\nautosave = true\n").unwrap();
        let loaded = Config::load(&newer).await;
        assert!(!loaded.config.files.autosave);
        assert_eq!(loaded.warnings, [ConfigWarning::NewerSchema(7)]);
    }

    #[tokio::test]
    async fn load_reads_a_valid_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[files]\nautosave = true\nautosave_delay_ms = 1500\n",
        )
        .unwrap();
        let loaded = Config::load(&path).await;
        assert!(loaded.config.files.autosave);
        assert_eq!(loaded.config.files.autosave_delay_ms, 1500);
        assert_eq!(loaded.warnings, Vec::new());
    }

    #[tokio::test]
    async fn ensure_file_creates_the_template_once_and_never_overwrites() {
        // AC3: la primera vez que se abre la configuración se crea la plantilla.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mdx/config.toml");
        assert!(Config::ensure_file(&path).await.unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);
        std::fs::write(&path, "[files]\nautosave = true\n").unwrap();
        assert!(!Config::ensure_file(&path).await.unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[files]\nautosave = true\n"
        );
    }
}
