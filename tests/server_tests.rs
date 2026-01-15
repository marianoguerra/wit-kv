//! Integration tests for the wit-kv HTTP API server.
//!
//! These tests use axum-test to make requests against the router without starting a real server.

#![cfg(feature = "server")]

use axum::http::StatusCode;
use axum_test::TestServer;
use tempfile::TempDir;
use wit_kv::server::{router, AppState, Config, DatabaseConfig, ServerConfig};

/// Test application wrapper that manages a temporary database.
struct TestApp {
    server: TestServer,
    _temp_dir: TempDir, // Keep alive for test duration
}

impl TestApp {
    fn new() -> anyhow::Result<Self> {
        let temp_dir = TempDir::new()?;
        // Use a subdirectory so it doesn't exist yet and will be initialized
        let db_path = temp_dir.path().join("db");
        let config = Config {
            server: ServerConfig {
                bind: "127.0.0.1".into(),
                port: 0,
            },
            databases: vec![DatabaseConfig {
                name: "test".into(),
                path: db_path.to_string_lossy().into(),
            }],
        };
        let state = AppState::from_config(&config)?;
        let server = TestServer::new(router(state))?;
        Ok(Self {
            server,
            _temp_dir: temp_dir,
        })
    }
}

// =============================================================================
// Health Check Tests
// =============================================================================

#[tokio::test]
async fn test_health_check() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    let response = app.server.get("/health").await;

    response.assert_status_ok();
    response.assert_text("ok");

    Ok(())
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_database_not_found() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    let response = app
        .server
        .get("/api/v1/db/nonexistent/kv/myspace/mykey")
        .await;

    response.assert_status(StatusCode::NOT_FOUND);

    // Verify it's a JSON error response
    let body: serde_json::Value = response.json();
    assert!(body.get("error").is_some());
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("DATABASE_NOT_FOUND")
    );

    Ok(())
}

#[tokio::test]
async fn test_keyspace_not_found_on_get() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Try to get a value from a keyspace that hasn't been registered
    let response = app
        .server
        .get("/api/v1/db/test/kv/unregistered/somekey")
        .await;

    response.assert_status(StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("KEYSPACE_NOT_FOUND")
    );

    Ok(())
}

// =============================================================================
// Type Operations Tests
// =============================================================================

#[tokio::test]
async fn test_register_and_get_type() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type for a keyspace
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    let response = app
        .server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await;

    response.assert_status_ok();

    // Verify we can get the type back
    let response = app.server.get("/api/v1/db/test/types/points").await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["name"].as_str(), Some("points"));
    assert_eq!(body["type_name"].as_str(), Some("point"));

    Ok(())
}

#[tokio::test]
async fn test_list_types() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register two types
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    let wit_def2 = r#"
        package test:types;

        interface types {
            record counter {
                value: u64,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/counters")
        .add_query_param("type_name", "counter")
        .content_type("text/plain")
        .text(wit_def2)
        .await
        .assert_status_ok();

    // List all types
    let response = app.server.get("/api/v1/db/test/types").await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let types = body.as_array().expect("expected array");
    assert_eq!(types.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_delete_type() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Delete it
    let response = app.server.delete("/api/v1/db/test/types/points").await;
    response.assert_status(StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = app.server.get("/api/v1/db/test/types/points").await;
    response.assert_status(StatusCode::NOT_FOUND);

    Ok(())
}

// =============================================================================
// Key-Value Operations Tests
// =============================================================================

#[tokio::test]
async fn test_set_and_get_value() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // First, register a type for the keyspace
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Set a value
    let response = app
        .server
        .put("/api/v1/db/test/kv/points/origin")
        .content_type("application/x-wasm-wave")
        .text("{x: 0, y: 0}")
        .await;

    response.assert_status(StatusCode::NO_CONTENT);

    // Get the value back
    let response = app.server.get("/api/v1/db/test/kv/points/origin").await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("x:") || body.contains("x :"));
    assert!(body.contains("y:") || body.contains("y :"));

    Ok(())
}

#[tokio::test]
async fn test_list_keys() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Add some values
    app.server
        .put("/api/v1/db/test/kv/points/a")
        .content_type("application/x-wasm-wave")
        .text("{x: 1, y: 1}")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    app.server
        .put("/api/v1/db/test/kv/points/b")
        .content_type("application/x-wasm-wave")
        .text("{x: 2, y: 2}")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    app.server
        .put("/api/v1/db/test/kv/points/c")
        .content_type("application/x-wasm-wave")
        .text("{x: 3, y: 3}")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // List all keys
    let response = app.server.get("/api/v1/db/test/kv/points").await;

    response.assert_status_ok();
    let keys: Vec<String> = response.json();
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"a".to_string()));
    assert!(keys.contains(&"b".to_string()));
    assert!(keys.contains(&"c".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_list_keys_with_prefix() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Add some values with different prefixes
    for key in ["user:1", "user:2", "admin:1", "admin:2"] {
        app.server
            .put(&format!("/api/v1/db/test/kv/points/{}", key))
            .content_type("application/x-wasm-wave")
            .text("{x: 0, y: 0}")
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    // List keys with prefix
    let response = app
        .server
        .get("/api/v1/db/test/kv/points")
        .add_query_param("prefix", "user:")
        .await;

    response.assert_status_ok();
    let keys: Vec<String> = response.json();
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().all(|k| k.starts_with("user:")));

    Ok(())
}

#[tokio::test]
async fn test_list_keys_with_limit() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Add several values
    for i in 0..10 {
        app.server
            .put(&format!("/api/v1/db/test/kv/points/key{}", i))
            .content_type("application/x-wasm-wave")
            .text("{x: 0, y: 0}")
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    // List with limit
    let response = app
        .server
        .get("/api/v1/db/test/kv/points")
        .add_query_param("limit", "5")
        .await;

    response.assert_status_ok();
    let keys: Vec<String> = response.json();
    assert_eq!(keys.len(), 5);

    Ok(())
}

#[tokio::test]
async fn test_delete_key() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Set a value
    app.server
        .put("/api/v1/db/test/kv/points/to_delete")
        .content_type("application/x-wasm-wave")
        .text("{x: 0, y: 0}")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // Delete it
    let response = app
        .server
        .delete("/api/v1/db/test/kv/points/to_delete")
        .await;
    response.assert_status(StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = app
        .server
        .get("/api/v1/db/test/kv/points/to_delete")
        .await;
    response.assert_status(StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn test_key_not_found() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: u32,
                y: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Try to get a non-existent key
    let response = app
        .server
        .get("/api/v1/db/test/kv/points/nonexistent")
        .await;

    response.assert_status(StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json();
    assert_eq!(body["error"]["code"].as_str(), Some("KEY_NOT_FOUND"));

    Ok(())
}
