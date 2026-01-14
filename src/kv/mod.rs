//! Key-value store module for typed WIT values.
//!
//! This module provides a persistent key-value store where each keyspace
//! is associated with a WIT type. Values are stored using the canonical ABI
//! binary format.

mod error;
mod format;
mod store;
mod types;

pub use error::KvError;
pub use store::KvStore;
pub use types::{KeyspaceMetadata, StoredValue};
