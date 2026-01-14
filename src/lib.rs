//! WIT Value encoding/decoding library using canonical ABI.
//!
//! This library provides functions to lower (encode) and lift (decode) WIT values
//! to/from binary format using the WebAssembly Component Model's canonical ABI.

pub mod abi;
pub mod kv;
pub mod wasm;

pub use abi::{CanonicalAbi, CanonicalAbiError, LinearMemory};

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
