//! HTTP error types and JSON response formatting.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tracing::{debug, error};

use crate::content::format::ContentFormatError;
use crate::core::error::{ErrorKind, ResourceError};

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

/// HTTP API error that converts to a JSON error response.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl From<ResourceError> for ApiError {
    fn from(err: ResourceError) -> Self {
        let (status, code) = match err.kind() {
            ErrorKind::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ErrorKind::Conflict => (StatusCode::CONFLICT, "CONFLICT"),
            ErrorKind::BadRequest => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            ErrorKind::NotSupported => (StatusCode::METHOD_NOT_ALLOWED, "NOT_SUPPORTED"),
            ErrorKind::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };
        Self {
            status,
            code,
            message: err.to_string(),
        }
    }
}

impl From<ContentFormatError> for ApiError {
    fn from(err: ContentFormatError) -> Self {
        match &err {
            ContentFormatError::UnsupportedMediaType(_) => Self {
                status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
                code: "UNSUPPORTED_MEDIA_TYPE",
                message: err.to_string(),
            },
            ContentFormatError::Decoding(_) => Self {
                status: StatusCode::BAD_REQUEST,
                code: "DECODING_ERROR",
                message: err.to_string(),
            },
            ContentFormatError::Encoding(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "ENCODING_ERROR",
                message: err.to_string(),
            },
        }
    }
}

#[cfg(feature = "run")]
impl From<wit_run::RunError> for ApiError {
    fn from(err: wit_run::RunError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "WASM_ERROR",
            message: err.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
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
            },
        };
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_error_to_api_error_status_codes() {
        let cases = vec![
            (
                ResourceError::NotFound("x".into()),
                StatusCode::NOT_FOUND,
            ),
            (
                ResourceError::AlreadyExists("x".into()),
                StatusCode::CONFLICT,
            ),
            (
                ResourceError::InvalidInput("x".into()),
                StatusCode::BAD_REQUEST,
            ),
            (
                ResourceError::NotSupported("x".into()),
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (
                ResourceError::Internal("x".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                ResourceError::TypeError("x".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];

        for (resource_err, expected_status) in cases {
            let api_err = ApiError::from(resource_err);
            assert_eq!(api_err.status, expected_status);
        }
    }

    #[test]
    fn content_format_error_to_api_error() {
        let err = ContentFormatError::UnsupportedMediaType("application/xml".into());
        let api = ApiError::from(err);
        assert_eq!(api.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let err = ContentFormatError::Decoding("bad data".into());
        let api = ApiError::from(err);
        assert_eq!(api.status, StatusCode::BAD_REQUEST);

        let err = ContentFormatError::Encoding("failed".into());
        let api = ApiError::from(err);
        assert_eq!(api.status, StatusCode::INTERNAL_SERVER_ERROR);
    }
}
