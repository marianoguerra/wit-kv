//! Integration tests for wit-http using axum test utilities.
//!
//! These tests exercise the full request/response cycle without starting
//! a server, using `tower::ServiceExt::oneshot` to send requests directly
//! to the router.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Mutex;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use wit_http::{
    CollectionQuery, ContentFormat, MountConfig, ResourceError, WitResource, WitType,
    mount_resource,
};

// ---------------------------------------------------------------------------
// Test fixtures
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
        package test:types;
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

struct UserStore {
    data: Mutex<HashMap<String, User>>,
}

impl UserStore {
    fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

    fn with_users(users: Vec<(&str, User)>) -> Self {
        let mut map = HashMap::new();
        for (id, user) in users {
            map.insert(id.to_string(), user);
        }
        Self {
            data: Mutex::new(map),
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

fn test_app() -> Router {
    mount_resource("/users", UserStore::new(), MountConfig::crud())
}

fn test_app_with_data() -> Router {
    let store = UserStore::with_users(vec![
        (
            "alice",
            User {
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                age: 30,
            },
        ),
        (
            "bob",
            User {
                name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
                age: 25,
            },
        ),
    ]);
    mount_resource("/users", store, MountConfig::crud())
}

async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

// ---------------------------------------------------------------------------
// PUT (set)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_creates_user() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/users/alice")
                .header(header::CONTENT_TYPE, "application/x-wasm-wave")
                .body(Body::from(
                    r#"{name: "Alice", email: "alice@example.com", age: 30}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn put_then_get_roundtrip() {
    let app = test_app();

    // PUT
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/users/alice")
                .header(header::CONTENT_TYPE, "application/x-wasm-wave")
                .body(Body::from(
                    r#"{name: "Alice", email: "alice@example.com", age: 30}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // GET
    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/alice")
                .header(header::ACCEPT, "application/x-wasm-wave")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-wasm-wave"
    );

    let body = body_string(response).await;
    assert!(body.contains("Alice"));
    assert!(body.contains("alice@example.com"));
    assert!(body.contains("30"));
}

// ---------------------------------------------------------------------------
// GET
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_existing_user_wave() {
    let app = test_app_with_data();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/alice")
                .header(header::ACCEPT, "application/x-wasm-wave")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    assert!(body.contains("Alice"));
    assert!(body.contains("alice@example.com"));
    assert!(body.contains("30"));
}

#[tokio::test]
async fn get_existing_user_binary() {
    let app = test_app_with_data();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/alice")
                .header(header::ACCEPT, "application/octet-stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/octet-stream"
    );

    // Binary response should be non-empty
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert!(!bytes.is_empty());
}

#[tokio::test]
async fn get_nonexistent_returns_404() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/nobody")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = body_string(response).await;
    assert!(body.contains("NOT_FOUND"));
}

#[tokio::test]
async fn get_default_format_is_wave() {
    let app = test_app_with_data();

    // No Accept header — should default to Wave
    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/x-wasm-wave"
    );
}

// ---------------------------------------------------------------------------
// DELETE
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_existing_user() {
    let app = test_app_with_data();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/users/alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_nonexistent_returns_404() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/users/nobody")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// LIST
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_returns_all_users() {
    let app = test_app_with_data();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users")
                .header(header::ACCEPT, "application/x-wasm-wave")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    // Should contain both users in a WAVE list
    assert!(body.starts_with('['));
    assert!(body.ends_with(']'));
    assert!(body.contains("Alice"));
    assert!(body.contains("Bob"));
}

#[tokio::test]
async fn list_empty_collection() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn list_with_limit() {
    let app = test_app_with_data();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users?limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    // Sorted by name: Alice comes first, Bob should be excluded
    assert!(body.contains("Alice"));
    assert!(!body.contains("Bob"));
}

#[tokio::test]
async fn list_with_offset() {
    let app = test_app_with_data();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users?offset=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    // Sorted by name: Alice skipped, only Bob
    assert!(!body.contains("Alice"));
    assert!(body.contains("Bob"));
}

// ---------------------------------------------------------------------------
// Content negotiation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_with_unsupported_content_type_returns_415() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/users/alice")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name": "Alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn put_with_invalid_wave_returns_400() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/users/alice")
                .header(header::CONTENT_TYPE, "application/x-wasm-wave")
                .body(Body::from("not valid wave {{{"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn binary_roundtrip() {
    let app = test_app();

    // PUT as wave
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/users/alice")
                .header(header::CONTENT_TYPE, "application/x-wasm-wave")
                .body(Body::from(
                    r#"{name: "Alice", email: "alice@example.com", age: 30}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // GET as binary
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/users/alice")
                .header(header::ACCEPT, "application/octet-stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let binary_body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();

    // PUT the binary back as a new user
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/users/alice2")
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(binary_body.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // GET the copy as wave and verify
    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/alice2")
                .header(header::ACCEPT, "application/x-wasm-wave")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    assert!(body.contains("Alice"));
    assert!(body.contains("alice@example.com"));
    assert!(body.contains("30"));
}

// ---------------------------------------------------------------------------
// MountConfig variants
// ---------------------------------------------------------------------------

#[tokio::test]
async fn read_only_rejects_put_and_delete() {
    let store = UserStore::with_users(vec![(
        "alice",
        User {
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            age: 30,
        },
    )]);
    let app = mount_resource("/users", store, MountConfig::read_only());

    // GET should work
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/users/alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // LIST should work
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // PUT should be rejected (405 Method Not Allowed)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/users/alice")
                .header(header::CONTENT_TYPE, "application/x-wasm-wave")
                .body(Body::from(
                    r#"{name: "Alice", email: "a@b.com", age: 31}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);

    // DELETE should be rejected (405 Method Not Allowed)
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/users/alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// ---------------------------------------------------------------------------
// Serde round-trip tests
// ---------------------------------------------------------------------------

#[test]
fn serde_wave_roundtrip() {
    use wit_http::content::format::{decode_request, encode_response};

    let user = User {
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        age: 25,
    };

    // Wave roundtrip
    let wave_bytes = encode_response(ContentFormat::Wave, &user).unwrap();
    let decoded: User = decode_request(ContentFormat::Wave, &wave_bytes).unwrap();
    assert_eq!(decoded.name, "Bob");
    assert_eq!(decoded.email, "bob@example.com");
    assert_eq!(decoded.age, 25);

    // Binary roundtrip
    let binary_bytes = encode_response(ContentFormat::Binary, &user).unwrap();
    let decoded: User = decode_request(ContentFormat::Binary, &binary_bytes).unwrap();
    assert_eq!(decoded.name, "Bob");
    assert_eq!(decoded.email, "bob@example.com");
    assert_eq!(decoded.age, 25);
}

#[test]
fn wit_type_resolved_type_is_cached() {
    let r1 = User::resolved_type().unwrap();
    let r2 = User::resolved_type().unwrap();
    assert!(std::sync::Arc::ptr_eq(&r1, &r2));
}
