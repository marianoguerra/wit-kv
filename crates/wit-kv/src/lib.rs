//! A typed key-value store for WIT values.
//!
//! wit-kv provides a persistent key-value store where each keyspace is
//! associated with a WIT (WebAssembly Interface Types) type definition.
//! Values are stored using the canonical ABI binary format.
//!
//! # Module Organization
//!
//! - [`kv`]: The core key-value store implementation (requires `kv` feature)
//! - [`wasm`]: WebAssembly component execution for map/reduce (requires `wasm` feature)
//! - [`error`]: Unified error types
//! - [`prelude`]: Convenient re-exports
//! - `logging`: Conditional logging macros (internal)
//!
//! # Features
//!
//! - `kv` (default): Key-value store functionality
//! - `wasm` (default): WASM execution for map/reduce operations
//! - `logging`: Enable tracing-based logging
//!
//! # Example
//!
//! ```ignore
//! use wit_kv::prelude::*;
//!
//! // Initialize a store
//! let store = KvStore::init(".wit-kv")?;
//!
//! // Register a type for a keyspace
//! store.set_type("tasks", "types.wit", Some("task"), false)?;
//!
//! // Store and retrieve values
//! store.set("tasks", "task-1", "{name: \"Build\", done: false}")?;
//! let value = store.get("tasks", "task-1")?;
//! ```

pub mod error;
#[cfg(feature = "kv")]
pub mod kv;
#[macro_use]
pub(crate) mod logging;
pub mod prelude;
#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export from wit-kv-abi
pub use wit_kv_abi::{CanonicalAbi, CanonicalAbiError, EncodedValue, LinearMemory};

// Re-export from wit-parser and wasm-wave for convenience
pub use wasm_wave::value::{Type as WaveType, Value, resolve_wit_type};
pub use wasm_wave::{from_str as wave_from_str, to_string as wave_to_string};
pub use wit_parser::{Resolve, Type, TypeId};

// Re-export unified error types
pub use error::{Error, Result};

// Re-export KV types (when feature enabled)
#[cfg(feature = "kv")]
pub use kv::{
    BinaryExport, DatabaseInfo, DatabaseList, KeyList, KeyspaceList, KeyspaceMetadata, KvError,
    KvStore, ParseVersionError, SemanticVersion, StoredValue,
};

// Re-export WASM types (when feature enabled)
#[cfg(feature = "wasm")]
pub use wasm::{TypedRunner, TypedRunnerBuilder, WasmError, create_placeholder_val};

// Re-export Val conversion functions (when wasm feature enabled)
#[cfg(feature = "wasm")]
pub use wit_kv_abi::{ValConvertError, val_to_wave, wave_to_val};

// Re-export shared utilities from wit-core
pub use wit_core::{
    ResolvedType, binary_to_wave, find_first_named_type, find_type_by_name,
    load_wit_type_from_path, load_wit_type_from_string, wave_to_binary,
};
