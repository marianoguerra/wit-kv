//! Content-type negotiation for Wave text and binary formats.

use axum::{
    body::Bytes,
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};

use super::error::ApiError;

/// MIME type for WASM Wave text format.
pub const MIME_WASM_WAVE: &str = "application/x-wasm-wave";

/// MIME type for binary format.
pub const MIME_OCTET_STREAM: &str = "application/octet-stream";

/// MIME type for plain text (also accepts Wave).
pub const MIME_TEXT_PLAIN: &str = "text/plain";

/// Content format for requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentFormat {
    /// WASM Wave text format (default).
    #[default]
    Wave,
    /// Binary canonical ABI format.
    Binary,
}

impl ContentFormat {
    /// Parse content format from Content-Type header value.
    pub fn from_content_type(content_type: &str) -> Result<Self, ApiError> {
        let mime = content_type.split(';').next().unwrap_or(content_type).trim();

        match mime {
            MIME_WASM_WAVE | MIME_TEXT_PLAIN | "" => Ok(ContentFormat::Wave),
            MIME_OCTET_STREAM => Ok(ContentFormat::Binary),
            _ => Err(ApiError::unsupported_media_type(mime)),
        }
    }

    /// Parse content format from Accept header value.
    pub fn from_accept(accept: &str) -> Self {
        // Simple parsing - check for binary first, otherwise default to Wave
        let accept_lower = accept.to_lowercase();

        if accept_lower.contains(MIME_OCTET_STREAM) {
            ContentFormat::Binary
        } else {
            ContentFormat::Wave
        }
    }

    /// Get the Content-Type header value for this format.
    pub fn content_type_header(&self) -> HeaderValue {
        match self {
            ContentFormat::Wave => HeaderValue::from_static(MIME_WASM_WAVE),
            ContentFormat::Binary => HeaderValue::from_static(MIME_OCTET_STREAM),
        }
    }
}

/// Extractor for the desired response format from Accept header.
pub struct AcceptFormat(pub ContentFormat);

impl<S> FromRequestParts<S> for AcceptFormat
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let format = parts
            .headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(ContentFormat::from_accept)
            .unwrap_or_default();

        Ok(AcceptFormat(format))
    }
}

/// Extractor for request body format from Content-Type header.
pub struct RequestFormat(pub ContentFormat);

impl<S> FromRequestParts<S> for RequestFormat
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let format = match parts.headers.get(header::CONTENT_TYPE) {
            Some(ct) => {
                let ct_str = ct.to_str().map_err(|_| {
                    ApiError::unsupported_media_type("invalid Content-Type header")
                })?;
                ContentFormat::from_content_type(ct_str)?
            }
            None => ContentFormat::Wave,
        };

        Ok(RequestFormat(format))
    }
}

/// Response wrapper that sets the correct Content-Type header.
pub struct FormatResponse {
    format: ContentFormat,
    body: Bytes,
}

impl FormatResponse {
    /// Create a new format response.
    pub fn new(format: ContentFormat, body: impl Into<Bytes>) -> Self {
        Self {
            format,
            body: body.into(),
        }
    }

    /// Create a Wave text response.
    pub fn wave(body: impl Into<Bytes>) -> Self {
        Self::new(ContentFormat::Wave, body)
    }

    /// Create a binary response.
    pub fn binary(body: impl Into<Bytes>) -> Self {
        Self::new(ContentFormat::Binary, body)
    }
}

impl IntoResponse for FormatResponse {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, self.format.content_type_header());
        (StatusCode::OK, headers, self.body).into_response()
    }
}
