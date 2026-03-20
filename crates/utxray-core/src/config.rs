use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub ogmios: OgmiosConfig,
    #[serde(default)]
    pub blockfrost: BlockfrostConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_network")]
    pub default: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            default: default_network(),
        }
    }
}

fn default_network() -> String {
    "preview".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(default = "default_backend_primary")]
    pub primary: String,
    #[serde(default = "default_backend_query")]
    pub query: String,
    #[serde(default = "default_backend_evaluator")]
    pub evaluator: String,
    #[serde(default)]
    pub simulator: Option<String>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            primary: default_backend_primary(),
            query: default_backend_query(),
            evaluator: default_backend_evaluator(),
            simulator: None,
        }
    }
}

fn default_backend_primary() -> String {
    "blockfrost".to_string()
}

fn default_backend_query() -> String {
    "blockfrost".to_string()
}

fn default_backend_evaluator() -> String {
    "ogmios".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OgmiosConfig {
    #[serde(default = "default_ogmios_host")]
    pub host: String,
    #[serde(default = "default_ogmios_port")]
    pub port: u16,
}

impl Default for OgmiosConfig {
    fn default() -> Self {
        Self {
            host: default_ogmios_host(),
            port: default_ogmios_port(),
        }
    }
}

fn default_ogmios_host() -> String {
    "127.0.0.1".to_string()
}

fn default_ogmios_port() -> u16 {
    1337
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlockfrostConfig {
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default = "default_context_path")]
    pub context_path: String,
    #[serde(default = "default_auto_update")]
    pub auto_update_context: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            context_path: default_context_path(),
            auto_update_context: default_auto_update(),
        }
    }
}

fn default_context_path() -> String {
    ".utxray/context.json".to_string()
}

fn default_auto_update() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default = "default_trace_level")]
    pub trace_level: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub include_raw: bool,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            trace_level: default_trace_level(),
            format: default_format(),
            include_raw: false,
        }
    }
}

fn default_trace_level() -> String {
    "verbose".to_string()
}

fn default_format() -> String {
    "json".to_string()
}

/// Load config from .utxray.toml in the given project directory.
/// If the project directory doesn't exist, returns an error.
/// If the config file doesn't exist, returns the default config.
/// If the file exists but is malformed, returns an error.
pub fn load(project_dir: &str) -> Result<Config, ConfigError> {
    let dir = Path::new(project_dir);
    if !dir.exists() {
        return Err(ConfigError::ProjectDirNotFound(project_dir.to_string()));
    }

    let path = dir.join(".utxray.toml");
    if !path.exists() {
        return Ok(Config::default());
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadError(e.to_string()))?;

    toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("project directory not found: {0}")]
    ProjectDirNotFound(String),
    #[error("failed to read config file: {0}")]
    ReadError(String),
    #[error("failed to parse config file: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_load_error_when_dir_not_found() {
        let result = load("/nonexistent/path");
        assert!(result.is_err());
        assert!(matches!(result, Err(ConfigError::ProjectDirNotFound(_))));
    }

    #[test]
    fn test_load_default_when_no_config_file() -> TestResult {
        let tmp = std::env::temp_dir();
        let tmp_str = tmp.to_str().ok_or("invalid temp dir")?;
        let config = load(tmp_str)?;
        assert_eq!(config.network.default, "preview");
        assert_eq!(config.backend.primary, "blockfrost");
        assert_eq!(config.ogmios.port, 1337);
        Ok(())
    }

    #[test]
    fn test_load_valid_config() -> TestResult {
        let dir = std::env::current_dir()?.join("../../tests/fixtures/example-project");
        if dir.join(".utxray.toml").exists() {
            let dir_str = dir.to_str().ok_or("invalid path")?;
            let config = load(dir_str)?;
            assert_eq!(config.network.default, "preview");
        }
        Ok(())
    }

    #[test]
    fn test_parse_malformed_config() -> TestResult {
        let tmp = std::env::temp_dir().join("utxray-test-malformed");
        std::fs::create_dir_all(&tmp)?;
        std::fs::write(tmp.join(".utxray.toml"), "this is not valid { toml")?;
        let tmp_str = tmp.to_str().ok_or("invalid path")?;
        let result = load(tmp_str);
        assert!(result.is_err());
        assert!(matches!(result, Err(ConfigError::ParseError(_))));
        let _ = std::fs::remove_dir_all(&tmp);
        Ok(())
    }
}
