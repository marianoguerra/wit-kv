//! WebAssembly module execution for map/reduce operations.
//!
//! This module re-exports types from the `wit-run` crate, providing
//! backward compatibility for existing consumers of `wit-kv`.

pub use wit_run::{
    RunError as WasmError, TypedRunner, TypedRunnerBuilder, TypedValue, ValConvertError,
    create_placeholder_val, val_to_wave, wave_to_val,
};
