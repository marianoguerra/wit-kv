//! Common test utilities and fixtures.
//!
//! This module provides shared constants, helper functions, and test fixtures
//! to reduce duplication across the test suite.

#![cfg(feature = "server")]

use axum::http::StatusCode;
use axum_test::TestServer;
use tempfile::TempDir;
use wit_kv::server::{router, AppState, Config, CorsConfig, DatabaseConfig, LoggingConfig, ServerConfig};

// =============================================================================
// WIT Type Definitions
// =============================================================================

/// Simple point type (u32 coordinates) for basic KV tests.
pub const POINT_WIT_U32: &str = r#"
    package test:types;

    interface types {
        record point {
            x: u32,
            y: u32,
        }
    }
"#;

/// Point type (s32 coordinates) for map/reduce tests.
pub const POINT_WIT_S32: &str = r#"
    package test:types;

    interface types {
        record point {
            x: s32,
            y: s32,
        }
    }
"#;

/// Counter type for list tests.
pub const COUNTER_WIT: &str = r#"
    package test:types;

    interface types {
        record counter {
            value: u64,
        }
    }
"#;

/// Person type for reduce tests.
pub const PERSON_WIT: &str = r#"
    package test:types;

    interface types {
        record person {
            age: u8,
            score: u32,
        }
    }
"#;

/// Full WIT for point-filter WASM module.
pub const POINT_FILTER_MODULE_WIT: &str = r#"
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

/// Full WIT for sum-scores WASM module.
pub const SUM_SCORES_MODULE_WIT: &str = r#"
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

// =============================================================================
// WASM Module Paths
// =============================================================================

/// Path to the pre-built point-filter WASM component.
pub const POINT_FILTER_WASM: &str =
    "examples/point-filter/target/wasm32-unknown-unknown/release/point_filter.wasm";

/// Path to the pre-built sum-scores WASM component.
pub const SUM_SCORES_WASM: &str =
    "examples/sum-scores/target/wasm32-unknown-unknown/release/sum_scores.wasm";

// =============================================================================
// Test Application
// =============================================================================

/// Test application wrapper that manages a temporary database.
pub struct TestApp {
    pub server: TestServer,
    _temp_dir: TempDir, // Keep alive for test duration
}

impl TestApp {
    /// Create a new test application with a fresh temporary database.
    pub fn new() -> anyhow::Result<Self> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("db");
        let config = Config {
            server: ServerConfig {
                bind: "127.0.0.1".into(),
                port: 0,
                static_path: None,
            },
            cors: CorsConfig::default(),
            logging: LoggingConfig::default(),
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

    /// Register a type for a keyspace using a WIT definition.
    pub async fn register_type(
        &self,
        keyspace: &str,
        type_name: &str,
        wit_def: &str,
    ) -> anyhow::Result<()> {
        self.server
            .put(&format!("/api/v1/db/test/types/{}", keyspace))
            .add_query_param("type_name", type_name)
            .content_type("text/plain")
            .text(wit_def)
            .await
            .assert_status_ok();
        Ok(())
    }

    /// Register the standard point type (u32 coordinates).
    pub async fn register_point_type(&self, keyspace: &str) -> anyhow::Result<()> {
        self.register_type(keyspace, "point", POINT_WIT_U32).await
    }

    /// Register the s32 point type for map/reduce operations.
    pub async fn register_point_type_s32(&self, keyspace: &str) -> anyhow::Result<()> {
        self.register_type(keyspace, "point", POINT_WIT_S32).await
    }

    /// Register the person type.
    pub async fn register_person_type(&self, keyspace: &str) -> anyhow::Result<()> {
        self.register_type(keyspace, "person", PERSON_WIT).await
    }

    /// Set a WAVE value in a keyspace.
    pub async fn set_value(
        &self,
        keyspace: &str,
        key: &str,
        wave_value: &str,
    ) -> anyhow::Result<()> {
        self.server
            .put(&format!("/api/v1/db/test/kv/{}/{}", keyspace, key))
            .content_type("application/x-wasm-wave")
            .text(wave_value)
            .await
            .assert_status(StatusCode::NO_CONTENT);
        Ok(())
    }

    /// Set multiple WAVE values in a keyspace.
    pub async fn set_values(
        &self,
        keyspace: &str,
        values: &[(&str, &str)],
    ) -> anyhow::Result<()> {
        for (key, value) in values {
            self.set_value(keyspace, key, value).await?;
        }
        Ok(())
    }
}

// =============================================================================
// Assertion Helpers
// =============================================================================

/// Assert that a WAVE key list contains all expected keys.
pub fn assert_keys_present(body: &str, expected_keys: &[&str]) {
    for key in expected_keys {
        assert!(
            body.contains(&format!("\"{}\"", key)),
            "Expected key '{}' not found in response: {}",
            key,
            body
        );
    }
}

/// Assert that a WAVE key list does not contain any of the excluded keys.
pub fn assert_keys_absent(body: &str, excluded_keys: &[&str]) {
    for key in excluded_keys {
        assert!(
            !body.contains(&format!("\"{}\"", key)),
            "Unexpected key '{}' found in response: {}",
            key,
            body
        );
    }
}

/// Check if a WASM module exists (for skipping tests).
pub fn wasm_module_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}
