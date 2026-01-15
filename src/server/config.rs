//! Server configuration parsing.

use serde::Deserialize;
use std::path::Path;

/// Server configuration loaded from TOML file.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Server settings.
    pub server: ServerConfig,
    /// CORS settings (optional, deny all by default).
    #[serde(default)]
    pub cors: CorsConfig,
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
    /// Optional path to serve static files from.
    /// Files are served from the root path after API endpoints.
    pub static_path: Option<String>,
}

/// CORS (Cross-Origin Resource Sharing) configuration.
/// By default, all cross-origin requests are denied.
#[derive(Debug, Deserialize)]
pub struct CorsConfig {
    /// Whether CORS is enabled. If false, all cross-origin requests are denied.
    #[serde(default)]
    pub enabled: bool,
    /// Allowed origins. Use ["*"] to allow all origins (not recommended for production).
    /// Example: ["http://localhost:3000", "https://myapp.com"]
    #[serde(default)]
    pub allow_origins: Vec<String>,
    /// Allowed HTTP methods. Default: ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
    #[serde(default = "default_methods")]
    pub allow_methods: Vec<String>,
    /// Allowed headers. Default: ["Content-Type", "Accept", "Authorization"]
    #[serde(default = "default_headers")]
    pub allow_headers: Vec<String>,
    /// Whether to allow credentials (cookies, authorization headers).
    #[serde(default)]
    pub allow_credentials: bool,
    /// Max age for preflight request caching in seconds.
    #[serde(default = "default_max_age")]
    pub max_age: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_origins: Vec::new(),
            allow_methods: default_methods(),
            allow_headers: default_headers(),
            allow_credentials: false,
            max_age: default_max_age(),
        }
    }
}

fn default_methods() -> Vec<String> {
    vec![
        "GET".to_string(),
        "POST".to_string(),
        "PUT".to_string(),
        "DELETE".to_string(),
        "OPTIONS".to_string(),
    ]
}

fn default_headers() -> Vec<String> {
    vec![
        "Content-Type".to_string(),
        "Accept".to_string(),
        "Authorization".to_string(),
    ]
}

fn default_max_age() -> u64 {
    3600 // 1 hour
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
        // Defaults
        assert!(!config.cors.enabled);
        assert!(config.server.static_path.is_none());
    }

    #[test]
    fn test_parse_config_with_cors_and_static() {
        let toml = r#"
[server]
bind = "0.0.0.0"
port = 3000
static_path = "./public"

[cors]
enabled = true
allow_origins = ["http://localhost:3000", "https://myapp.com"]
allow_methods = ["GET", "POST"]
allow_headers = ["Content-Type", "X-Custom-Header"]
allow_credentials = true
max_age = 7200

[[databases]]
name = "main"
path = "./data"
"#;
        let config = Config::from_str(toml).unwrap();
        assert_eq!(config.server.bind, "0.0.0.0");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.server.static_path, Some("./public".to_string()));

        assert!(config.cors.enabled);
        assert_eq!(config.cors.allow_origins.len(), 2);
        assert_eq!(config.cors.allow_origins.first().unwrap(), "http://localhost:3000");
        assert_eq!(config.cors.allow_methods.len(), 2);
        assert_eq!(config.cors.allow_headers.len(), 2);
        assert!(config.cors.allow_credentials);
        assert_eq!(config.cors.max_age, 7200);
    }

    #[test]
    fn test_cors_defaults() {
        let config = CorsConfig::default();
        assert!(!config.enabled);
        assert!(config.allow_origins.is_empty());
        assert_eq!(config.allow_methods.len(), 5); // GET, POST, PUT, DELETE, OPTIONS
        assert_eq!(config.allow_headers.len(), 3); // Content-Type, Accept, Authorization
        assert!(!config.allow_credentials);
        assert_eq!(config.max_age, 3600);
    }
}
