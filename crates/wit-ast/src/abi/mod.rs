//! Canonical ABI lowering and lifting for WIT values.

#![allow(dead_code)]

mod buffer;
mod error;
mod memory;
mod wave_lift;
mod wave_lower;

pub use error::CanonicalAbiError;
pub use memory::LinearMemory;

use wasm_wave::value::{Type as WaveType, Value};
use wit_parser::{Resolve, SizeAlign, Type};

/// Canonical ABI implementation for lowering and lifting values.
pub struct CanonicalAbi<'a> {
    pub(crate) resolve: &'a Resolve,
    pub(crate) sizes: SizeAlign,
}

impl<'a> CanonicalAbi<'a> {
    /// Create a new CanonicalAbi instance for the given WIT resolve.
    pub fn new(resolve: &'a Resolve) -> Self {
        let mut sizes = SizeAlign::default();
        sizes.fill(resolve);
        Self { resolve, sizes }
    }

    /// Encode a WAVE value to canonical ABI format.
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

    /// Decode an EncodedValue back to a WAVE value.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedValue {
    pub buffer: Vec<u8>,
    pub memory: Option<Vec<u8>>,
}

impl EncodedValue {
    pub fn new(buffer: Vec<u8>, memory: Option<Vec<u8>>) -> Self {
        Self { buffer, memory }
    }

    pub fn from_buffer(buffer: Vec<u8>) -> Self {
        Self { buffer, memory: None }
    }

    pub fn has_memory(&self) -> bool {
        self.memory.is_some()
    }

    pub fn total_size(&self) -> usize {
        self.buffer.len() + self.memory.as_ref().map_or(0, |m| m.len())
    }
}
