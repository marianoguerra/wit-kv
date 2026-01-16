//! Canonical ABI lowering and lifting for WIT values.
//!
//! This crate implements the canonical ABI memory layout for lowering values
//! to binary and lifting binary data back to values.
//!
//! # Module Organization
//!
//! - [`error`]: Error types for ABI operations
//! - [`memory`]: Simulated linear memory for variable-length types
//! - [`buffer`]: Low-level buffer read/write helpers
//! - [`wave_lower`]: WAVE value lowering to binary
//! - [`wave_lift`]: WAVE value lifting from binary
//! - `val_lower`: Direct wasmtime Val lowering (requires `val` feature)
//! - `val_lift`: Direct wasmtime Val lifting (requires `val` feature)
//! - `val_convert`: Conversions between wasmtime Val and wasm_wave Value (requires `val` feature)
//!
//! # Convenience Types
//!
//! The [`EncodedValue`] struct bundles the main buffer and optional linear memory
//! together, making it easier to pass encoded values around without managing
//! separate components.

mod buffer;
mod error;
mod memory;
#[cfg(feature = "val")]
mod val_convert;
#[cfg(feature = "val")]
mod val_lift;
#[cfg(feature = "val")]
mod val_lower;
mod wave_lift;
mod wave_lower;

pub use error::CanonicalAbiError;
pub use memory::LinearMemory;

#[cfg(feature = "val")]
pub use val_convert::{ValConvertError, val_to_wave, wave_to_val};

use wasm_wave::value::{Type as WaveType, Value};
use wit_parser::{Resolve, SizeAlign, Type};

/// Canonical ABI implementation for lowering and lifting values.
///
/// This struct provides methods to convert between WIT values and their
/// binary canonical ABI representation. It supports both WAVE values
/// (via `wasm_wave`) and direct wasmtime `Val` types.
///
/// # Example
///
/// ```ignore
/// use wit_kv_abi::{CanonicalAbi, LinearMemory};
///
/// let resolve = /* ... */;
/// let abi = CanonicalAbi::new(&resolve);
///
/// // Lower a WAVE value to binary
/// let bytes = abi.lower(&value, &wit_ty, &wave_ty)?;
///
/// // Lift binary data back to a WAVE value
/// let (value, size) = abi.lift(&bytes, &wit_ty, &wave_ty)?;
/// ```
pub struct CanonicalAbi<'a> {
    pub(crate) resolve: &'a Resolve,
    pub(crate) sizes: SizeAlign,
}

impl<'a> CanonicalAbi<'a> {
    /// Create a new CanonicalAbi instance for the given WIT resolve.
    ///
    /// This precomputes size and alignment information for all types
    /// in the resolve, which is used during lowering and lifting.
    pub fn new(resolve: &'a Resolve) -> Self {
        let mut sizes = SizeAlign::default();
        sizes.fill(resolve);
        Self { resolve, sizes }
    }

    /// Encode a WAVE value to canonical ABI format.
    ///
    /// This is a convenience method that handles linear memory automatically,
    /// returning an [`EncodedValue`] that bundles the buffer and memory together.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wit_kv_abi::{CanonicalAbi, EncodedValue};
    ///
    /// let abi = CanonicalAbi::new(&resolve);
    /// let encoded = abi.encode(&value, &wit_ty, &wave_ty)?;
    ///
    /// // Access the components
    /// println!("Buffer size: {}", encoded.buffer.len());
    /// if let Some(mem) = &encoded.memory {
    ///     println!("Memory size: {}", mem.len());
    /// }
    /// ```
    pub fn encode(
        &self,
        value: &Value,
        wit_ty: &Type,
        wave_ty: &WaveType,
    ) -> Result<EncodedValue, CanonicalAbiError> {
        let mut memory = LinearMemory::new();
        let buffer = self.lower_with_memory(value, wit_ty, wave_ty, &mut memory)?;
        Ok(EncodedValue {
            buffer,
            memory: if memory.is_empty() {
                None
            } else {
                Some(memory.into_bytes())
            },
        })
    }

    /// Decode an [`EncodedValue`] back to a WAVE value.
    ///
    /// This is a convenience method that handles linear memory automatically.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wit_kv_abi::{CanonicalAbi, EncodedValue};
    ///
    /// let abi = CanonicalAbi::new(&resolve);
    /// let encoded = abi.encode(&value, &wit_ty, &wave_ty)?;
    /// let decoded = abi.decode(&encoded, &wit_ty, &wave_ty)?;
    /// ```
    pub fn decode(
        &self,
        encoded: &EncodedValue,
        wit_ty: &Type,
        wave_ty: &WaveType,
    ) -> Result<Value, CanonicalAbiError> {
        let memory = LinearMemory::from_option(encoded.memory.clone());
        let (value, _) = self.lift_with_memory(&encoded.buffer, wit_ty, wave_ty, &memory)?;
        Ok(value)
    }
}

/// Encoded value containing both the main buffer and optional linear memory.
///
/// This struct bundles the canonical ABI buffer with any associated linear memory
/// (used for variable-length types like strings and lists), making it convenient
/// to pass encoded values around without managing separate components.
///
/// # Example
///
/// ```ignore
/// use wit_kv_abi::{CanonicalAbi, EncodedValue};
///
/// let abi = CanonicalAbi::new(&resolve);
///
/// // Encode a value
/// let encoded = abi.encode(&value, &wit_ty, &wave_ty)?;
///
/// // Store or transmit the encoded value
/// save_to_file(&encoded.buffer, &encoded.memory);
///
/// // Later, decode it back
/// let decoded = abi.decode(&encoded, &wit_ty, &wave_ty)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedValue {
    /// The main canonical ABI buffer containing fixed-size data.
    ///
    /// For types with variable-length components (strings, lists), this buffer
    /// contains pointer/length pairs that reference data in the `memory` field.
    pub buffer: Vec<u8>,

    /// Optional linear memory containing variable-length data.
    ///
    /// This is `Some` when the value contains strings, lists, or other
    /// variable-length types. The main buffer contains pointers into this memory.
    pub memory: Option<Vec<u8>>,
}

impl EncodedValue {
    /// Create a new EncodedValue with the given buffer and optional memory.
    pub fn new(buffer: Vec<u8>, memory: Option<Vec<u8>>) -> Self {
        Self { buffer, memory }
    }

    /// Create an EncodedValue with only a buffer (no linear memory).
    ///
    /// Use this for fixed-size types that don't contain strings or lists.
    pub fn from_buffer(buffer: Vec<u8>) -> Self {
        Self {
            buffer,
            memory: None,
        }
    }

    /// Returns true if this encoded value has associated linear memory.
    pub fn has_memory(&self) -> bool {
        self.memory.is_some()
    }

    /// Returns the total size in bytes (buffer + memory).
    pub fn total_size(&self) -> usize {
        self.buffer.len() + self.memory.as_ref().map_or(0, |m| m.len())
    }
}
