//! Shared WIT utilities for type resolution, canonical ABI encoding, and WAVE helpers.
//!
//! This crate provides the common functionality shared between `wit-kv` and `wit-file`:
//!
//! - WIT type lookup and loading from files and strings
//! - Re-exports from `wit-kv-abi` (canonical ABI encoding/decoding)
//! - Re-exports from `wit-parser` and `wasm-wave` for convenience
//!
//! # Example
//!
//! ```ignore
//! use wit_core::*;
//!
//! let (resolve, type_id) = load_wit_type_from_path(
//!     Path::new("types.wit"),
//!     Some("point"),
//! )?;
//! let wave_type = resolve_wit_type(&resolve, type_id)?;
//! ```

mod error;

pub use error::{Error, Result};

// Re-export from wit-kv-abi
pub use wit_kv_abi::{CanonicalAbi, CanonicalAbiError, EncodedValue, LinearMemory};

// Re-export from wit-parser and wasm-wave for convenience
pub use wasm_wave::value::{Type as WaveType, Value, resolve_wit_type};
pub use wasm_wave::{from_str as wave_from_str, to_string as wave_to_string};
pub use wit_parser::{Resolve, Type, TypeId};

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

/// Load a WIT type definition from a file path.
///
/// Parses the WIT file at the given path and returns the Resolve and TypeId
/// for the specified type. If `type_name` is None, uses the first named type.
///
/// # Example
///
/// ```ignore
/// use wit_core::load_wit_type_from_path;
/// use std::path::Path;
///
/// let (resolve, type_id) = load_wit_type_from_path(
///     Path::new("types.wit"),
///     Some("point"),
/// )?;
/// ```
pub fn load_wit_type_from_path(
    wit_path: &std::path::Path,
    type_name: Option<&str>,
) -> Result<(Resolve, TypeId)> {
    let mut resolve = Resolve::new();
    resolve.push_path(wit_path)?;

    let type_id = match type_name {
        Some(name) => find_type_by_name(&resolve, name)
            .ok_or_else(|| Error::WaveParse(format!("Type '{}' not found", name))),
        None => find_first_named_type(&resolve)
            .ok_or_else(|| Error::WaveParse("No named type found in WIT file".to_string())),
    }?;

    Ok((resolve, type_id))
}

/// Load a WIT type definition from a string.
///
/// Returns the Resolve, TypeId, and WaveType for the specified type.
/// If `type_name` is None, uses the first named type in the definition.
///
/// # Example
///
/// ```ignore
/// use wit_core::load_wit_type_from_string;
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
