//! Key-value operation handlers.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::kv::BinaryExport;

use super::super::{
    content::{AcceptFormat, ContentFormat, FormatResponse, RequestFormat},
    error::ApiError,
    state::AppState,
};

/// Query parameters for listing keys.
#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub limit: Option<usize>,
}

/// List keys in a keyspace.
pub async fn list_keys(
    State(state): State<AppState>,
    Path((database, keyspace)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<String>>, ApiError> {
    let store = state.get_database(&database)?;

    let keys = store.list(
        &keyspace,
        query.prefix.as_deref(),
        query.start.as_deref(),
        query.end.as_deref(),
        query.limit,
    )?;

    Ok(Json(keys))
}

/// Get a value from the store.
pub async fn get_value(
    State(state): State<AppState>,
    Path((database, keyspace, key)): Path<(String, String, String)>,
    AcceptFormat(format): AcceptFormat,
) -> Result<Response, ApiError> {
    let store = state.get_database(&database)?;

    match format {
        ContentFormat::Wave => {
            let value = store
                .get(&keyspace, &key)?
                .ok_or_else(|| ApiError::key_not_found(&database, &keyspace, &key))?;
            Ok(FormatResponse::wave(value).into_response())
        }
        ContentFormat::Binary => {
            let stored = store
                .get_raw(&keyspace, &key)?
                .ok_or_else(|| ApiError::key_not_found(&database, &keyspace, &key))?;

            let export = BinaryExport::from_stored(&stored);
            let (buffer, memory) = export
                .encode()
                .map_err(|e| ApiError::internal(e.to_string()))?;

            // Concatenate buffer and memory for transport
            let mut bytes = buffer;
            bytes.extend(memory);

            Ok(FormatResponse::binary(bytes).into_response())
        }
    }
}

/// Set a value in the store.
pub async fn set_value(
    State(state): State<AppState>,
    Path((database, keyspace, key)): Path<(String, String, String)>,
    RequestFormat(format): RequestFormat,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let store = state.get_database(&database)?;

    match format {
        ContentFormat::Wave => {
            let wave_str = std::str::from_utf8(&body)
                .map_err(|e| ApiError::invalid_wave_format(format!("Invalid UTF-8: {}", e)))?;
            store.set(&keyspace, &key, wave_str)?;
        }
        ContentFormat::Binary => {
            // Decode the binary export and set via raw API
            // For now, we require Wave format for setting values
            // Binary set would require a way to set raw StoredValue
            return Err(ApiError::new(
                StatusCode::NOT_IMPLEMENTED,
                "BINARY_SET_NOT_IMPLEMENTED",
                "Setting values via binary format is not yet implemented",
            ));
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Delete a value from the store.
pub async fn delete_value(
    State(state): State<AppState>,
    Path((database, keyspace, key)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let store = state.get_database(&database)?;
    store.delete(&keyspace, &key)?;
    Ok(StatusCode::NO_CONTENT)
}
