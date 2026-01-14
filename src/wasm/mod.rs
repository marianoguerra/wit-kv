//! WebAssembly module execution for map/reduce operations.
//!
//! This module provides functionality to execute WebAssembly Components
//! that implement the map/reduce interfaces defined in `mapreduce.wit`.

mod error;
mod map;
mod reduce;
mod runner;

pub use error::WasmError;
pub use map::{KeyFilter, MapOperation, MapResult};
pub use reduce::{ReduceOperation, ReduceResult};
pub use runner::WasmRunner;
