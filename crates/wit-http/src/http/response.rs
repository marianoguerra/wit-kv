//! HTTP response wrapper that sets Content-Type from the negotiated format.

use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::content::format::ContentFormat;

/// Response wrapper that sets the correct Content-Type header based on the
/// negotiated content format.
pub struct FormatResponse {
    format: ContentFormat,
    body: Bytes,
    status: StatusCode,
}

impl FormatResponse {
    /// Create a new format response with explicit status code.
    pub fn new(format: ContentFormat, body: impl Into<Bytes>, status: StatusCode) -> Self {
        Self {
            format,
            body: body.into(),
            status,
        }
    }
}

impl IntoResponse for FormatResponse {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(self.format.mime_type()),
        );
        (self.status, headers, self.body).into_response()
    }
}
