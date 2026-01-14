//! Reduce (fold) operation for aggregating values in a keyspace.

use std::path::Path;

use crate::kv::{BinaryExport, KvStore};

use super::error::WasmError;
use super::map::KeyFilter;
use super::runner::WasmRunner;

/// Result of a reduce operation.
#[derive(Debug)]
pub struct ReduceResult {
    /// The final accumulated state.
    pub final_state: BinaryExport,
    /// Number of values that were processed.
    pub processed_count: usize,
}

/// Execute a reduce (fold-left) operation on a keyspace.
pub struct ReduceOperation<'a> {
    store: &'a KvStore,
    runner: WasmRunner,
}

impl<'a> ReduceOperation<'a> {
    /// Create a new reduce operation.
    pub fn new(store: &'a KvStore, module_path: &Path) -> Result<Self, WasmError> {
        let runner = WasmRunner::new(module_path)?;
        Ok(Self { store, runner })
    }

    /// Execute the reduce operation.
    pub fn execute(
        &mut self,
        keyspace: &str,
        key_filter: KeyFilter,
        limit: Option<usize>,
    ) -> Result<ReduceResult, WasmError> {
        // Initialize state
        let mut state = self.runner.call_init_state()?;
        let mut processed_count = 0;

        // Get iterator over keys
        let keys = self.get_keys(keyspace, &key_filter)?;

        for key in keys {
            // Check limit
            if let Some(l) = limit {
                if processed_count >= l {
                    break;
                }
            }

            // Get raw value
            let stored = match self.store.get_raw(keyspace, &key)? {
                Some(s) => s,
                None => continue, // Key was deleted between list and get
            };

            // Convert to BinaryExport for the wasm module
            let export = BinaryExport::from_stored(&stored);

            // Call reduce to fold in this value
            state = self.runner.call_reduce(&state, &export)?;
            processed_count += 1;
        }

        Ok(ReduceResult {
            final_state: state,
            processed_count,
        })
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
