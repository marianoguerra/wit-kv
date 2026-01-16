//! API error types and JSON response formatting.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::{debug, error};

use wit_kv::kv::KvError;
use wit_kv::wasm::WasmError;

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

    /// Internal server error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", message)
    }

    /// Invalid multipart request error.
    pub fn invalid_multipart(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "INVALID_MULTIPART", message)
    }

    /// Missing multipart field error.
    pub fn missing_field(field_name: &str) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "MISSING_FIELD",
            format!("Required field '{}' is missing from the request", field_name),
        )
        .with_details(serde_json::json!({ "field": field_name }))
    }

    /// WASM module error.
    pub fn wasm_error(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "WASM_ERROR", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Log server errors at error level, client errors at debug level
        if self.status.is_server_error() {
            error!(
                status = %self.status.as_u16(),
                code = %self.code,
                message = %self.message,
                "server error response"
            );
        } else if self.status.is_client_error() {
            debug!(
                status = %self.status.as_u16(),
                code = %self.code,
                message = %self.message,
                "client error response"
            );
        }

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

impl From<WasmError> for ApiError {
    fn from(err: WasmError) -> Self {
        match &err {
            WasmError::FunctionNotFound(name) => Self::wasm_error(format!(
                "Required function '{}' not found in module. Map modules must export 'filter' and 'transform'; reduce modules must export 'init-state' and 'reduce'.",
                name
            )),
            WasmError::InvalidSignature { name, expected, actual } => Self::wasm_error(format!(
                "Function '{}' has wrong signature: expected {}, got {}",
                name, expected, actual
            )),
            WasmError::InvalidReturnType { expected } => {
                Self::wasm_error(format!("Invalid return type: expected {}", expected))
            }
            WasmError::Trap(msg) => Self::wasm_error(format!("WASM execution error: {}", msg)),
            WasmError::TypeMismatch { keyspace_type } => Self::wasm_error(format!(
                "Type mismatch: {}",
                keyspace_type
            )),
            WasmError::ModuleLoad(io_err) => {
                Self::wasm_error(format!("Failed to load module: {}", io_err))
            }
            WasmError::Wasmtime(wt_err) => {
                Self::wasm_error(format!("WASM runtime error: {}", wt_err))
            }
            WasmError::CanonicalAbi(abi_err) => {
                Self::wasm_error(format!("Canonical ABI error: {}", abi_err))
            }
            _ => Self::internal(err.to_string()),
        }
    }
}
