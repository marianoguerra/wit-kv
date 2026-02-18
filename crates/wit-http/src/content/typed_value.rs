//! Conversion between `WitType` structs and `TypedValue`.

use wit_core::{CanonicalAbi, LinearMemory, Type};
use wit_run::TypedValue;

use crate::content::format::ContentFormatError;
use crate::core::wit_type::WitType;

/// Convert a `WitType` struct to a `TypedValue` for WASM processing.
pub fn to_typed_value<T: WitType>(item: &T) -> Result<TypedValue, ContentFormatError> {
    let resolved = T::resolved_type()
        .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;

    let value = wit_core::serde::to_value(item, &resolved.wave_type)
        .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;

    let abi = CanonicalAbi::new(&resolved.resolve);
    let ty = Type::Id(resolved.type_id);
    let mut memory = LinearMemory::new();
    let buffer = abi
        .lower_with_memory(&value, &ty, &resolved.wave_type, &mut memory)
        .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;

    let memory_bytes = memory.into_bytes();
    Ok(TypedValue {
        value: buffer,
        memory: if memory_bytes.is_empty() {
            None
        } else {
            Some(memory_bytes)
        },
    })
}

/// Convert a `TypedValue` back to a `WitType` struct.
pub fn from_typed_value<T: WitType>(typed: &TypedValue) -> Result<T, ContentFormatError> {
    let resolved = T::resolved_type()
        .map_err(|e| ContentFormatError::Decoding(e.to_string()))?;

    let abi = CanonicalAbi::new(&resolved.resolve);
    let ty = Type::Id(resolved.type_id);
    let memory = LinearMemory::from_optional(typed.memory.as_ref());

    let (value, _) = abi
        .lift_with_memory(&typed.value, &ty, &resolved.wave_type, &memory)
        .map_err(|e| ContentFormatError::Decoding(e.to_string()))?;

    wit_core::serde::from_value(&value)
        .map_err(|e| ContentFormatError::Decoding(e.to_string()))
}
