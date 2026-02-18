//! Axum extractors for content negotiation.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};

use crate::content::format::ContentFormat;
use crate::http::error::ApiError;

/// Extractor for the desired response format from the Accept header.
///
/// Infallible: defaults to Wave if the header is missing or unrecognized.
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

/// Extractor for the request body format from the Content-Type header.
///
/// Rejects requests with unsupported content types.
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
                    ApiError::from(
                        crate::content::format::ContentFormatError::UnsupportedMediaType(
                            "invalid Content-Type header".to_string(),
                        ),
                    )
                })?;
                ContentFormat::from_content_type(ct_str)?
            }
            None => ContentFormat::Wave,
        };

        Ok(RequestFormat(format))
    }
}
