//! WIT Value encoding/decoding library using canonical ABI.
//!
//! This library provides functions to lower (encode) and lift (decode) WIT values
//! to/from binary format using the WebAssembly Component Model's canonical ABI.
//!
//! # Quick Start
//!
//! ```ignore
//! use wit_kv::prelude::*;
//!
//! // Initialize a typed key-value store
//! let store = KvStore::init(".wit-kv")?;
//!
//! // Register a type for a keyspace
//! store.set_type("points", "types.wit", Some("point"), false)?;
//!
//! // Store and retrieve typed values
//! store.set("points", "origin", "{x: 0, y: 0}")?;
//! let value = store.get("points", "origin")?;
//! ```
//!
//! # Modules
//!
//! - [`abi`] - Canonical ABI encoding/decoding implementation (always available)
//! - [`kv`] - Typed key-value store backed by fjall (requires `kv` feature)
//! - [`wasm`] - WebAssembly component execution for map/reduce operations (requires `wasm` feature)
//!
//! # Feature Flags
//!
//! - `kv` - Enable the key-value store module (enabled by default)
//! - `wasm` - Enable WebAssembly component execution (enabled by default)
//! - `cli` - Enable the command-line interface binary
//! - `full` - Enable all features

pub mod abi;
#[cfg(feature = "kv")]
pub mod kv;
pub mod prelude;
#[cfg(feature = "wasm")]
pub mod wasm;

mod error;

// Re-export the unified error type
pub use error::{Error, Result};

// Re-export ABI types
pub use abi::{CanonicalAbi, CanonicalAbiError, EncodedValue, LinearMemory};

// Re-export KV types at crate root for convenience
#[cfg(feature = "kv")]
pub use kv::{
    BinaryExport, KeyspaceMetadata, KvError, KvStore, ParseVersionError, SemanticVersion,
    StoredValue,
};

// Re-export WASM types at crate root for convenience
#[cfg(feature = "wasm")]
pub use wasm::{
    create_placeholder_val, val_to_wave, wave_to_val, TypedRunner, TypedRunnerBuilder, WasmError,
};

// Re-export commonly used types from dependencies for convenience
pub use wasm_wave::value::{resolve_wit_type, Type as WaveType, Value};
pub use wit_parser::{Resolve, Type, TypeId};

/// Find a type by name in a WIT Resolve.
/// Returns the TypeId if found, None otherwise.
pub fn find_type_by_name(resolve: &Resolve, name: &str) -> Option<TypeId> {
    resolve
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some(name))
        .map(|(id, _)| id)
}

/// Find the first named type in a WIT Resolve.
/// Returns the TypeId if found, None if no named types exist.
pub fn find_first_named_type(resolve: &Resolve) -> Option<TypeId> {
    resolve
        .types
        .iter()
        .find(|(_, ty)| ty.name.is_some())
        .map(|(id, _)| id)
}
