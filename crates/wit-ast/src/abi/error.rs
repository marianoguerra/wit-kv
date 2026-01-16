//! Error types for canonical ABI operations.

use std::fmt;

/// Errors that can occur during canonical ABI lowering and lifting.
#[derive(Debug)]
pub enum CanonicalAbiError {
    BufferTooSmall { needed: usize, available: usize },
    InvalidUtf8,
    InvalidDiscriminant { discriminant: u32, num_cases: usize },
    InvalidBool(u8),
    InvalidChar(u32),
    TypeMismatch { expected: String, got: String },
    UnsupportedType(String),
    LinearMemoryRequired(String),
    InvalidMemoryPointer { ptr: u32, len: u32, memory_size: usize },
}

impl fmt::Display for CanonicalAbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { needed, available } => {
                write!(f, "Buffer too small: need {} bytes, have {}", needed, available)
            }
            Self::InvalidUtf8 => write!(f, "Invalid UTF-8 in string"),
            Self::InvalidDiscriminant { discriminant, num_cases } => {
                write!(f, "Invalid discriminant {} for variant with {} cases", discriminant, num_cases)
            }
            Self::InvalidBool(v) => write!(f, "Invalid bool value: {}", v),
            Self::InvalidChar(v) => write!(f, "Invalid char value: {}", v),
            Self::TypeMismatch { expected, got } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, got)
            }
            Self::UnsupportedType(t) => write!(f, "Unsupported type: {}", t),
            Self::LinearMemoryRequired(t) => {
                write!(f, "Linear memory required for variable-length type: {}", t)
            }
            Self::InvalidMemoryPointer { ptr, len, memory_size } => {
                write!(f, "Invalid memory pointer: {} with length {} exceeds memory size {}", ptr, len, memory_size)
            }
        }
    }
}

impl std::error::Error for CanonicalAbiError {}
