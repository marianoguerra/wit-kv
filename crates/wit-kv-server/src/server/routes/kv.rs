//! Key-value operation handlers.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{debug, info, instrument};

use wit_kv::kv::{BinaryExport, KeyList};

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
#[instrument(skip(state, format), fields(database = %database, keyspace = %keyspace))]
pub async fn list_keys(
    State(state): State<AppState>,
    Path((database, keyspace)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
    AcceptFormat(format): AcceptFormat,
) -> Result<Response, ApiError> {
    debug!(
        prefix = query.prefix.as_deref(),
        start = query.start.as_deref(),
        end = query.end.as_deref(),
        limit = query.limit,
        "listing keys"
    );

    let store = state.get_database(&database)?;

    let keys = store.list(
        &keyspace,
        query.prefix.as_deref(),
        query.start.as_deref(),
        query.end.as_deref(),
        query.limit,
    )?;

    let count = keys.len();
    let key_list = KeyList::new(keys);

    info!(count, "listed keys");

    match format {
        ContentFormat::Wave => Ok(FormatResponse::wave(key_list.to_wave()).into_response()),
        ContentFormat::Binary => {
            let (buffer, memory) = key_list
                .encode()
                .map_err(|e| ApiError::internal(e.to_string()))?;

            let mut bytes = buffer;
            bytes.extend(memory);

            Ok(FormatResponse::binary(bytes).into_response())
        }
    }
}

/// Get a value from the store.
#[instrument(skip(state, format), fields(database = %database, keyspace = %keyspace, key = %key))]
pub async fn get_value(
    State(state): State<AppState>,
    Path((database, keyspace, key)): Path<(String, String, String)>,
    AcceptFormat(format): AcceptFormat,
) -> Result<Response, ApiError> {
    debug!("getting value");

    let store = state.get_database(&database)?;

    match format {
        ContentFormat::Wave => {
            let value = store
                .get(&keyspace, &key)?
                .ok_or_else(|| ApiError::key_not_found(&database, &keyspace, &key))?;
            info!("retrieved value");
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

            info!("retrieved value (binary)");
            Ok(FormatResponse::binary(bytes).into_response())
        }
    }
}

/// Set a value in the store.
#[instrument(skip(state, format, body), fields(database = %database, keyspace = %keyspace, key = %key, body_len = body.len()))]
pub async fn set_value(
    State(state): State<AppState>,
    Path((database, keyspace, key)): Path<(String, String, String)>,
    RequestFormat(format): RequestFormat,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    debug!("setting value");

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

    info!("value set");
    Ok(StatusCode::NO_CONTENT)
}

/// Delete a value from the store.
#[instrument(skip(state), fields(database = %database, keyspace = %keyspace, key = %key))]
pub async fn delete_value(
    State(state): State<AppState>,
    Path((database, keyspace, key)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    debug!("deleting value");

    let store = state.get_database(&database)?;
    store.delete(&keyspace, &key)?;

    info!("value deleted");
    Ok(StatusCode::NO_CONTENT)
}
