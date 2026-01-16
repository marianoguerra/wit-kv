//! Application state management.

use std::collections::HashMap;
use std::sync::Arc;

use wit_kv::kv::KvStore;

use super::config::{Config, DatabaseConfig};
use super::error::ApiError;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Map of database name to KvStore instance.
    databases: Arc<HashMap<String, KvStore>>,
}

impl AppState {
    /// Create a new AppState from configuration.
    pub fn from_config(config: &Config) -> Result<Self, StateError> {
        let mut databases = HashMap::new();

        for db_config in &config.databases {
            let store = Self::open_or_init_database(db_config)?;
            databases.insert(db_config.name.clone(), store);
        }

        Ok(Self {
            databases: Arc::new(databases),
        })
    }

    /// Get a database by name.
    pub fn get_database(&self, name: &str) -> Result<&KvStore, ApiError> {
        self.databases
            .get(name)
            .ok_or_else(|| ApiError::database_not_found(name))
    }

    /// List all database names.
    pub fn database_names(&self) -> Vec<&str> {
        self.databases.keys().map(String::as_str).collect()
    }

    fn open_or_init_database(config: &DatabaseConfig) -> Result<KvStore, StateError> {
        let path = std::path::Path::new(&config.path);

        if path.exists() {
            KvStore::open(path).map_err(|e| StateError::OpenDatabase {
                name: config.name.clone(),
                path: config.path.clone(),
                source: e,
            })
        } else {
            KvStore::init(path).map_err(|e| StateError::InitDatabase {
                name: config.name.clone(),
                path: config.path.clone(),
                source: e,
            })
        }
    }
}

/// Errors that can occur when setting up application state.
#[derive(Debug)]
pub enum StateError {
    /// Failed to open an existing database.
    OpenDatabase {
        name: String,
        path: String,
        source: wit_kv::kv::KvError,
    },
    /// Failed to initialize a new database.
    InitDatabase {
        name: String,
        path: String,
        source: wit_kv::kv::KvError,
    },
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::OpenDatabase { name, path, source } => {
                write!(f, "Failed to open database '{}' at '{}': {}", name, path, source)
            }
            StateError::InitDatabase { name, path, source } => {
                write!(f, "Failed to initialize database '{}' at '{}': {}", name, path, source)
            }
        }
    }
}

impl std::error::Error for StateError {}
