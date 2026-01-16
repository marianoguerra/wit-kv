//! Unified error type for the wit-kv library.
//!
//! This module provides a single [`Error`] type that encompasses all errors
//! that can occur in the library, making it easier to handle errors in
//! application code.

use thiserror::Error;

#[cfg(feature = "kv")]
use crate::kv::KvError;
#[cfg(feature = "wasm")]
use crate::wasm::WasmError;
use wit_kv_abi::CanonicalAbiError;

/// Unified error type for all wit-kv operations.
///
/// This enum wraps all module-specific error types, allowing callers to
/// use a single error type throughout their application.
///
/// # Example
///
/// ```ignore
/// use wit_kv::{Result, KvStore};
///
/// fn do_something() -> Result<()> {
///     let store = KvStore::open(".wit-kv")?;
///     store.set("keyspace", "key", "{value: 42}")?;
///     Ok(())
/// }
/// ```
#[derive(Error, Debug)]
pub enum Error {
    /// Error from canonical ABI encoding/decoding operations.
    #[error(transparent)]
    Abi(#[from] CanonicalAbiError),

    /// Error from key-value store operations.
    #[cfg(feature = "kv")]
    #[error(transparent)]
    Kv(#[from] KvError),

    /// Error from WebAssembly component execution.
    #[cfg(feature = "wasm")]
    #[error(transparent)]
    Wasm(#[from] WasmError),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// WIT parsing error.
    #[error("WIT parsing error: {0}")]
    WitParse(#[from] anyhow::Error),

    /// WAVE parsing error.
    #[error("WAVE parsing error: {0}")]
    WaveParse(String),
}

/// A [`Result`] type alias using the unified [`Error`] type.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a WAVE parsing error from a string message.
    pub fn wave_parse(msg: impl Into<String>) -> Self {
        Self::WaveParse(msg.into())
    }

    /// Returns `true` if this is an ABI error.
    pub fn is_abi(&self) -> bool {
        matches!(self, Self::Abi(_))
    }

    /// Returns `true` if this is a KV store error.
    #[cfg(feature = "kv")]
    pub fn is_kv(&self) -> bool {
        matches!(self, Self::Kv(_))
    }

    /// Returns `true` if this is a WASM execution error.
    #[cfg(feature = "wasm")]
    pub fn is_wasm(&self) -> bool {
        matches!(self, Self::Wasm(_))
    }

    /// Returns `true` if this is an I/O error.
    pub fn is_io(&self) -> bool {
        matches!(self, Self::Io(_))
    }
}
