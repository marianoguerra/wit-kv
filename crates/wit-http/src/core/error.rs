//! Transport-independent domain errors for WIT resources.

/// Semantic error categories that any transport can map to its status system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    NotFound,
    Conflict,
    BadRequest,
    NotSupported,
    Internal,
}

/// Domain errors for WIT resource operations.
///
/// These are transport-independent: the HTTP adapter maps them to status codes,
/// a future RPC adapter would map them to its own error codes.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not supported: {0}")]
    NotSupported(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("type error: {0}")]
    TypeError(String),
}

impl ResourceError {
    /// Return the semantic error category for this error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::NotFound(_) => ErrorKind::NotFound,
            Self::AlreadyExists(_) => ErrorKind::Conflict,
            Self::InvalidInput(_) => ErrorKind::BadRequest,
            Self::NotSupported(_) => ErrorKind::NotSupported,
            Self::Internal(_) | Self::TypeError(_) => ErrorKind::Internal,
        }
    }
}

impl From<wit_core::Error> for ResourceError {
    fn from(err: wit_core::Error) -> Self {
        Self::TypeError(err.to_string())
    }
}
