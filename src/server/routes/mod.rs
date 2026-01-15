//! API routes and handlers.

mod kv;
mod mapreduce;
mod types;

use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Router,
};

use crate::kv::DatabaseList;

use super::{
    content::{AcceptFormat, ContentFormat, FormatResponse},
    error::ApiError,
    state::AppState,
};

/// Build the API router.
pub fn router(state: AppState) -> Router {
    let db_routes = Router::new()
        // KV operations
        .route("/kv/{keyspace}", get(kv::list_keys))
        .route("/kv/{keyspace}/{key}", get(kv::get_value))
        .route("/kv/{keyspace}/{key}", put(kv::set_value))
        .route("/kv/{keyspace}/{key}", delete(kv::delete_value))
        // Type operations
        .route("/types", get(types::list_types))
        .route("/types/{keyspace}", get(types::get_type))
        .route("/types/{keyspace}", put(types::set_type))
        .route("/types/{keyspace}", delete(types::delete_type))
        // Map/reduce operations
        .route("/map/{keyspace}", post(mapreduce::map_operation))
        .route("/reduce/{keyspace}", post(mapreduce::reduce_operation));

    Router::new()
        .route("/health", get(health))
        .route("/api/v1/databases", get(list_databases))
        .nest("/api/v1/db/{database}", db_routes)
        .with_state(state)
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}

/// List all databases.
pub async fn list_databases(
    State(state): State<AppState>,
    AcceptFormat(format): AcceptFormat,
) -> Result<Response, ApiError> {
    let names = state.database_names();
    let db_list = DatabaseList::from_names(names);

    match format {
        ContentFormat::Wave => Ok(FormatResponse::wave(db_list.to_wave()).into_response()),
        ContentFormat::Binary => {
            let (buffer, memory) = db_list
                .encode()
                .map_err(|e| ApiError::internal(e.to_string()))?;

            let mut bytes = buffer;
            bytes.extend(memory);

            Ok(FormatResponse::binary(bytes).into_response())
        }
    }
}
