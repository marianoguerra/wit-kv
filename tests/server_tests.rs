//! Integration tests for the wit-kv HTTP API server.
//!
//! These tests use axum-test to make requests against the router without starting a real server.

#![cfg(feature = "server")]

mod common;

use axum::http::StatusCode;
use axum_test::multipart::{MultipartForm, Part};
use common::{
    assert_keys_absent, assert_keys_present, wasm_module_exists, TestApp,
    POINT_FILTER_MODULE_WIT, POINT_FILTER_WASM, POINT_WIT_S32, POINT_WIT_U32,
    SUM_SCORES_MODULE_WIT, SUM_SCORES_WASM,
};
use urlencoding;

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
    app.register_point_type("points").await?;

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
    app.register_point_type("points").await?;
    app.register_type("counters", "counter", common::COUNTER_WIT).await?;

    let response = app.server.get("/api/v1/db/test/types").await;
    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("points"));
    assert!(body.contains("counters"));

    Ok(())
}

#[tokio::test]
async fn test_delete_type() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    let response = app.server.delete("/api/v1/db/test/types/points").await;
    response.assert_status(StatusCode::NO_CONTENT);

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
    app.register_point_type("points").await?;
    app.set_value("points", "origin", "{x: 0, y: 0}").await?;

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
    app.register_point_type("points").await?;
    app.set_values("points", &[
        ("a", "{x: 1, y: 1}"),
        ("b", "{x: 2, y: 2}"),
        ("c", "{x: 3, y: 3}"),
    ]).await?;

    let response = app.server.get("/api/v1/db/test/kv/points").await;
    response.assert_status_ok();
    let body = response.text();
    assert_keys_present(&body, &["a", "b", "c"]);

    Ok(())
}

#[tokio::test]
async fn test_list_keys_with_prefix() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;
    app.set_values("points", &[
        ("user:1", "{x: 0, y: 0}"),
        ("user:2", "{x: 0, y: 0}"),
        ("admin:1", "{x: 0, y: 0}"),
        ("admin:2", "{x: 0, y: 0}"),
    ]).await?;

    let response = app.server
        .get("/api/v1/db/test/kv/points")
        .add_query_param("prefix", "user:")
        .await;

    response.assert_status_ok();
    let body = response.text();
    assert_keys_present(&body, &["user:1", "user:2"]);
    assert_keys_absent(&body, &["admin:1", "admin:2"]);

    Ok(())
}

#[tokio::test]
async fn test_list_keys_with_limit() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    // Add several values
    for i in 0..10 {
        app.set_value("points", &format!("key{}", i), "{x: 0, y: 0}").await?;
    }

    let response = app.server
        .get("/api/v1/db/test/kv/points")
        .add_query_param("limit", "5")
        .await;

    response.assert_status_ok();
    let body = response.text();
    let key_count = body.matches("\"key").count();
    assert_eq!(key_count, 5);

    Ok(())
}

#[tokio::test]
async fn test_delete_key() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;
    app.set_value("points", "to_delete", "{x: 0, y: 0}").await?;

    let response = app.server.delete("/api/v1/db/test/kv/points/to_delete").await;
    response.assert_status(StatusCode::NO_CONTENT);

    let response = app.server.get("/api/v1/db/test/kv/points/to_delete").await;
    response.assert_status(StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn test_key_not_found() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    let response = app.server.get("/api/v1/db/test/kv/points/nonexistent").await;
    response.assert_status(StatusCode::NOT_FOUND);

    let body: serde_json::Value = response.json();
    assert_eq!(body["error"]["code"].as_str(), Some("KEY_NOT_FOUND"));

    Ok(())
}

// =============================================================================
// Map/Reduce Operations Tests
// =============================================================================

