//! Value types for WASM execution.

/// A typed value carrier for the WASM boundary.
///
/// This is the generic equivalent of `StoredValue` without KV store metadata.
/// It carries the canonical ABI flat buffer and optional linear memory bytes
/// needed to pass WIT-typed values to and from WASM components.
#[derive(Debug, Clone)]
pub struct TypedValue {
    /// Canonical ABI flat buffer.
    pub value: Vec<u8>,
    /// Linear memory bytes (for variable-length types: strings, lists).
    pub memory: Option<Vec<u8>>,
}
