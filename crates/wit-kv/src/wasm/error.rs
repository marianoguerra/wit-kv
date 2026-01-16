//! Error types for WebAssembly module execution.

use thiserror::Error;

use wit_kv_abi::{CanonicalAbiError, ValConvertError};

/// Errors that can occur during WebAssembly module execution.
#[derive(Error, Debug)]
pub enum WasmError {
    /// Failed to load the WebAssembly module file.
    #[error("Failed to load wasm module: {0}")]
    ModuleLoad(#[from] std::io::Error),

    /// Wasmtime engine or execution error.
    #[error("Wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),

    /// Required function not found in the module exports.
    #[error("Function not found in module: {0}")]
    FunctionNotFound(String),

    /// Function signature does not match the expected interface.
    #[error("Invalid function signature for '{name}': expected {expected}, got {actual}")]
    InvalidSignature {
        name: String,
        expected: String,
        actual: String,
    },

    /// Function returned an unexpected type.
    #[error("Invalid return type: expected {expected}")]
    InvalidReturnType { expected: String },

    /// WebAssembly execution trapped (runtime error).
    #[error("Wasm execution trapped: {0}")]
    Trap(String),

    /// Type mismatch between keyspace type and module expectations.
    #[error("Type mismatch: keyspace type '{keyspace_type}' incompatible with module")]
    TypeMismatch { keyspace_type: String },

    /// KV store error during iteration.
    #[error("KV store error: {0}")]
    KvError(#[from] crate::kv::KvError),

    /// Component encoding error.
    #[error("Component encoding error: {0}")]
    ComponentEncoding(String),

    /// Canonical ABI error.
    #[error("Canonical ABI error: {0}")]
    CanonicalAbi(#[from] CanonicalAbiError),

    /// Val conversion error.
    #[error("Val conversion error: {0}")]
    ValConvert(#[from] ValConvertError),
}
