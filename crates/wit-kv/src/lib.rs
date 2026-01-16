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

/// Find the first named type in a WIT resolve.
///
/// This searches through all types in the resolve and returns the first
/// one that has the given name.
pub fn find_first_named_type(resolve: &Resolve) -> Option<TypeId> {
    resolve
        .types
        .iter()
        .find_map(|(id, ty)| ty.name.as_ref().map(|_| id))
}

/// Find a type by name in a WIT resolve.
///
/// This searches through all types in the resolve and returns the first
/// one with a matching name.
pub fn find_type_by_name(resolve: &Resolve, name: &str) -> Option<TypeId> {
    resolve.types.iter().find_map(|(id, ty)| {
        if ty.name.as_ref().is_some_and(|n| n == name) {
            Some(id)
        } else {
            None
        }
    })
}

/// Load a WIT type definition from a string.
///
/// Returns the Resolve, TypeId, and WaveType for the specified type.
/// If `type_name` is None, uses the first named type in the definition.
///
/// # Example
///
/// ```ignore
/// use wit_kv::load_wit_type_from_string;
///
/// let wit_def = r#"
///     package test:types;
///     interface types {
///         record point { x: u32, y: u32 }
///     }
/// "#;
///
/// let (resolve, type_id, wave_type) = load_wit_type_from_string(wit_def, Some("point"))?;
/// ```
pub fn load_wit_type_from_string(
    wit_definition: &str,
    type_name: Option<&str>,
) -> Result<(Resolve, TypeId, WaveType)> {
    let mut resolve = Resolve::new();
    resolve.push_str("input.wit", wit_definition)?;

    let type_id = match type_name {
        Some(name) => find_type_by_name(&resolve, name)
            .ok_or_else(|| Error::WaveParse(format!("Type '{}' not found", name))),
        None => find_first_named_type(&resolve)
            .ok_or_else(|| Error::WaveParse("No named type found in WIT definition".to_string())),
    }?;

    let wave_type =
        resolve_wit_type(&resolve, type_id).map_err(|e| Error::WaveParse(e.to_string()))?;

    Ok((resolve, type_id, wave_type))
}
