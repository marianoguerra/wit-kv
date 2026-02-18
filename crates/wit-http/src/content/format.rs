//! Content format detection, encoding, and decoding.
//!
//! This module is transport-agnostic (no axum dependency). It provides
//! the bridge between Rust types (via `WitType`) and wire formats
//! (WAVE text or canonical ABI binary).

use crate::core::wit_type::WitType;

/// MIME type for WASM Wave text format.
pub const MIME_WASM_WAVE: &str = "application/x-wasm-wave";

/// MIME type for binary canonical ABI format.
pub const MIME_OCTET_STREAM: &str = "application/octet-stream";

/// MIME type for plain text (also accepted as Wave).
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

/// Errors from content format detection and encoding/decoding.
#[derive(Debug, thiserror::Error)]
pub enum ContentFormatError {
    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),
    #[error("decoding error: {0}")]
    Decoding(String),
    #[error("encoding error: {0}")]
    Encoding(String),
}

impl ContentFormat {
    /// Parse content format from a Content-Type header value.
    pub fn from_content_type(ct: &str) -> Result<Self, ContentFormatError> {
        let mime = ct.split(';').next().unwrap_or(ct).trim();
        match mime {
            MIME_WASM_WAVE | MIME_TEXT_PLAIN | "" => Ok(ContentFormat::Wave),
            MIME_OCTET_STREAM => Ok(ContentFormat::Binary),
            _ => Err(ContentFormatError::UnsupportedMediaType(mime.to_string())),
        }
    }

    /// Parse content format from an Accept header value.
    ///
    /// Defaults to Wave if no binary format is requested.
    pub fn from_accept(accept: &str) -> Self {
        if accept.to_lowercase().contains(MIME_OCTET_STREAM) {
            ContentFormat::Binary
        } else {
            ContentFormat::Wave
        }
    }

    /// Get the MIME type string for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            ContentFormat::Wave => MIME_WASM_WAVE,
            ContentFormat::Binary => MIME_OCTET_STREAM,
        }
    }
}

/// Decode an HTTP request body into a Rust type.
///
/// Both formats go through `wasm_wave::Value` as the intermediate representation,
/// then use serde to convert to the target type.
pub fn decode_request<T: WitType>(
    format: ContentFormat,
    body: &[u8],
) -> Result<T, ContentFormatError> {
    let resolved = T::resolved_type()
        .map_err(|e| ContentFormatError::Decoding(e.to_string()))?;

    let value = match format {
        ContentFormat::Wave => {
            let text = std::str::from_utf8(body)
                .map_err(|e| ContentFormatError::Decoding(e.to_string()))?;
            wit_core::wave_from_str(&resolved.wave_type, text)
                .map_err(|e| ContentFormatError::Decoding(e.to_string()))?
        }
        ContentFormat::Binary => {
            wit_core::binary_to_value(body, &resolved)
                .map_err(|e| ContentFormatError::Decoding(e.to_string()))?
        }
    };

    wit_core::serde::from_value(&value)
        .map_err(|e| ContentFormatError::Decoding(e.to_string()))
}

/// Encode a Rust type into an HTTP response body.
///
/// Serializes the Rust type to `wasm_wave::Value` via serde, then converts
/// to the requested wire format.
pub fn encode_response<T: WitType>(
    format: ContentFormat,
    item: &T,
) -> Result<Vec<u8>, ContentFormatError> {
    let resolved = T::resolved_type()
        .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;

    let value = wit_core::serde::to_value(item, &resolved.wave_type)
        .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;

    match format {
        ContentFormat::Wave => {
            let text = wit_core::wave_to_string(&value)
                .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;
            Ok(text.into_bytes())
        }
        ContentFormat::Binary => wit_core::value_to_binary(&value, &resolved)
            .map_err(|e| ContentFormatError::Encoding(e.to_string())),
    }
}

/// Encode a list of items into an HTTP response body.
///
/// Builds a WAVE list `[item1, item2, ...]` and returns it in the
/// requested format.
pub fn encode_list_response<T: WitType>(
    format: ContentFormat,
    items: &[T],
) -> Result<Vec<u8>, ContentFormatError> {
    let resolved = T::resolved_type()
        .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;

    let wave_items: Result<Vec<String>, _> = items
        .iter()
        .map(|item| {
            let value = wit_core::serde::to_value(item, &resolved.wave_type)
                .map_err(|e| ContentFormatError::Encoding(e.to_string()))?;
            wit_core::wave_to_string(&value)
                .map_err(|e| ContentFormatError::Encoding(e.to_string()))
        })
        .collect();
    let wave_list = format!("[{}]", wave_items?.join(", "));

    match format {
        ContentFormat::Wave => Ok(wave_list.into_bytes()),
        ContentFormat::Binary => {
            // Binary list encoding requires constructing a list<T> ResolvedType.
            // Deferred to a future iteration.
            Err(ContentFormatError::Encoding(
                "binary encoding of list responses is not yet supported".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_format_from_content_type() {
        assert_eq!(
            ContentFormat::from_content_type(MIME_WASM_WAVE).ok(),
            Some(ContentFormat::Wave)
        );
        assert_eq!(
            ContentFormat::from_content_type(MIME_OCTET_STREAM).ok(),
            Some(ContentFormat::Binary)
        );
        assert_eq!(
            ContentFormat::from_content_type(MIME_TEXT_PLAIN).ok(),
            Some(ContentFormat::Wave)
        );
        assert_eq!(
            ContentFormat::from_content_type("").ok(),
            Some(ContentFormat::Wave)
        );
        assert!(ContentFormat::from_content_type("application/json").is_err());
    }

    #[test]
    fn content_format_from_accept() {
        assert_eq!(
            ContentFormat::from_accept(MIME_OCTET_STREAM),
            ContentFormat::Binary
        );
        assert_eq!(
            ContentFormat::from_accept(MIME_WASM_WAVE),
            ContentFormat::Wave
        );
        assert_eq!(ContentFormat::from_accept("*/*"), ContentFormat::Wave);
        assert_eq!(ContentFormat::from_accept(""), ContentFormat::Wave);
    }

    #[test]
    fn content_format_mime_type() {
        assert_eq!(ContentFormat::Wave.mime_type(), MIME_WASM_WAVE);
        assert_eq!(ContentFormat::Binary.mime_type(), MIME_OCTET_STREAM);
    }

    #[test]
    fn content_format_default_is_wave() {
        assert_eq!(ContentFormat::default(), ContentFormat::Wave);
    }
}
