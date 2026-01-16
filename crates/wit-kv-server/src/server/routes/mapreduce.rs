//! Map/reduce operation handlers.

use axum::{
    Json,
    extract::{Multipart, Path, State},
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};

use wit_kv::kv::KvStore;
use wit_kv::wasm::TypedRunner;

use super::super::{error::ApiError, state::AppState};

/// JSON config for map operation (sent in multipart 'config' field).
#[derive(Debug, Deserialize)]
pub struct MapConfig {
    /// WIT definition text for the module's types
    pub wit_definition: String,
    /// Name of the input type in the WIT definition
    pub input_type: String,
    /// Name of the output type (defaults to input_type if not specified)
    pub output_type: Option<String>,
    /// Optional key filters
    #[serde(default)]
    pub filter: KeyFilter,
}

/// JSON config for reduce operation (sent in multipart 'config' field).
#[derive(Debug, Deserialize)]
pub struct ReduceConfig {
    /// WIT definition text for the module's types
    pub wit_definition: String,
    /// Name of the input/value type in the WIT definition
    pub input_type: String,
    /// Name of the state type in the WIT definition
    pub state_type: String,
    /// Optional key filters
    #[serde(default)]
    pub filter: KeyFilter,
}

/// Key filter options.
#[derive(Debug, Deserialize, Default)]
pub struct KeyFilter {
    /// Single key to process (if set, other filters are ignored)
    pub key: Option<String>,
    /// Prefix filter for keys
    pub prefix: Option<String>,
    /// Start key (inclusive)
    pub start: Option<String>,
    /// End key (exclusive)
    pub end: Option<String>,
    /// Maximum number of keys to process
    pub limit: Option<usize>,
}

/// Result of a map operation.
#[derive(Debug, Serialize)]
pub struct MapResult {
    /// Number of keys processed
    pub processed: u32,
    /// Number of keys that passed the filter and were transformed
    pub transformed: u32,
    /// Number of keys filtered out
    pub filtered: u32,
    /// Errors encountered: list of (key, error message)
    pub errors: Vec<(String, String)>,
    /// Transformed results: list of (key, wave-encoded value)
    pub results: Vec<(String, String)>,
}

/// Result of a reduce operation.
#[derive(Debug, Serialize)]
pub struct ReduceResult {
    /// Number of values processed
    pub processed: u32,
    /// Number of errors encountered
    pub error_count: u32,
    /// Errors encountered: list of (key, error message)
    pub errors: Vec<(String, String)>,
    /// Final state as wave-encoded value
    pub state: String,
}

