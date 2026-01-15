//! API error types and JSON response formatting.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::kv::KvError;

/// API error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

/// Error details in the response.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// API error type that converts to HTTP responses.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    /// Create a new API error.
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }

    /// Add details to the error.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Database not found error.
    pub fn database_not_found(name: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "DATABASE_NOT_FOUND",
            format!("Database '{}' not found", name),
        )
        .with_details(serde_json::json!({ "database": name }))
    }

    /// Keyspace not found error.
    pub fn keyspace_not_found(database: &str, keyspace: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "KEYSPACE_NOT_FOUND",
            format!("Keyspace '{}' not found in database '{}'", keyspace, database),
        )
        .with_details(serde_json::json!({ "database": database, "keyspace": keyspace }))
    }

    /// Key not found error.
    pub fn key_not_found(database: &str, keyspace: &str, key: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "KEY_NOT_FOUND",
            format!("Key '{}' not found in keyspace '{}' of database '{}'", key, keyspace, database),
        )
        .with_details(serde_json::json!({ "database": database, "keyspace": keyspace, "key": key }))
    }

    /// Keyspace already exists error.
    pub fn keyspace_exists(database: &str, keyspace: &str) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "KEYSPACE_EXISTS",
            format!("Keyspace '{}' already exists in database '{}'", keyspace, database),
        )
        .with_details(serde_json::json!({ "database": database, "keyspace": keyspace }))
    }

    /// Invalid Wave format error.
    pub fn invalid_wave_format(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "INVALID_WAVE_FORMAT", message)
    }

    /// Invalid binary format error.
    pub fn invalid_binary_format(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "INVALID_BINARY_FORMAT", message)
    }

    /// Unsupported media type error.
    pub fn unsupported_media_type(content_type: &str) -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "UNSUPPORTED_MEDIA_TYPE",
            format!("Content-Type '{}' is not supported", content_type),
        )
    }

    /// Type version mismatch error.
    pub fn type_version_mismatch(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "TYPE_VERSION_MISMATCH", message)
    }

    /// Internal server error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<KvError> for ApiError {
    fn from(err: KvError) -> Self {
        match &err {
            KvError::KeyspaceNotFound(keyspace) => {
                // We don't have database context here, so use generic message
                Self::new(
                    StatusCode::NOT_FOUND,
                    "KEYSPACE_NOT_FOUND",
                    format!("Keyspace '{}' not found", keyspace),
                )
            }
            KvError::KeyspaceExists(keyspace) => Self::new(
                StatusCode::CONFLICT,
                "KEYSPACE_EXISTS",
                format!("Keyspace '{}' already exists", keyspace),
            ),
            KvError::KeyNotFound(key) => Self::new(
                StatusCode::NOT_FOUND,
                "KEY_NOT_FOUND",
                format!("Key '{}' not found", key),
            ),
            KvError::TypeNotFound(type_name) => Self::new(
                StatusCode::NOT_FOUND,
                "TYPE_NOT_FOUND",
                format!("Type '{}' not found", type_name),
            ),
            KvError::TypeVersionMismatch { stored, current } => Self::new(
                StatusCode::CONFLICT,
                "TYPE_VERSION_MISMATCH",
                format!(
                    "Type version mismatch: stored {}.{}.{}, current {}.{}.{}",
                    stored.major, stored.minor, stored.patch, current.major, current.minor, current.patch
                ),
            ),
            KvError::WaveParse(msg) => Self::invalid_wave_format(msg.clone()),
            KvError::NotInitialized(path) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_NOT_INITIALIZED",
                format!("Database at '{}' is not initialized", path),
            ),
            KvError::InvalidFormat(msg) => Self::invalid_binary_format(msg.clone()),
            _ => Self::internal(err.to_string()),
        }
    }
}
