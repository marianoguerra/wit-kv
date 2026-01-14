//! WebAssembly module execution for map/reduce operations.
//!
//! This module provides functionality to execute WebAssembly Components
//! that implement typed map/reduce operations.
//!
//! Components receive actual WIT types with direct field access.
//! The `TypedRunner` handles type conversion between stored values and
//! component interfaces, used by the `map` and `reduce` commands.

mod error;
mod typed_runner;

pub use error::WasmError;
pub use typed_runner::{create_placeholder_val, val_to_wave, wave_to_val, TypedRunner};
