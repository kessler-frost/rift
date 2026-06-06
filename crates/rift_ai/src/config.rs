use std::path::Path;

use serde::Deserialize;

/// Local AI configuration, loaded from `~/.config/rift/config.toml` (`[ai]` table).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RiftAiConfig {
    pub endpoint: String,
    pub model: String,
    #[serde(default = "default_api_key")]
    pub api_key: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_api_key() -> String {
    "omlx-local".to_string()
}

fn default_timeout_ms() -> u64 {
    3000
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    ai: RiftAiConfig,
}

impl RiftAiConfig {
    /// Parse a `[ai]`-tabled TOML string.
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        Ok(toml::from_str::<ConfigFile>(s)?.ai)
    }

    /// Load from a TOML file at `path`.
    pub fn load_from(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_toml_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Default location: `~/.config/rift/config.toml`.
    pub fn default_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        Path::new(&home).join(".config/rift/config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let toml = r#"
[ai]
endpoint = "http://localhost:8000"
model = "qwen"
api_key = "k"
timeout_ms = 1500
"#;
        let cfg = RiftAiConfig::from_toml_str(toml).unwrap();
        assert_eq!(cfg.endpoint, "http://localhost:8000");
        assert_eq!(cfg.model, "qwen");
        assert_eq!(cfg.api_key, "k");
        assert_eq!(cfg.timeout_ms, 1500);
    }

    #[test]
    fn applies_defaults_for_optional_fields() {
        let toml = r#"
[ai]
endpoint = "http://localhost:8000"
model = "qwen"
"#;
        let cfg = RiftAiConfig::from_toml_str(toml).unwrap();
        assert_eq!(cfg.api_key, "omlx-local");
        assert_eq!(cfg.timeout_ms, 3000);
    }

    #[test]
    fn missing_required_field_is_error() {
        let toml = r#"
[ai]
model = "qwen"
"#;
        assert!(RiftAiConfig::from_toml_str(toml).is_err());
    }
}
