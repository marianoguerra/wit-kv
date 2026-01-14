//! WIT Value encoding/decoding library using canonical ABI.
//!
//! This library provides functions to lower (encode) and lift (decode) WIT values
//! to/from binary format using the WebAssembly Component Model's canonical ABI.

pub mod abi;

pub use abi::{CanonicalAbi, CanonicalAbiError, LinearMemory};

// Re-export commonly used types from dependencies for convenience
pub use wasm_wave::value::{resolve_wit_type, Type as WaveType, Value};
pub use wit_parser::{Resolve, Type, TypeId};
