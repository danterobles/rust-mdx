//! `config.toml`: ajustes que edita el usuario (solo lectura para Mdx).
//!
//! En US-3.5 solo se interpreta `[files]` (autoguardado); el resto de secciones se ignora y la
//! US-4.2 las añade. El archivo del usuario **nunca se sobrescribe**.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Versión del esquema que entiende esta versión de Mdx.
pub const SCHEMA_VERSION: u32 = 1;
const MIN_AUTOSAVE_MS: u64 = 500;
const MAX_AUTOSAVE_MS: u64 = 10_000;

/// Ajustes de `[files]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesConfig {
    pub autosave: bool,
    pub autosave_delay_ms: u64,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            autosave: false,
            autosave_delay_ms: 2000,
        }
    }
}

/// Configuración de Mdx.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema_version: u32,
    pub files: FilesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            files: FilesConfig::default(),
        }
    }
}

/// Avisos que la interfaz debe mostrar al cargar la configuración.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigWarning {
    /// El TOML no se pudo interpretar; se usan los valores por defecto.
    Invalid(String),
    /// El archivo es de una versión de Mdx más nueva; se usan los valores por defecto.
    NewerSchema(u32),
    /// Un valor estaba fuera de rango y se acotó.
    Clamped(&'static str),
}

/// Resultado de cargar la configuración: siempre hay una `Config` utilizable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: Config,
    pub warnings: Vec<ConfigWarning>,
}

impl Config {
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
        let mut warnings = Vec::new();
        let delay = config
            .files
            .autosave_delay_ms
            .clamp(MIN_AUTOSAVE_MS, MAX_AUTOSAVE_MS);
        if delay != config.files.autosave_delay_ms {
            warnings.push(ConfigWarning::Clamped("files.autosave_delay_ms"));
            config.files.autosave_delay_ms = delay;
        }
        Ok(LoadedConfig { config, warnings })
    }

    /// Lee `path`. Un archivo ausente da los valores por defecto sin avisos; uno inválido, los
    /// valores por defecto con un aviso. Nunca escribe en `path`.
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_have_autosave_off_and_two_seconds() {
        let c = Config::default();
        assert!(!c.files.autosave);
        assert_eq!(c.files.autosave_delay_ms, 2000);
    }

    #[test]
    fn partial_files_and_missing_sections_use_defaults() {
        let loaded = Config::parse("[files]\nautosave = true\n").unwrap();
        assert!(loaded.config.files.autosave);
        assert_eq!(loaded.config.files.autosave_delay_ms, 2000);
        assert_eq!(Config::parse("").unwrap().config, Config::default());
    }

    #[test]
    fn unknown_sections_and_fields_are_ignored() {
        let text =
            "schema_version = 1\n[editor]\nfont_size = 18\n[files]\nautosave = true\nfuturo = 1\n";
        assert!(Config::parse(text).unwrap().config.files.autosave);
    }

    #[test]
    fn out_of_range_delay_is_clamped_with_a_warning() {
        let low = Config::parse("[files]\nautosave_delay_ms = 10\n").unwrap();
        assert_eq!(low.config.files.autosave_delay_ms, 500);
        assert_eq!(
            low.warnings,
            [ConfigWarning::Clamped("files.autosave_delay_ms")]
        );
        let high = Config::parse("[files]\nautosave_delay_ms = 999999\n").unwrap();
        assert_eq!(high.config.files.autosave_delay_ms, 10_000);
    }

    #[test]
    fn invalid_toml_and_newer_schema_are_errors_to_parse() {
        assert!(Config::parse("esto no es [toml").is_err());
        assert!(Config::parse("schema_version = 99\n").is_err());
        assert!(Config::parse("[files]\nautosave = \"sí\"\n").is_err());
    }

    #[tokio::test]
    async fn load_never_fails_and_never_writes() {
        let dir = TempDir::new().unwrap();
        // Ausente: defaults sin avisos.
        let missing = Config::load(&dir.path().join("config.toml")).await;
        assert_eq!(missing, LoadedConfig::default());
        // Inválido: defaults + aviso, y el archivo del usuario queda intacto (INV-6).
        let path = dir.path().join("malo.toml");
        std::fs::write(&path, "[files\nautosave = true").unwrap();
        let bad = Config::load(&path).await;
        assert_eq!(bad.config, Config::default());
        assert!(matches!(bad.warnings[0], ConfigWarning::Invalid(_)));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[files\nautosave = true"
        );
        // Esquema nuevo: defaults + aviso con la versión.
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
        assert!(loaded.warnings.is_empty());
    }
}
