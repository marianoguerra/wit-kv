//! In-memory user store example for wit-http.
//!
//! Demonstrates mounting a WIT-typed resource as RESTful endpoints on axum.
//!
//! Run:
//!     cargo run --example user_store -p wit-http
//!
//! Then test with curl:
//!     # Create a user
//!     curl -X PUT http://localhost:3000/api/v1/users/alice \
//!       -H 'Content-Type: application/x-wasm-wave' \
//!       -d '{name: "Alice", email: "alice@example.com", age: 30}'
//!
//!     # Get a user (WAVE text)
//!     curl http://localhost:3000/api/v1/users/alice
//!
//!     # Get a user (binary)
//!     curl http://localhost:3000/api/v1/users/alice \
//!       -H 'Accept: application/octet-stream' --output -
//!
//!     # List all users
//!     curl http://localhost:3000/api/v1/users
//!
//!     # Delete a user
//!     curl -X DELETE http://localhost:3000/api/v1/users/alice

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Mutex;

use axum::Router;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use wit_http::{
    CollectionQuery, MountConfig, ResourceError, WitResource, WitType, mount_resource,
};

// ---------------------------------------------------------------------------
// WIT type: User
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    name: String,
    email: String,
    age: u32,
}

impl WitType for User {
    fn wit_definition() -> &'static str {
        r#"
        package example:types;
        interface types {
            record user {
                name: string,
                email: string,
                age: u32,
            }
        }
        "#
    }

    fn type_name() -> &'static str {
        "user"
    }
}

// ---------------------------------------------------------------------------
// Resource: UserStore
// ---------------------------------------------------------------------------

struct UserStore {
    data: Mutex<HashMap<String, User>>,
}

impl UserStore {
    fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
}

impl WitResource for UserStore {
    type Item = User;

    async fn get(&self, id: &str) -> Result<User, ResourceError> {
        let guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .get(id)
            .cloned()
            .ok_or_else(|| ResourceError::NotFound(id.to_string()))
    }

    async fn set(&self, id: &str, item: User) -> Result<(), ResourceError> {
        let mut guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(id.to_string(), item);
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), ResourceError> {
        let mut guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .remove(id)
            .ok_or_else(|| ResourceError::NotFound(id.to_string()))?;
        Ok(())
    }

    async fn list(&self, query: CollectionQuery) -> Result<Vec<User>, ResourceError> {
        let guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let mut users: Vec<User> = guard.values().cloned().collect();

        // Sort by name for deterministic output
        users.sort_by(|a, b| a.name.cmp(&b.name));

        if let Some(offset) = query.offset {
            users = users.into_iter().skip(offset).collect();
        }
        if let Some(limit) = query.limit {
            users.truncate(limit);
        }

        Ok(users)
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    let app = Router::new().merge(mount_resource(
        "/api/v1/users",
        UserStore::new(),
        MountConfig::crud(),
    ));

    let addr = format!("127.0.0.1:{port}");
    tracing::info!("Listening on {addr}");
    let listener = TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
