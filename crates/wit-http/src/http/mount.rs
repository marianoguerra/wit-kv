//! Resource mounting: turn a `WitResource` into an axum `Router`.

use std::sync::Arc;

use axum::Router;
use axum::routing::{MethodRouter, get};

use crate::core::resource::WitResource;
use crate::http::handlers;

/// Configuration for which endpoints to generate when mounting a resource.
#[derive(Debug, Clone)]
pub struct MountConfig {
    /// Enable `GET /{id}` — single item retrieval.
    pub get: bool,
    /// Enable `PUT /{id}` — create or update.
    pub set: bool,
    /// Enable `DELETE /{id}` — delete.
    pub delete: bool,
    /// Enable `GET /` — collection listing.
    pub list: bool,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self::crud()
    }
}

impl MountConfig {
    /// All CRUD operations enabled (default).
    pub fn crud() -> Self {
        Self {
            get: true,
            set: true,
            delete: true,
            list: true,
        }
    }

    /// Read-only: get + list only.
    pub fn read_only() -> Self {
        Self {
            get: true,
            set: false,
            delete: false,
            list: true,
        }
    }
}

/// Mount a `WitResource` as RESTful endpoints under `path`.
///
/// Returns a `Router` with the configured endpoints. Merge it into your
/// application router with `Router::merge()` or nest it further.
///
/// # Generated endpoints
///
/// For `mount_resource("/users", store, MountConfig::crud())`:
///
/// | Method | Path          | Handler  | Status |
/// |--------|---------------|----------|--------|
/// | GET    | `/users`      | `list`   | 200    |
/// | GET    | `/users/{id}` | `get`    | 200    |
/// | PUT    | `/users/{id}` | `set`    | 204    |
/// | DELETE | `/users/{id}` | `delete` | 204    |
///
/// # Example
///
/// ```ignore
/// let app = Router::new()
///     .merge(mount_resource("/api/v1/users", user_store, MountConfig::crud()));
/// ```
pub fn mount_resource<R: WitResource>(path: &str, resource: R, config: MountConfig) -> Router {
    let state = Arc::new(resource);

    let mut inner = Router::<Arc<R>>::new();

    if config.list {
        inner = inner.route("/", get(handlers::handle_list::<R>));
    }

    if config.get || config.set || config.delete {
        let mut item_routes = MethodRouter::<Arc<R>>::new();
        if config.get {
            item_routes = item_routes.get(handlers::handle_get::<R>);
        }
        if config.set {
            item_routes = item_routes.put(handlers::handle_set::<R>);
        }
        if config.delete {
            item_routes = item_routes.delete(handlers::handle_delete::<R>);
        }
        inner = inner.route("/{id}", item_routes);
    }

    Router::new().nest(path, inner.with_state(state))
}
