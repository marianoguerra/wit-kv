//! WebAssembly module execution for map/reduce operations.
//!
//! This module provides functionality to execute WebAssembly Components
//! that implement the map/reduce interfaces defined in `mapreduce.wit`.
//!
//! Components receive actual WIT types with direct field access.
//! The `TypedRunner` handles type conversion between stored values and
//! component interfaces, used by the `map` and `reduce` commands.

mod error;
mod map;
mod reduce;
mod runner;
mod typed_runner;

pub use error::WasmError;
pub use map::{KeyFilter, MapOperation, MapResult};
pub use reduce::{ReduceOperation, ReduceResult};
pub use runner::WasmRunner;
pub use typed_runner::{wave_to_val, val_to_wave, create_placeholder_val, TypedRunner};
