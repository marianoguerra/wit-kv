//! HTTP API server for wit-kv.
//!
//! This module provides an HTTP API on top of the wit-kv library using axum.
//! It supports multiple databases, content-type negotiation (Wave text and binary),
//! and versioned API endpoints.

mod config;
mod content;
mod error;
mod logging;
mod routes;
mod state;

pub use config::{Config, CorsConfig};
pub use logging::init as init_logging;
pub use routes::router;
pub use state::AppState;
