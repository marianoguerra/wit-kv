//! Server configuration parsing.

use serde::Deserialize;
use std::path::Path;

/// Server configuration loaded from TOML file.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Server settings.
    pub server: ServerConfig,
    /// Database configurations.
    pub databases: Vec<DatabaseConfig>,
}

/// Server bind settings.
#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// Bind address (e.g., "127.0.0.1" or "0.0.0.0").
    pub bind: String,
    /// Port to listen on.
    pub port: u16,
}

/// Database configuration.
#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    /// Database name (used in API paths).
    pub name: String,
    /// Path to the database directory.
    pub path: String,
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::Io(path.as_ref().display().to_string(), e))?;
        Self::from_str(&content)
    }

    /// Parse configuration from a TOML string.
    pub fn from_str(content: &str) -> Result<Self, ConfigError> {
        toml::from_str(content).map_err(ConfigError::Parse)
    }

    /// Get the socket address string for binding.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.server.bind, self.server.port)
    }
}

/// Configuration error.
#[derive(Debug)]
pub enum ConfigError {
    /// IO error reading config file.
    Io(String, std::io::Error),
    /// TOML parse error.
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(path, e) => write!(f, "Failed to read config file '{}': {}", path, e),
            ConfigError::Parse(e) => write!(f, "Failed to parse config: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml = r#"
[server]
bind = "127.0.0.1"
port = 8080

[[databases]]
name = "default"
path = ".wit-kv"

[[databases]]
name = "archive"
path = "/var/lib/wit-kv/archive"
"#;
        let config = Config::from_str(toml).unwrap();
        assert_eq!(config.server.bind, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.databases.len(), 2);
        assert_eq!(config.databases.first().unwrap().name, "default");
        assert_eq!(config.databases.get(1).unwrap().name, "archive");
    }
}
