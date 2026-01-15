//! API routes and handlers.

mod kv;
mod types;

use axum::{
    routing::{delete, get, put},
    Router,
};

use super::state::AppState;

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
        .route("/types/{keyspace}", delete(types::delete_type));

    Router::new()
        .route("/health", get(health))
        .nest("/api/v1/db/{database}", db_routes)
        .with_state(state)
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}
