//! HTTP API server for wit-kv.
//!
//! This module provides an HTTP API on top of the wit-kv library using axum.
//! It supports multiple databases, content-type negotiation (Wave text and binary),
//! and versioned API endpoints.

mod config;
mod content;
mod error;
mod routes;
mod state;

pub use config::{Config, DatabaseConfig, ServerConfig};
pub use content::ContentFormat;
pub use error::ApiError;
pub use routes::router;
pub use state::AppState;
