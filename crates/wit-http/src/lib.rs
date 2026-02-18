//! Mount WIT types as RESTful endpoints on axum.
//!
//! `wit-http` bridges WIT-typed Rust structs to HTTP, providing content
//! negotiation (WAVE text and canonical ABI binary), error mapping, and
//! RESTful endpoint generation with minimal boilerplate.
//!
//! # Architecture
//!
//! - **`core`** — Transport-agnostic traits and types (`WitType`, `WitResource`,
//!   `ResourceError`, `CollectionQuery`). Reusable for future transports like RPC.
//! - **`content`** — Content format detection and encoding/decoding (WAVE text
//!   and canonical ABI binary). No axum dependency.
//! - **`http`** — Axum-specific adapter: extractors, error mapping, response
//!   formatting, and the `mount_resource()` function.
//!
//! # Quick Start
//!
//! ```ignore
//! use wit_http::{WitType, WitResource, ResourceError, CollectionQuery, mount_resource, MountConfig};
//! use axum::Router;
//!
//! // 1. Define a WIT-typed struct
//! struct User { name: String, age: u32 }
//! impl WitType for User { /* ... */ }
//!
//! // 2. Implement resource handlers
//! struct UserStore { /* ... */ }
//! impl WitResource for UserStore {
//!     type Item = User;
//!     // override get, set, delete, list as needed...
//! }
//!
//! // 3. Mount as REST endpoints
//! let app = Router::new()
//!     .merge(mount_resource("/api/v1/users", UserStore::new(), MountConfig::crud()));
//! ```

pub mod content;
pub mod core;
pub mod http;

// Re-export core types for convenient access.
pub use core::error::{ErrorKind, ResourceError};
pub use core::query::CollectionQuery;
pub use core::resource::WitResource;
pub use core::wit_type::WitType;

// Re-export content types.
pub use content::format::ContentFormat;

// Re-export HTTP adapter.
pub use http::mount::{MountConfig, mount_resource};

// Re-export wit-core serde for direct Value conversions if needed.
pub use wit_core::serde as wit_serde;
