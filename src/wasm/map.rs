//! Map operation for transforming values in a keyspace.

use std::path::Path;

use crate::kv::{BinaryExport, KvStore};

use super::error::WasmError;
use super::runner::WasmRunner;

/// Filter for selecting which keys to process.
#[derive(Debug, Clone)]
pub enum KeyFilter {
    /// Process all keys in the keyspace.
    All,
    /// Process a single specific key.
    Single(String),
    /// Process keys matching a prefix.
    Prefix(String),
    /// Process keys in a range.
    Range {
        start: Option<String>,
        end: Option<String>,
    },
}

/// Result of a map operation.
#[derive(Debug)]
pub struct MapResult {
    /// Successfully mapped values: (key, transformed_value).
    pub values: Vec<(String, BinaryExport)>,
    /// Count of values filtered out by the filter function.
    pub filtered_count: usize,
    /// Errors encountered during processing.
    pub errors: Vec<(String, String)>,
}

impl MapResult {
    /// Create a new empty MapResult.
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            filtered_count: 0,
            errors: Vec::new(),
        }
    }

    /// Check if any errors occurred.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get a summary string.
    pub fn summary(&self) -> String {
        format!(
            "Processed: {} mapped, {} filtered out, {} errors",
            self.values.len(),
            self.filtered_count,
            self.errors.len()
        )
    }
}

impl Default for MapResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a map operation on a keyspace.
pub struct MapOperation<'a> {
    store: &'a KvStore,
    runner: WasmRunner,
}

impl<'a> MapOperation<'a> {
    /// Create a new map operation.
    pub fn new(store: &'a KvStore, module_path: &Path) -> Result<Self, WasmError> {
        let runner = WasmRunner::new(module_path)?;
        Ok(Self { store, runner })
    }

    /// Execute the map operation.
    pub fn execute(
        &mut self,
        keyspace: &str,
        key_filter: KeyFilter,
        limit: Option<usize>,
    ) -> Result<MapResult, WasmError> {
        let mut result = MapResult::new();
        let mut count = 0;

        // Get iterator over keys
        let keys = self.get_keys(keyspace, &key_filter)?;

        for key in keys {
            // Check limit
            if let Some(l) = limit
                && count >= l
            {
                break;
            }

            // Get raw value
            let stored = match self.store.get_raw(keyspace, &key)? {
                Some(s) => s,
                None => continue, // Key was deleted between list and get
            };

            // Convert to BinaryExport for the wasm module
            let export = BinaryExport::from_stored(&stored);

            // Call filter
            match self.runner.call_filter(&export) {
                Ok(should_transform) => {
                    if should_transform {
                        // Call transform
                        match self.runner.call_transform(&export) {
                            Ok(transformed) => {
                                result.values.push((key, transformed));
                            }
                            Err(e) => {
                                result.errors.push((key, e.to_string()));
                            }
                        }
                    } else {
                        result.filtered_count += 1;
                    }
                }
                Err(e) => {
                    result.errors.push((key, e.to_string()));
                }
            }

            count += 1;
        }

        Ok(result)
    }

    /// Get the list of keys to process based on the filter.
    fn get_keys(&self, keyspace: &str, filter: &KeyFilter) -> Result<Vec<String>, WasmError> {
        match filter {
            KeyFilter::All => {
                self.store.list(keyspace, None, None).map_err(WasmError::from)
            }
            KeyFilter::Single(key) => Ok(vec![key.clone()]),
            KeyFilter::Prefix(prefix) => self
                .store
                .list(keyspace, Some(prefix), None)
                .map_err(WasmError::from),
            KeyFilter::Range { start, end } => {
                // For range queries, we get all keys and filter
                // A more efficient implementation would use range iterators
                let all_keys = self.store.list(keyspace, None, None)?;
                Ok(all_keys
                    .into_iter()
                    .filter(|k| {
                        let after_start = match start {
                            Some(s) => k >= s,
                            None => true,
                        };
                        let before_end = match end {
                            Some(e) => k < e,
                            None => true,
                        };
                        after_start && before_end
                    })
                    .collect())
            }
        }
    }
}