/// Extract module bytes and config from multipart request for map operation.
async fn extract_map_multipart(
    multipart: &mut Multipart,
) -> Result<(Vec<u8>, MapConfig), ApiError> {
    let mut module_bytes: Option<Vec<u8>> = None;
    let mut config: Option<MapConfig> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::invalid_multipart(e.to_string()))?
    {
        let name = field.name().map(|s| s.to_string());

        match name.as_deref() {
            Some("module") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::invalid_multipart(e.to_string()))?;
                module_bytes = Some(bytes.to_vec());
            }
            Some("config") => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::invalid_multipart(e.to_string()))?;
                config = Some(serde_json::from_str(&text).map_err(|e| {
                    ApiError::invalid_multipart(format!("Invalid config JSON: {}", e))
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    let module_bytes = module_bytes.ok_or_else(|| ApiError::missing_field("module"))?;
    let config = config.ok_or_else(|| ApiError::missing_field("config"))?;

    Ok((module_bytes, config))
}

/// Extract module bytes and config from multipart request for reduce operation.
async fn extract_reduce_multipart(
    multipart: &mut Multipart,
) -> Result<(Vec<u8>, ReduceConfig), ApiError> {
    let mut module_bytes: Option<Vec<u8>> = None;
    let mut config: Option<ReduceConfig> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::invalid_multipart(e.to_string()))?
    {
        let name = field.name().map(|s| s.to_string());

        match name.as_deref() {
            Some("module") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::invalid_multipart(e.to_string()))?;
                module_bytes = Some(bytes.to_vec());
            }
            Some("config") => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::invalid_multipart(e.to_string()))?;
                config = Some(serde_json::from_str(&text).map_err(|e| {
                    ApiError::invalid_multipart(format!("Invalid config JSON: {}", e))
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    let module_bytes = module_bytes.ok_or_else(|| ApiError::missing_field("module"))?;
    let config = config.ok_or_else(|| ApiError::missing_field("config"))?;

    Ok((module_bytes, config))
}

/// Get filtered keys from the store based on the filter options.
fn get_filtered_keys(
    store: &KvStore,
    keyspace: &str,
    filter: &KeyFilter,
) -> Result<Vec<String>, ApiError> {
    if let Some(key) = &filter.key {
        // Single key mode
        Ok(vec![key.clone()])
    } else {
        // Range/prefix query
        store
            .list(
                keyspace,
                filter.prefix.as_deref(),
                filter.start.as_deref(),
                filter.end.as_deref(),
                filter.limit,
            )
            .map_err(ApiError::from)
    }
}

/// Execute a map operation.
///
/// Expects a multipart/form-data request with:
/// - `module`: WASM component bytes
/// - `config`: JSON with MapConfig
#[instrument(skip(state, multipart), fields(database = %database, keyspace = %keyspace))]
pub async fn map_operation(
    State(state): State<AppState>,
    Path((database, keyspace)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Json<MapResult>, ApiError> {
    debug!("starting map operation");

    // Extract module bytes and config from multipart
    let (module_bytes, config) = extract_map_multipart(&mut multipart).await?;

    debug!(
        module_size = module_bytes.len(),
        input_type = %config.input_type,
        output_type = config.output_type.as_deref(),
        filter.key = config.filter.key.as_deref(),
        filter.prefix = config.filter.prefix.as_deref(),
        filter.limit = config.filter.limit,
        "map config extracted"
    );

    let store = state.get_database(&database)?;

    // Create TypedRunner from bytes
    let output_type = config.output_type.as_deref().unwrap_or(&config.input_type);
    let mut runner = TypedRunner::builder()
        .component_bytes(module_bytes)
        .wit_text(&config.wit_definition)
        .input_type(&config.input_type)
        .output_type(output_type)
        .build()
        .map_err(ApiError::from)?;

    // Get keyspace metadata for type version
    let metadata = store
        .get_type(&keyspace)?
        .ok_or_else(|| ApiError::keyspace_not_found(&database, &keyspace))?;

    // Get keys based on filter
    let keys = get_filtered_keys(store, &keyspace, &config.filter)?;

    // Execute map operation
    let mut processed: u32 = 0;
    let mut transformed: u32 = 0;
    let mut filtered: u32 = 0;
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut results: Vec<(String, String)> = Vec::new();

    for key in keys {
        match store.get_raw(&keyspace, &key)? {
            Some(stored) => {
                // Call filter
                match runner.call_filter(&stored) {
                    Ok(true) => {
                        // Call transform
                        match runner.call_transform(&stored, metadata.type_version) {
                            Ok(result) => {
                                match runner.stored_to_wave_string(&result) {
                                    Ok(wave_str) => results.push((key.clone(), wave_str)),
                                    Err(e) => errors.push((key.clone(), format!("encode: {}", e))),
                                }
                                transformed += 1;
                            }
                            Err(e) => {
                                errors.push((key.clone(), format!("transform: {}", e)));
                            }
                        }
                    }
                    Ok(false) => {
                        filtered += 1;
                    }
                    Err(e) => {
                        errors.push((key.clone(), format!("filter: {}", e)));
                    }
                }
                processed += 1;
            }
            None => {
                errors.push((key.clone(), "not found".to_string()));
            }
        }
    }

    // Log individual errors at warn level
    for (key, error) in &errors {
        warn!(key = %key, error = %error, "map error for key");
    }

    info!(
        processed,
        transformed,
        filtered,
        errors = errors.len(),
        "map operation completed"
    );

    Ok(Json(MapResult {
        processed,
        transformed,
        filtered,
        errors,
        results,
    }))
}

/// Execute a reduce operation.
///
/// Expects a multipart/form-data request with:
/// - `module`: WASM component bytes
/// - `config`: JSON with ReduceConfig
#[instrument(skip(state, multipart), fields(database = %database, keyspace = %keyspace))]
pub async fn reduce_operation(
    State(state): State<AppState>,
    Path((database, keyspace)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Json<ReduceResult>, ApiError> {
    debug!("starting reduce operation");

    // Extract module bytes and config from multipart
    let (module_bytes, config) = extract_reduce_multipart(&mut multipart).await?;

    debug!(
        module_size = module_bytes.len(),
        input_type = %config.input_type,
        state_type = %config.state_type,
        filter.prefix = config.filter.prefix.as_deref(),
        filter.limit = config.filter.limit,
        "reduce config extracted"
    );

    let store = state.get_database(&database)?;

    // Create TypedRunner with input_type for values and state_type for state
    let mut runner = TypedRunner::builder()
        .component_bytes(module_bytes)
        .wit_text(&config.wit_definition)
        .input_type(&config.input_type)
        .output_type(&config.state_type)
        .build()
        .map_err(ApiError::from)?;

    // Get keyspace metadata for type version
    let metadata = store
        .get_type(&keyspace)?
        .ok_or_else(|| ApiError::keyspace_not_found(&database, &keyspace))?;

    // Get keys based on filter
    let keys = get_filtered_keys(store, &keyspace, &config.filter)?;

    // Initialize state
    let mut current_state = runner
        .call_init_state(metadata.type_version)
        .map_err(ApiError::from)?;

    let mut processed: u32 = 0;
    let mut errors: Vec<(String, String)> = Vec::new();

    for key in keys {
        match store.get_raw(&keyspace, &key)? {
            Some(stored) => {
                match runner.call_reduce(&current_state, &stored, metadata.type_version) {
                    Ok(new_state) => {
                        current_state = new_state;
                        processed += 1;
                    }
                    Err(e) => {
                        errors.push((key.clone(), format!("reduce: {}", e)));
                    }
                }
            }
            None => {
                errors.push((key.clone(), "not found".to_string()));
            }
        }
    }

    // Convert final state to WAVE string
    let state_str = runner
        .stored_to_wave_string(&current_state)
        .map_err(|e| ApiError::internal(format!("encode: {}", e)))?;

    let error_count = errors.len() as u32;

    // Log individual errors at warn level
    for (key, error) in &errors {
        warn!(key = %key, error = %error, "reduce error for key");
    }

    info!(processed, error_count, "reduce operation completed");

    Ok(Json(ReduceResult {
        processed,
        error_count,
        errors,
        state: state_str,
    }))
}
