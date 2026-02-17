use wit_core::{CanonicalAbi, LinearMemory, Type, wave_from_str};

use super::error::{ErrorKind, ValidationError};
use super::schema::ParsedSchema;

/// Validate WAVE text against a parsed schema and return the encoded binary.
pub fn validate_wave(
    wave_text: &str,
    schema: &ParsedSchema,
) -> Result<Vec<u8>, ValidationError> {
    let wave_text = wave_text.trim();

    // Parse WAVE text
    let value = wave_from_str(&schema.resolved.wave_type, wave_text).map_err(|e| {
        ValidationError::new(e.to_string(), wave_text.to_string(), ErrorKind::WaveParse)
    })?;

    // Encode to canonical ABI binary
    let abi = CanonicalAbi::new(&schema.resolved.resolve);
    let ty = Type::Id(schema.resolved.type_id);
    let mut memory = LinearMemory::new();
    let buffer = abi
        .lower_with_memory(&value, &ty, &schema.resolved.wave_type, &mut memory)
        .map_err(|e| {
            ValidationError::new(e.to_string(), wave_text.to_string(), ErrorKind::AbiError)
        })?;

    let memory_bytes = memory.into_bytes();
    let mut result = buffer;
    if !memory_bytes.is_empty() {
        result.extend_from_slice(&memory_bytes);
    }
    Ok(result)
}

/// Validate canonical ABI binary against a parsed schema.
///
/// Returns the binary data unchanged if valid.
pub fn validate_binary(
    binary: &[u8],
    schema: &ParsedSchema,
) -> Result<Vec<u8>, ValidationError> {
    let abi = CanonicalAbi::new(&schema.resolved.resolve);
    let ty = Type::Id(schema.resolved.type_id);
    let flat_size = abi.flat_size(&ty);

    if binary.len() < flat_size {
        return Err(ValidationError::new(
            format!(
                "Binary data is {} bytes but type requires at least {} bytes",
                binary.len(),
                flat_size
            ),
            format!("<{} bytes of binary data>", binary.len()),
            ErrorKind::AbiError,
        ));
    }

    // Split into buffer and memory
    let (buffer, memory_bytes) = binary.split_at(flat_size);
    let memory = if memory_bytes.is_empty() {
        LinearMemory::new()
    } else {
        LinearMemory::from_bytes(memory_bytes.to_vec())
    };

    // Try to lift the value to verify it's valid
    let _value = abi
        .lift_with_memory(buffer, &ty, &schema.resolved.wave_type, &memory)
        .map_err(|e| {
            ValidationError::new(
                e.to_string(),
                format!("<{} bytes of binary data>", binary.len()),
                ErrorKind::AbiError,
            )
        })?;

    Ok(binary.to_vec())
}
