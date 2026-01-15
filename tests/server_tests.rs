//! Integration tests for the wit-kv HTTP API server.
//!
//! These tests use axum-test to make requests against the router without starting a real server.

#![cfg(feature = "server")]

use axum::http::StatusCode;
use axum_test::multipart::{MultipartForm, Part};
use axum_test::TestServer;
use tempfile::TempDir;
use wit_kv::server::{router, AppState, Config, CorsConfig, DatabaseConfig, ServerConfig};

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
                static_path: None,
            },
            cors: CorsConfig::default(),
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

    // List all types (returns WAVE format: {keyspaces: [...]})
    let response = app.server.get("/api/v1/db/test/types").await;

    response.assert_status_ok();
    let body = response.text();
    // WAVE format: {keyspaces: [{name: "...", ...}, ...]}
    // Just verify both keyspaces are mentioned
    assert!(body.contains("points"));
    assert!(body.contains("counters"));

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

    // List all keys (returns WAVE format: {keys: ["a", "b", "c"]})
    let response = app.server.get("/api/v1/db/test/kv/points").await;

    response.assert_status_ok();
    let body = response.text();
    // WAVE format: {keys: ["a", "b", "c"]}
    assert!(body.contains("\"a\""));
    assert!(body.contains("\"b\""));
    assert!(body.contains("\"c\""));

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

    // List keys with prefix (returns WAVE format: {keys: ["user:1", "user:2"]})
    let response = app
        .server
        .get("/api/v1/db/test/kv/points")
        .add_query_param("prefix", "user:")
        .await;

    response.assert_status_ok();
    let body = response.text();
    // WAVE format: {keys: ["user:1", "user:2"]}
    assert!(body.contains("\"user:1\""));
    assert!(body.contains("\"user:2\""));
    assert!(!body.contains("\"admin:"));

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

    // List with limit (returns WAVE format: {keys: [...]})
    let response = app
        .server
        .get("/api/v1/db/test/kv/points")
        .add_query_param("limit", "5")
        .await;

    response.assert_status_ok();
    let body = response.text();
    // WAVE format: {keys: ["key0", "key1", ..., "key4"]}
    // Count the number of quoted strings in the keys array
    let key_count = body.matches("\"key").count();
    assert_eq!(key_count, 5);

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

// =============================================================================
// Map/Reduce Operations Tests
// =============================================================================

/// Path to the pre-built point-filter WASM component
const POINT_FILTER_WASM: &str = "examples/point-filter/target/wasm32-unknown-unknown/release/point_filter.wasm";

/// Path to the pre-built sum-scores WASM component
const SUM_SCORES_WASM: &str = "examples/sum-scores/target/wasm32-unknown-unknown/release/sum_scores.wasm";

/// WIT definition for point type (matches point-filter)
const POINT_WIT: &str = r#"
package wit-kv:typed-map@0.1.0;

interface types {
    record point {
        x: s32,
        y: s32,
    }
}

world typed-map-module {
    use types.{point};
    export filter: func(value: point) -> bool;
    export transform: func(value: point) -> point;
}
"#;

/// WIT definition for person/total types (matches sum-scores)
const PERSON_WIT: &str = r#"
package wit-kv:typed-sum-scores@0.1.0;

interface types {
    record person {
        age: u8,
        score: u32,
    }

    record total {
        sum: u64,
        count: u32,
    }
}

world typed-reduce-module {
    use types.{person, total};
    export init-state: func() -> total;
    export reduce: func(state: total, value: person) -> total;
}
"#;

