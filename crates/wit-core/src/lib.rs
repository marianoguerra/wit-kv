//! Shared WIT utilities for type resolution, canonical ABI encoding, and WAVE helpers.
//!
//! This crate provides the common functionality shared between `wit-kv` and `wit-file`:
//!
//! - WIT type lookup and loading from files and strings
//! - WAVE↔binary conversion via canonical ABI
//! - Re-exports from `wit-kv-abi` (canonical ABI encoding/decoding)
//! - Re-exports from `wit-parser` and `wasm-wave` for convenience
//!
//! # Example
//!
//! ```ignore
//! use wit_core::*;
//!
//! let resolved = load_wit_type_from_path(
//!     Path::new("types.wit"),
//!     Some("point"),
//! )?;
//! let binary = wave_to_binary("{x: 1, y: 2}", &resolved)?;
//! let wave = binary_to_wave(&binary, &resolved)?;
//! ```

mod error;

pub use error::{Error, Result};

// Re-export from wit-kv-abi
pub use wit_kv_abi::{CanonicalAbi, CanonicalAbiError, EncodedValue, LinearMemory};

// Re-export from wit-parser and wasm-wave for convenience
pub use wasm_wave::value::{Type as WaveType, Value, resolve_wit_type};
pub use wasm_wave::{from_str as wave_from_str, to_string as wave_to_string};
pub use wit_parser::{Resolve, Type, TypeId};

/// A fully resolved WIT type: the parse context, type ID, and WAVE type together.
pub struct ResolvedType {
    pub resolve: Resolve,
    pub type_id: TypeId,
    pub wave_type: WaveType,
}

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
/// Parses the WIT file at the given path and returns a [`ResolvedType`]
/// for the specified type. If `type_name` is None, uses the first named type.
///
/// # Example
///
/// ```ignore
/// use wit_core::load_wit_type_from_path;
/// use std::path::Path;
///
/// let resolved = load_wit_type_from_path(
///     Path::new("types.wit"),
///     Some("point"),
/// )?;
/// ```
pub fn load_wit_type_from_path(
    wit_path: &std::path::Path,
    type_name: Option<&str>,
) -> Result<ResolvedType> {
    let mut resolve = Resolve::new();
    resolve.push_path(wit_path)?;

    let type_id = match type_name {
        Some(name) => find_type_by_name(&resolve, name)
            .ok_or_else(|| Error::WaveParse(format!("Type '{}' not found", name))),
        None => find_first_named_type(&resolve)
            .ok_or_else(|| Error::WaveParse("No named type found in WIT file".to_string())),
    }?;

    let wave_type =
        resolve_wit_type(&resolve, type_id).map_err(|e| Error::WaveParse(e.to_string()))?;

    Ok(ResolvedType {
        resolve,
        type_id,
        wave_type,
    })
}

/// Load a WIT type definition from a string.
///
/// Returns a [`ResolvedType`] for the specified type.
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
/// let resolved = load_wit_type_from_string(wit_def, Some("point"))?;
/// ```
pub fn load_wit_type_from_string(
    wit_definition: &str,
    type_name: Option<&str>,
) -> Result<ResolvedType> {
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

    Ok(ResolvedType {
        resolve,
        type_id,
        wave_type,
    })
}

/// Encode WAVE text to canonical ABI binary.
pub fn wave_to_binary(wave_text: &str, resolved: &ResolvedType) -> Result<Vec<u8>> {
    let wave_text = wave_text.trim();
    let value = wave_from_str(&resolved.wave_type, wave_text)
        .map_err(|e| Error::WaveParse(e.to_string()))?;

    let abi = CanonicalAbi::new(&resolved.resolve);
    let ty = Type::Id(resolved.type_id);
    let mut memory = LinearMemory::new();
    let buffer = abi.lower_with_memory(&value, &ty, &resolved.wave_type, &mut memory)?;

    let memory_bytes = memory.into_bytes();
    let mut result = buffer;
    if !memory_bytes.is_empty() {
        result.extend_from_slice(&memory_bytes);
    }
    Ok(result)
}

/// Decode canonical ABI binary to WAVE text.
pub fn binary_to_wave(binary: &[u8], resolved: &ResolvedType) -> Result<String> {
    let abi = CanonicalAbi::new(&resolved.resolve);
    let ty = Type::Id(resolved.type_id);
    let flat_size = abi.flat_size(&ty);

    if binary.len() < flat_size {
        return Err(Error::DataTooSmall {
            expected: flat_size,
            actual: binary.len(),
        });
    }

    let (buffer, memory_bytes) = binary.split_at(flat_size);
    let memory = if memory_bytes.is_empty() {
        LinearMemory::new()
    } else {
        LinearMemory::from_bytes(memory_bytes.to_vec())
    };

    let (value, _) = abi.lift_with_memory(buffer, &ty, &resolved.wave_type, &memory)?;
    wave_to_string(&value).map_err(|e| Error::WaveParse(e.to_string()))
}
