//! Canonical ABI lowering and lifting for WIT values.
//!
//! This module implements the canonical ABI memory layout for lowering values
//! to binary and lifting binary data back to values.
//!
//! # Module Organization
//!
//! - [`error`]: Error types for ABI operations
//! - [`memory`]: Simulated linear memory for variable-length types
//! - [`buffer`]: Low-level buffer read/write helpers
//! - [`wave_lower`]: WAVE value lowering to binary
//! - [`wave_lift`]: WAVE value lifting from binary
//! - [`val_lower`]: Direct wasmtime Val lowering (hot path)
//! - [`val_lift`]: Direct wasmtime Val lifting (hot path)

mod buffer;
mod error;
mod memory;
mod val_lift;
mod val_lower;
mod wave_lift;
mod wave_lower;

pub use error::CanonicalAbiError;
pub use memory::LinearMemory;

use wit_parser::{Resolve, SizeAlign};

/// Canonical ABI implementation for lowering and lifting values.
///
/// This struct provides methods to convert between WIT values and their
/// binary canonical ABI representation. It supports both WAVE values
/// (via `wasm_wave`) and direct wasmtime `Val` types.
///
/// # Example
///
/// ```ignore
/// use wit_kv::abi::{CanonicalAbi, LinearMemory};
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
}
