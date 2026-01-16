//! Error types for the KV store module.

use thiserror::Error;

use crate::CanonicalAbiError;

use super::version::SemanticVersion;

/// Errors that can occur during KV store operations.
#[derive(Error, Debug)]
pub enum KvError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Fjall error: {0}")]
    Fjall(#[from] fjall::Error),

    #[error("Keyspace not found: {0}")]
    KeyspaceNotFound(String),

    #[error("Keyspace already exists: {0}")]
    KeyspaceExists(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Type version mismatch: stored version {stored}, current version {current}")]
    TypeVersionMismatch {
        stored: SemanticVersion,
        current: SemanticVersion,
    },

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Canonical ABI error: {0}")]
    CanonicalAbi(#[from] CanonicalAbiError),

    #[error("WIT parsing error: {0}")]
    WitParse(#[from] anyhow::Error),

    #[error("WAVE parsing error: {0}")]
    WaveParse(String),

    #[error("Type not found in WIT: {0}")]
    TypeNotFound(String),

    #[error("Database not initialized at {0}")]
    NotInitialized(String),
}
