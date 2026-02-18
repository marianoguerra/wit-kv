//! Run WebAssembly components on WIT-typed values.
//!
//! This crate provides the `TypedRunner` for executing WebAssembly Components
//! that operate on WIT-typed values using the canonical ABI.
//!
//! # Example
//!
//! ```ignore
//! use wit_run::{TypedRunner, TypedValue};
//!
//! let mut runner = TypedRunner::builder()
//!     .component("filter.wasm")
//!     .wit("types.wit")
//!     .input_type("point")
//!     .build()?;
//!
//! let input = TypedValue { value: bytes, memory: None };
//! let passes = runner.call_filter(&input)?;
//! ```

mod error;
#[macro_use]
pub(crate) mod logging;
mod runner;
mod value;

pub use error::RunError;
pub use runner::{TypedRunner, TypedRunnerBuilder, create_placeholder_val};
pub use value::TypedValue;
pub use wit_core::{ValConvertError, val_to_wave, wave_to_val};