#[tokio::test]
async fn test_map_operation() -> anyhow::Result<()> {
    // Skip test if WASM module not built
    if !std::path::Path::new(POINT_FILTER_WASM).exists() {
        eprintln!("Skipping test_map_operation: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;

    // Register the point type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: s32,
                y: s32,
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

    // Add test data: points at various distances from origin
    // point-filter keeps points within radius 100 and doubles coordinates
    let points = [
        ("p1", "{x: 10, y: 20}"),   // distance ~22, will be kept and doubled
        ("p2", "{x: 50, y: 50}"),   // distance ~70, will be kept and doubled
        ("p3", "{x: 150, y: 0}"),   // distance 150, will be filtered out
        ("p4", "{x: 3, y: 4}"),     // distance 5, will be kept and doubled
    ];

    for (key, value) in points {
        app.server
            .put(&format!("/api/v1/db/test/kv/points/{}", key))
            .content_type("application/x-wasm-wave")
            .text(value)
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    // Load the WASM module
    let wasm_bytes = std::fs::read(POINT_FILTER_WASM)?;

    // Create multipart form with module and config
    let config = serde_json::json!({
        "wit_definition": POINT_WIT,
        "input_type": "point",
        "output_type": "point"
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    // Execute map operation
    let response = app
        .server
        .post("/api/v1/db/test/map/points")
        .multipart(multipart)
        .await;

    response.assert_status_ok();

    let result: serde_json::Value = response.json();

    // Verify results
    assert_eq!(result["processed"].as_u64(), Some(4), "should process 4 keys");
    assert_eq!(result["transformed"].as_u64(), Some(3), "should transform 3 keys (p1, p2, p4)");
    assert_eq!(result["filtered"].as_u64(), Some(1), "should filter 1 key (p3)");

    // Check that results contain transformed values (coordinates doubled)
    let results = result["results"].as_array().expect("results should be array");
    assert_eq!(results.len(), 3, "should have 3 transformed results");

    // Verify p4 was doubled: {x: 3, y: 4} -> {x: 6, y: 8}
    let p4_result = results.iter().find(|r| r[0].as_str() == Some("p4"));
    assert!(p4_result.is_some(), "p4 should be in results");
    let p4_value = p4_result.unwrap()[1].as_str().unwrap();
    assert!(p4_value.contains("6") && p4_value.contains("8"), "p4 should be doubled to {{x: 6, y: 8}}");

    Ok(())
}

#[tokio::test]
async fn test_map_operation_with_filter() -> anyhow::Result<()> {
    // Skip test if WASM module not built
    if !std::path::Path::new(POINT_FILTER_WASM).exists() {
        eprintln!("Skipping test: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;

    // Register the point type
    let wit_def = r#"
        package test:types;

        interface types {
            record point {
                x: s32,
                y: s32,
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

    // Add test data with prefixed keys
    let points = [
        ("user:p1", "{x: 10, y: 20}"),
        ("user:p2", "{x: 50, y: 50}"),
        ("admin:p1", "{x: 3, y: 4}"),
    ];

    for (key, value) in points {
        app.server
            .put(&format!("/api/v1/db/test/kv/points/{}", key))
            .content_type("application/x-wasm-wave")
            .text(value)
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    // Load the WASM module
    let wasm_bytes = std::fs::read(POINT_FILTER_WASM)?;

    // Create config with prefix filter
    let config = serde_json::json!({
        "wit_definition": POINT_WIT,
        "input_type": "point",
        "output_type": "point",
        "filter": {
            "prefix": "user:"
        }
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    let response = app
        .server
        .post("/api/v1/db/test/map/points")
        .multipart(multipart)
        .await;

    response.assert_status_ok();

    let result: serde_json::Value = response.json();

    // Should only process keys with "user:" prefix
    assert_eq!(result["processed"].as_u64(), Some(2), "should process 2 user: keys");
    assert_eq!(result["transformed"].as_u64(), Some(2), "should transform 2 keys");

    Ok(())
}

#[tokio::test]
async fn test_reduce_operation() -> anyhow::Result<()> {
    // Skip test if WASM module not built
    if !std::path::Path::new(SUM_SCORES_WASM).exists() {
        eprintln!("Skipping test_reduce_operation: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;

    // Register the person type
    let wit_def = r#"
        package test:types;

        interface types {
            record person {
                age: u8,
                score: u32,
            }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/users")
        .add_query_param("type_name", "person")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Add test data
    let users = [
        ("alice", "{age: 30, score: 100}"),
        ("bob", "{age: 25, score: 85}"),
        ("charlie", "{age: 35, score: 120}"),
    ];

    for (key, value) in users {
        app.server
            .put(&format!("/api/v1/db/test/kv/users/{}", key))
            .content_type("application/x-wasm-wave")
            .text(value)
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    // Load the WASM module
    let wasm_bytes = std::fs::read(SUM_SCORES_WASM)?;

    // Create multipart form with module and config
    let config = serde_json::json!({
        "wit_definition": PERSON_WIT,
        "input_type": "person",
        "state_type": "total"
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    // Execute reduce operation
    let response = app
        .server
        .post("/api/v1/db/test/reduce/users")
        .multipart(multipart)
        .await;

    response.assert_status_ok();

    let result: serde_json::Value = response.json();

    // Verify results
    assert_eq!(result["processed"].as_u64(), Some(3), "should process 3 users");
    assert_eq!(result["error_count"].as_u64(), Some(0), "should have no errors");

    // Check final state: sum = 100 + 85 + 120 = 305, count = 3
    let state = result["state"].as_str().expect("state should be a string");
    assert!(state.contains("305"), "sum should be 305, got: {}", state);
    assert!(state.contains("3"), "count should be 3, got: {}", state);

    Ok(())
}

#[tokio::test]
async fn test_map_missing_module_field() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;
        interface types {
            record point { x: s32, y: s32, }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Send multipart without 'module' field
    let config = serde_json::json!({
        "wit_definition": "record point { x: s32, y: s32 }",
        "input_type": "point"
    });

    let multipart = MultipartForm::new()
        .add_part("config", Part::text(config.to_string()));

    let response = app
        .server
        .post("/api/v1/db/test/map/points")
        .multipart(multipart)
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json();
    assert_eq!(body["error"]["code"].as_str(), Some("MISSING_FIELD"));

    Ok(())
}

#[tokio::test]
async fn test_map_missing_config_field() -> anyhow::Result<()> {
    let app = TestApp::new()?;

    // Register a type
    let wit_def = r#"
        package test:types;
        interface types {
            record point { x: s32, y: s32, }
        }
    "#;

    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .content_type("text/plain")
        .text(wit_def)
        .await
        .assert_status_ok();

    // Send multipart without 'config' field
    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(vec![0u8; 10]).file_name("module.wasm"));

    let response = app
        .server
        .post("/api/v1/db/test/map/points")
        .multipart(multipart)
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json();
    assert_eq!(body["error"]["code"].as_str(), Some("MISSING_FIELD"));

    Ok(())
}

#[tokio::test]
async fn test_map_keyspace_not_found() -> anyhow::Result<()> {
    // Skip test if WASM module not built (we need a real component to get past the runner creation)
    if !std::path::Path::new(POINT_FILTER_WASM).exists() {
        eprintln!("Skipping test: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;

    // Don't register any type - keyspace doesn't exist
    // Use valid WASM and WIT so we get to the keyspace check
    let wasm_bytes = std::fs::read(POINT_FILTER_WASM)?;

    let config = serde_json::json!({
        "wit_definition": POINT_WIT,
        "input_type": "point"
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    let response = app
        .server
        .post("/api/v1/db/test/map/nonexistent")
        .multipart(multipart)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json();
    assert_eq!(body["error"]["code"].as_str(), Some("KEYSPACE_NOT_FOUND"));

    Ok(())
}
