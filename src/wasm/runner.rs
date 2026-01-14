//! WebAssembly Component runner using wasmtime.
//!
//! This module provides a wrapper around wasmtime's component model for executing
//! map and reduce operations on KV store values.

use std::path::Path;

use wasmtime::component::{Component, Func, Instance, Linker, Val};
use wasmtime::{Config, Engine, Store};

use super::error::WasmError;
use crate::kv::BinaryExport;

/// Runner for WebAssembly Components implementing map/reduce interfaces.
pub struct WasmRunner {
    engine: Engine,
    store: Store<()>,
    instance: Instance,
}

impl WasmRunner {
    /// Create a new WasmRunner by loading a component from the given path.
    pub fn new(module_path: &Path) -> Result<Self, WasmError> {
        // Create engine with component model enabled
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)?;

        // Load the component
        let component_bytes = std::fs::read(module_path)?;
        let component = Component::new(&engine, &component_bytes)?;

        // Create linker and store
        let linker: Linker<()> = Linker::new(&engine);
        let mut store = Store::new(&engine, ());

        // Instantiate the component
        let instance = linker.instantiate(&mut store, &component)?;

        Ok(Self {
            engine,
            store,
            instance,
        })
    }

    /// Call the `filter` function exported by the component.
    ///
    /// The filter function has signature: `filter(value: binary-export) -> bool`
    pub fn call_filter(&mut self, value: &BinaryExport) -> Result<bool, WasmError> {
        let func = self.get_func("filter")?;

        // Convert BinaryExport to wasmtime Val
        let val_arg = Self::binary_export_to_val(value);

        // Prepare result storage
        let mut results = vec![Val::Bool(false)];

        // Call the function
        func.call(&mut self.store, &[val_arg], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        // Post-call cleanup
        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

        // Extract result
        match results.first() {
            Some(Val::Bool(b)) => Ok(*b),
            Some(other) => Err(WasmError::InvalidReturnType {
                expected: format!("bool, got {:?}", other),
            }),
            None => Err(WasmError::InvalidReturnType {
                expected: "bool, got no results".to_string(),
            }),
        }
    }

    /// Call the `transform` function exported by the component.
    ///
    /// The transform function has signature: `transform(value: binary-export) -> binary-export`
    pub fn call_transform(&mut self, value: &BinaryExport) -> Result<BinaryExport, WasmError> {
        let func = self.get_func("transform")?;

        // Convert BinaryExport to wasmtime Val
        let val_arg = Self::binary_export_to_val(value);

        // Prepare result storage - binary-export record
        let mut results = vec![Self::empty_binary_export_val()];

        // Call the function
        func.call(&mut self.store, &[val_arg], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        // Extract result before post_return
        let result_val = results.first().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "binary-export, got no results".to_string(),
        })?;
        let result = Self::val_to_binary_export(result_val)?;

        // Post-call cleanup
        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

        Ok(result)
    }

    /// Call the `init-state` function exported by the component.
    ///
    /// The init-state function has signature: `init-state() -> binary-export`
    pub fn call_init_state(&mut self) -> Result<BinaryExport, WasmError> {
        let func = self.get_func("init-state")?;

        // Prepare result storage - binary-export record
        let mut results = vec![Self::empty_binary_export_val()];

        // Call the function with no arguments
        func.call(&mut self.store, &[], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        // Extract result before post_return
        let result_val = results.first().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "binary-export, got no results".to_string(),
        })?;
        let result = Self::val_to_binary_export(result_val)?;

        // Post-call cleanup
        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

        Ok(result)
    }

    /// Call the `reduce` function exported by the component.
    ///
    /// The reduce function has signature: `reduce(state: binary-export, value: binary-export) -> binary-export`
    pub fn call_reduce(
        &mut self,
        state: &BinaryExport,
        value: &BinaryExport,
    ) -> Result<BinaryExport, WasmError> {
        let func = self.get_func("reduce")?;

        // Convert BinaryExport values to wasmtime Vals
        let state_val = Self::binary_export_to_val(state);
        let value_val = Self::binary_export_to_val(value);

        // Prepare result storage
        let mut results = vec![Self::empty_binary_export_val()];

        // Call the function
        func.call(&mut self.store, &[state_val, value_val], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        // Extract result before post_return
        let result_val = results.first().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "binary-export, got no results".to_string(),
        })?;
        let result = Self::val_to_binary_export(result_val)?;

        // Post-call cleanup
        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

        Ok(result)
    }

    /// Get a function by name from the component instance.
    fn get_func(&mut self, name: &str) -> Result<Func, WasmError> {
        self.instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| WasmError::FunctionNotFound(name.to_string()))
    }

    /// Convert a BinaryExport to a wasmtime Val (record type).
    fn binary_export_to_val(export: &BinaryExport) -> Val {
        // Create list of u8 values for the 'value' field
        let value_list: Vec<Val> = export.buffer.iter().map(|&b| Val::U8(b)).collect();

        // Create the optional memory field
        let memory_val = match &export.memory {
            Some(mem) => {
                let mem_list: Vec<Val> = mem.iter().map(|&b| Val::U8(b)).collect();
                Val::Option(Some(Box::new(Val::List(mem_list))))
            }
            None => Val::Option(None),
        };

        // Create the record
        Val::Record(vec![
            ("value".to_string(), Val::List(value_list)),
            ("memory".to_string(), memory_val),
        ])
    }

    /// Create an empty binary-export Val as a placeholder for results.
    fn empty_binary_export_val() -> Val {
        Val::Record(vec![
            ("value".to_string(), Val::List(Vec::new())),
            ("memory".to_string(), Val::Option(None)),
        ])
    }

    /// Convert a wasmtime Val (record) back to a BinaryExport.
    fn val_to_binary_export(val: &Val) -> Result<BinaryExport, WasmError> {
        match val {
            Val::Record(fields) => {
                let mut buffer = Vec::new();
                let mut memory = None;

                for (name, field_val) in fields {
                    match name.as_str() {
                        "value" => {
                            buffer = Self::extract_u8_list(field_val)?;
                        }
                        "memory" => {
                            memory = Self::extract_optional_u8_list(field_val)?;
                        }
                        _ => {} // Ignore unknown fields
                    }
                }

                Ok(BinaryExport { buffer, memory })
            }
            other => Err(WasmError::InvalidReturnType {
                expected: format!("record (binary-export), got {:?}", other),
            }),
        }
    }

    /// Extract a Vec<u8> from a Val::List of Val::U8.
    fn extract_u8_list(val: &Val) -> Result<Vec<u8>, WasmError> {
        match val {
            Val::List(items) => {
                let mut result = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Val::U8(b) => result.push(*b),
                        other => {
                            return Err(WasmError::InvalidReturnType {
                                expected: format!("u8, got {:?}", other),
                            })
                        }
                    }
                }
                Ok(result)
            }
            other => Err(WasmError::InvalidReturnType {
                expected: format!("list<u8>, got {:?}", other),
            }),
        }
    }

    /// Extract an Option<Vec<u8>> from a Val::Option containing a list.
    fn extract_optional_u8_list(val: &Val) -> Result<Option<Vec<u8>>, WasmError> {
        match val {
            Val::Option(Some(inner)) => {
                let list = Self::extract_u8_list(inner)?;
                Ok(Some(list))
            }
            Val::Option(None) => Ok(None),
            other => Err(WasmError::InvalidReturnType {
                expected: format!("option<list<u8>>, got {:?}", other),
            }),
        }
    }

    /// Get a reference to the engine (useful for type introspection).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}
