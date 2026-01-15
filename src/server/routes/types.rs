//! Type management handlers.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::io::Write;
use tempfile::NamedTempFile;

use crate::kv::{KeyspaceList, KeyspaceMetadata};

use super::super::{
    content::{AcceptFormat, ContentFormat, FormatResponse},
    error::ApiError,
    state::AppState,
};

/// Query parameters for setting a type.
#[derive(Debug, Deserialize, Default)]
pub struct SetTypeQuery {
    /// Name of the type within the WIT definition.
    pub type_name: Option<String>,
    /// Force overwrite if type already exists.
    #[serde(default)]
    pub force: bool,
}

/// Query parameters for deleting a type.
#[derive(Debug, Deserialize, Default)]
pub struct DeleteTypeQuery {
    /// Also delete all data in the keyspace.
    #[serde(default)]
    pub delete_data: bool,
}

/// Type metadata response (JSON serializable version of KeyspaceMetadata).
#[derive(Debug, Serialize)]
pub struct TypeMetadataResponse {
    pub name: String,
    pub qualified_name: String,
    pub wit_definition: String,
    pub type_name: String,
    pub type_version: TypeVersionResponse,
    pub type_hash: u32,
    pub created_at: u64,
}

/// Semantic version response.
#[derive(Debug, Serialize)]
pub struct TypeVersionResponse {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl From<KeyspaceMetadata> for TypeMetadataResponse {
    fn from(m: KeyspaceMetadata) -> Self {
        Self {
            name: m.name,
            qualified_name: m.qualified_name,
            wit_definition: m.wit_definition,
            type_name: m.type_name,
            type_version: TypeVersionResponse {
                major: m.type_version.major,
                minor: m.type_version.minor,
                patch: m.type_version.patch,
            },
            type_hash: m.type_hash,
            created_at: m.created_at,
        }
    }
}

/// List all types in a database.
pub async fn list_types(
    State(state): State<AppState>,
    Path(database): Path<String>,
    AcceptFormat(format): AcceptFormat,
) -> Result<Response, ApiError> {
    let store = state.get_database(&database)?;
    let types = store.list_types()?;
    let keyspace_list = KeyspaceList::new(types);

    match format {
        ContentFormat::Wave => {
            let wave = keyspace_list
                .to_wave()
                .map_err(|e| ApiError::internal(e.to_string()))?;
            Ok(FormatResponse::wave(wave).into_response())
        }
        ContentFormat::Binary => {
            let (buffer, memory) = keyspace_list
                .encode()
                .map_err(|e| ApiError::internal(e.to_string()))?;

            let mut bytes = buffer;
            bytes.extend(memory);

            Ok(FormatResponse::binary(bytes).into_response())
        }
    }
}

/// Get type metadata for a keyspace.
pub async fn get_type(
    State(state): State<AppState>,
    Path((database, keyspace)): Path<(String, String)>,
) -> Result<Json<TypeMetadataResponse>, ApiError> {
    let store = state.get_database(&database)?;

    let metadata = store
        .get_type(&keyspace)?
        .ok_or_else(|| ApiError::keyspace_not_found(&database, &keyspace))?;

    Ok(Json(metadata.into()))
}

/// Register a type for a keyspace.
pub async fn set_type(
    State(state): State<AppState>,
    Path((database, keyspace)): Path<(String, String)>,
    Query(query): Query<SetTypeQuery>,
    body: Bytes,
) -> Result<Json<TypeMetadataResponse>, ApiError> {
    let store = state.get_database(&database)?;

    // The WIT definition is provided in the request body
    let wit_content = std::str::from_utf8(&body)
        .map_err(|e| ApiError::invalid_wave_format(format!("Invalid UTF-8 in WIT definition: {}", e)))?;

    // Write to a temporary file since set_type expects a file path
    let mut temp_file = NamedTempFile::new()
        .map_err(|e| ApiError::internal(format!("Failed to create temp file: {}", e)))?;

    temp_file
        .write_all(wit_content.as_bytes())
        .map_err(|e| ApiError::internal(format!("Failed to write temp file: {}", e)))?;

    let metadata = store.set_type(
        &keyspace,
        temp_file.path(),
        query.type_name.as_deref(),
        query.force,
    )?;

    Ok(Json(metadata.into()))
}

/// Delete a type from a keyspace.
pub async fn delete_type(
    State(state): State<AppState>,
    Path((database, keyspace)): Path<(String, String)>,
    Query(query): Query<DeleteTypeQuery>,
) -> Result<StatusCode, ApiError> {
    let store = state.get_database(&database)?;
    store.delete_type(&keyspace, query.delete_data)?;
    Ok(StatusCode::NO_CONTENT)
}