#[tokio::test]
async fn test_map_operation() -> anyhow::Result<()> {
    if !wasm_module_exists(POINT_FILTER_WASM) {
        eprintln!("Skipping test_map_operation: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;
    app.register_point_type_s32("points").await?;

    // Add test data: points at various distances from origin
    // point-filter keeps points within radius 100 and doubles coordinates
    app.set_values("points", &[
        ("p1", "{x: 10, y: 20}"),   // distance ~22, will be kept and doubled
        ("p2", "{x: 50, y: 50}"),   // distance ~70, will be kept and doubled
        ("p3", "{x: 150, y: 0}"),   // distance 150, will be filtered out
        ("p4", "{x: 3, y: 4}"),     // distance 5, will be kept and doubled
    ]).await?;

    let wasm_bytes = std::fs::read(POINT_FILTER_WASM)?;
    let config = serde_json::json!({
        "wit_definition": POINT_FILTER_MODULE_WIT,
        "input_type": "point",
        "output_type": "point"
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    let response = app.server
        .post("/api/v1/db/test/map/points")
        .multipart(multipart)
        .await;

    response.assert_status_ok();
    let result: serde_json::Value = response.json();

    assert_eq!(result["processed"].as_u64(), Some(4), "should process 4 keys");
    assert_eq!(result["transformed"].as_u64(), Some(3), "should transform 3 keys (p1, p2, p4)");
    assert_eq!(result["filtered"].as_u64(), Some(1), "should filter 1 key (p3)");

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
    if !wasm_module_exists(POINT_FILTER_WASM) {
        eprintln!("Skipping test: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;
    app.register_point_type_s32("points").await?;
    app.set_values("points", &[
        ("user:p1", "{x: 10, y: 20}"),
        ("user:p2", "{x: 50, y: 50}"),
        ("admin:p1", "{x: 3, y: 4}"),
    ]).await?;

    let wasm_bytes = std::fs::read(POINT_FILTER_WASM)?;
    let config = serde_json::json!({
        "wit_definition": POINT_FILTER_MODULE_WIT,
        "input_type": "point",
        "output_type": "point",
        "filter": { "prefix": "user:" }
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    let response = app.server
        .post("/api/v1/db/test/map/points")
        .multipart(multipart)
        .await;

    response.assert_status_ok();
    let result: serde_json::Value = response.json();

    assert_eq!(result["processed"].as_u64(), Some(2), "should process 2 user: keys");
    assert_eq!(result["transformed"].as_u64(), Some(2), "should transform 2 keys");

    Ok(())
}

#[tokio::test]
async fn test_reduce_operation() -> anyhow::Result<()> {
    if !wasm_module_exists(SUM_SCORES_WASM) {
        eprintln!("Skipping test_reduce_operation: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;
    app.register_person_type("users").await?;
    app.set_values("users", &[
        ("alice", "{age: 30, score: 100}"),
        ("bob", "{age: 25, score: 85}"),
        ("charlie", "{age: 35, score: 120}"),
    ]).await?;

    let wasm_bytes = std::fs::read(SUM_SCORES_WASM)?;
    let config = serde_json::json!({
        "wit_definition": SUM_SCORES_MODULE_WIT,
        "input_type": "person",
        "state_type": "total"
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    let response = app.server
        .post("/api/v1/db/test/reduce/users")
        .multipart(multipart)
        .await;

    response.assert_status_ok();
    let result: serde_json::Value = response.json();

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
    app.register_point_type_s32("points").await?;

    let config = serde_json::json!({
        "wit_definition": POINT_WIT_S32,
        "input_type": "point"
    });

    let multipart = MultipartForm::new()
        .add_part("config", Part::text(config.to_string()));

    let response = app.server
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
    app.register_point_type_s32("points").await?;

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(vec![0u8; 10]).file_name("module.wasm"));

    let response = app.server
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
    if !wasm_module_exists(POINT_FILTER_WASM) {
        eprintln!("Skipping test: WASM module not built. Run 'just build-examples' first.");
        return Ok(());
    }

    let app = TestApp::new()?;
    // Don't register any type - keyspace doesn't exist

    let wasm_bytes = std::fs::read(POINT_FILTER_WASM)?;
    let config = serde_json::json!({
        "wit_definition": POINT_FILTER_MODULE_WIT,
        "input_type": "point"
    });

    let multipart = MultipartForm::new()
        .add_part("module", Part::bytes(wasm_bytes).file_name("module.wasm"))
        .add_part("config", Part::text(config.to_string()));

    let response = app.server
        .post("/api/v1/db/test/map/nonexistent")
        .multipart(multipart)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
    let body: serde_json::Value = response.json();
    assert_eq!(body["error"]["code"].as_str(), Some("KEYSPACE_NOT_FOUND"));

    Ok(())
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_empty_keyspace_list() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("empty").await?;

    // List should return empty array, not error
    let response = app.server.get("/api/v1/db/test/kv/empty").await;
    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("{keys: []}"), "Empty keyspace should return empty key list, got: {}", body);

    Ok(())
}

#[tokio::test]
async fn test_unicode_keys() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    // Test various unicode keys
    let unicode_keys = [
        ("hello_世界", "{x: 1, y: 1}"),
        ("مرحبا", "{x: 2, y: 2}"),
        ("キー", "{x: 3, y: 3}"),
    ];

    for (key, value) in unicode_keys {
        app.set_value("points", key, value).await?;
    }

    // Verify we can retrieve them
    for (key, _) in unicode_keys {
        let response = app.server
            .get(&format!("/api/v1/db/test/kv/points/{}", urlencoding::encode(key)))
            .await;
        response.assert_status_ok();
    }

    // List should include all keys
    let response = app.server.get("/api/v1/db/test/kv/points").await;
    response.assert_status_ok();

    Ok(())
}

#[tokio::test]
async fn test_special_character_keys() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    // Test keys with special characters
    let special_keys = [
        ("key-with-dashes", "{x: 1, y: 1}"),
        ("key_with_underscores", "{x: 2, y: 2}"),
        ("key.with.dots", "{x: 3, y: 3}"),
        ("key:with:colons", "{x: 4, y: 4}"),
    ];

    for (key, value) in special_keys {
        app.set_value("points", key, value).await?;
    }

    // Verify we can retrieve them
    for (key, _) in special_keys {
        let response = app.server
            .get(&format!("/api/v1/db/test/kv/points/{}", urlencoding::encode(key)))
            .await;
        response.assert_status_ok();
    }

    Ok(())
}

#[tokio::test]
async fn test_overwrite_existing_key() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    // Set initial value
    app.set_value("points", "key1", "{x: 1, y: 1}").await?;

    // Overwrite with new value
    app.set_value("points", "key1", "{x: 100, y: 200}").await?;

    // Verify the new value
    let response = app.server.get("/api/v1/db/test/kv/points/key1").await;
    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("100"), "Should contain updated x value, got: {}", body);
    assert!(body.contains("200"), "Should contain updated y value, got: {}", body);

    Ok(())
}

#[tokio::test]
async fn test_type_re_registration_with_force() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    // Re-registration without force should work if it's the same type (idempotent)
    // But with force, we can definitely re-register
    app.server
        .put("/api/v1/db/test/types/points")
        .add_query_param("type_name", "point")
        .add_query_param("force", "true")
        .content_type("text/plain")
        .text(POINT_WIT_U32)
        .await
        .assert_status_ok();

    // Verify type still works
    let response = app.server.get("/api/v1/db/test/types/points").await;
    response.assert_status_ok();

    Ok(())
}

#[tokio::test]
async fn test_list_with_start_and_end_range() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    app.register_point_type("points").await?;

    // Add ordered keys
    app.set_values("points", &[
        ("a", "{x: 1, y: 1}"),
        ("b", "{x: 2, y: 2}"),
        ("c", "{x: 3, y: 3}"),
        ("d", "{x: 4, y: 4}"),
        ("e", "{x: 5, y: 5}"),
    ]).await?;

    // Query range [b, d) - should return b, c
    let response = app.server
        .get("/api/v1/db/test/kv/points")
        .add_query_param("start", "b")
        .add_query_param("end", "d")
        .await;

    response.assert_status_ok();
    let body = response.text();
    assert_keys_present(&body, &["b", "c"]);
    assert_keys_absent(&body, &["a", "d", "e"]);

    Ok(())
}
