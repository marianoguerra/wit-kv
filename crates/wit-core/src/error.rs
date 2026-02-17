//! Error types for wit-core operations.

use thiserror::Error;
use wit_kv_abi::CanonicalAbiError;

/// Error type for wit-core operations.
#[derive(Error, Debug)]
pub enum Error {
    /// Error from canonical ABI encoding/decoding operations.
    #[error(transparent)]
    Abi(#[from] CanonicalAbiError),

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

/// A [`Result`] type alias using the [`Error`] type.
pub type Result<T> = std::result::Result<T, Error>;
