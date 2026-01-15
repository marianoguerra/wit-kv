//! Convenient re-exports for common usage patterns.
//!
//! This module provides a single import to bring all commonly used types
//! into scope.
//!
//! # Example
//!
//! ```ignore
//! use wit_kv::prelude::*;
//!
//! let store = KvStore::init(".wit-kv")?;
//! store.set_type("points", "types.wit", Some("point"), false)?;
//! store.set("points", "p1", "{x: 10, y: 20}")?;
//! ```

// Unified error handling
pub use crate::error::{Error, Result};

// ABI types
pub use crate::abi::{CanonicalAbi, CanonicalAbiError, EncodedValue, LinearMemory};

// KV store types (requires "kv" feature)
#[cfg(feature = "kv")]
pub use crate::kv::{
    BinaryExport, KeyspaceMetadata, KvError, KvStore, ParseVersionError, SemanticVersion,
    StoredValue,
};

// WASM execution types (requires "wasm" feature)
#[cfg(feature = "wasm")]
pub use crate::wasm::{
    create_placeholder_val, val_to_wave, wave_to_val, TypedRunner, TypedRunnerBuilder, WasmError,
};

// Dependency re-exports
pub use crate::{find_first_named_type, find_type_by_name, resolve_wit_type};
pub use crate::{Resolve, Type, TypeId, Value, WaveType};
