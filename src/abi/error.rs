//! Error types for canonical ABI operations.

use thiserror::Error;

/// Errors that can occur during canonical ABI lowering and lifting.
#[derive(Error, Debug)]
pub enum CanonicalAbiError {
    #[error("Buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall { needed: usize, available: usize },

    #[error("Invalid UTF-8 in string")]
    InvalidUtf8,

    #[error("Invalid discriminant {discriminant} for variant with {num_cases} cases")]
    InvalidDiscriminant { discriminant: u32, num_cases: usize },

    #[error("Invalid bool value: {0}")]
    InvalidBool(u8),

    #[error("Invalid char value: {0}")]
    InvalidChar(u32),

    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },

    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Linear memory required for variable-length type: {0}")]
    LinearMemoryRequired(String),

    #[error("Invalid memory pointer: {ptr} with length {len} exceeds memory size {memory_size}")]
    InvalidMemoryPointer {
        ptr: u32,
        len: u32,
        memory_size: usize,
    },
}
