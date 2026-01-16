//! Typed WebAssembly Component runner.
//!
//! This module provides a runner for typed map/reduce operations where
//! components receive actual WIT types.
//!
//! ## Example
//!
//! ```ignore
//! // Component WIT defines: filter: func(value: point) -> bool
//! let runner = TypedRunner::new(&component_path, &wit_path, "point")?;
//!
//! // Convert stored value to typed Val
//! let point_val = runner.stored_to_val(&stored_value)?;
//!
//! // Call with typed value
//! let result = runner.call_filter(&point_val)?;
//! ```

use std::path::{Path, PathBuf};

use wasmtime::component::types;
use wasmtime::component::{Component, Func, Instance, Linker, Val};
use wasmtime::{Config, Engine, Store};
use wit_parser::{Resolve, TypeId};

use super::error::WasmError;
use crate::find_type_by_name;
use crate::kv::{SemanticVersion, StoredValue};
use crate::logging::{debug, error, info, trace};
use wit_kv_abi::{CanonicalAbi, LinearMemory};

// Re-export val conversion functions from wit_kv_abi
pub use wit_kv_abi::{val_to_wave, wave_to_val};

/// Create a placeholder Val for function results based on type.
pub fn create_placeholder_val(ty: &types::Type) -> Result<Val, WasmError> {
    match ty {
        types::Type::Bool => Ok(Val::Bool(false)),
        types::Type::U8 => Ok(Val::U8(0)),
        types::Type::S8 => Ok(Val::S8(0)),
        types::Type::U16 => Ok(Val::U16(0)),
        types::Type::S16 => Ok(Val::S16(0)),
        types::Type::U32 => Ok(Val::U32(0)),
        types::Type::S32 => Ok(Val::S32(0)),
        types::Type::U64 => Ok(Val::U64(0)),
        types::Type::S64 => Ok(Val::S64(0)),
        types::Type::Float32 => Ok(Val::Float32(0.0)),
        types::Type::Float64 => Ok(Val::Float64(0.0)),
        types::Type::Char => Ok(Val::Char('\0')),
        types::Type::String => Ok(Val::String(String::new())),
        types::Type::Record(record_type) => {
            let fields: Result<Vec<_>, WasmError> = record_type
                .fields()
                .map(|f| {
                    let val = create_placeholder_val(&f.ty)?;
                    Ok((f.name.to_string(), val))
                })
                .collect();
            Ok(Val::Record(fields?))
        }
        types::Type::List(_) => Ok(Val::List(vec![])),
        types::Type::Option(_) => Ok(Val::Option(None)),
        types::Type::Tuple(tuple_type) => {
            let elements: Result<Vec<_>, WasmError> = tuple_type
                .types()
                .map(|ty| create_placeholder_val(&ty))
                .collect();
            Ok(Val::Tuple(elements?))
        }
        _ => Err(WasmError::TypeMismatch {
            keyspace_type: "cannot create placeholder for type".to_string(),
        }),
    }
}

/// Builder for creating [`TypedRunner`] instances with a fluent API.
///
/// # Example
///
/// ```ignore
/// use wit_kv::TypedRunner;
///
/// // Simple usage with same input/output type
/// let runner = TypedRunner::builder()
///     .component("filter.wasm")
///     .wit("types.wit")
///     .input_type("point")
///     .build()?;
///
/// // With different input and output types
/// let runner = TypedRunner::builder()
///     .component("transform.wasm")
///     .wit("types.wit")
///     .input_type("point")
///     .output_type("magnitude")
///     .build()?;
/// ```
#[derive(Default)]
pub struct TypedRunnerBuilder {
    component_path: Option<PathBuf>,
    component_bytes: Option<Vec<u8>>,
    wit_path: Option<PathBuf>,
    wit_text: Option<String>,
    input_type_name: Option<String>,
    output_type_name: Option<String>,
}

impl TypedRunnerBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the path to the WebAssembly component file.
    ///
    /// This is required (or use `component_bytes`) and must point to a valid `.wasm` component file.
    pub fn component(mut self, path: impl AsRef<Path>) -> Self {
        self.component_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the WebAssembly component bytes directly.
    ///
    /// Use this instead of `component()` when you have the WASM bytes in memory.
    /// This is useful for HTTP API requests where the component is uploaded.
    pub fn component_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.component_bytes = Some(bytes);
        self
    }

    /// Set the path to the WIT file defining the types.
    ///
    /// This is required (or use `wit_text`) and must point to a valid `.wit` file containing
    /// the type definitions for the component.
    pub fn wit(mut self, path: impl AsRef<Path>) -> Self {
        self.wit_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the WIT definition text directly.
    ///
    /// Use this instead of `wit()` when you have the WIT definition as a string.
    /// This is useful for HTTP API requests where the WIT is provided inline.
    pub fn wit_text(mut self, wit: impl Into<String>) -> Self {
        self.wit_text = Some(wit.into());
        self
    }

    /// Set the name of the input type.
    ///
    /// This is required and must match a type name defined in the WIT file.
    pub fn input_type(mut self, name: impl Into<String>) -> Self {
        self.input_type_name = Some(name.into());
        self
    }

    /// Set the name of the output type.
    ///
    /// If not specified, defaults to the input type name. This is useful
    /// for transformations that change the type (e.g., `point` -> `magnitude`).
    pub fn output_type(mut self, name: impl Into<String>) -> Self {
        self.output_type_name = Some(name.into());
        self
    }

    /// Build the [`TypedRunner`] with the configured options.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required fields (component or component_bytes, wit or wit_text, input_type) are not set
    /// - The component file cannot be loaded
    /// - The WIT file cannot be parsed
    /// - The specified types are not found in the WIT file
    pub fn build(self) -> Result<TypedRunner, WasmError> {
        // Get component bytes from path or direct bytes
        let component_bytes = match (self.component_path, self.component_bytes) {
            (Some(path), None) => std::fs::read(&path)?,
            (None, Some(bytes)) => bytes,
            (None, None) => {
                return Err(WasmError::TypeMismatch {
                    keyspace_type: "component path or bytes is required".to_string(),
                });
            }
            (Some(_), Some(_)) => {
                return Err(WasmError::TypeMismatch {
                    keyspace_type: "provide either component path or bytes, not both".to_string(),
                });
            }
        };

        // Load WIT definition from path or text
        let mut resolve = Resolve::new();
        match (self.wit_path, self.wit_text) {
            (Some(path), None) => {
                resolve.push_path(&path)?;
            }
            (None, Some(text)) => {
                resolve.push_str("<inline>", &text)?;
            }
            (None, None) => {
                return Err(WasmError::TypeMismatch {
                    keyspace_type: "WIT path or text is required".to_string(),
                });
            }
            (Some(_), Some(_)) => {
                return Err(WasmError::TypeMismatch {
                    keyspace_type: "provide either WIT path or text, not both".to_string(),
                });
            }
        };

        let input_type_name = self
            .input_type_name
            .ok_or_else(|| WasmError::TypeMismatch {
                keyspace_type: "input type name is required".to_string(),
            })?;

        let output_type_name = self.output_type_name;

        TypedRunner::from_parts(
            component_bytes,
            resolve,
            &input_type_name,
            output_type_name.as_deref(),
        )
    }
}

/// Runner for typed WebAssembly Components.
///
/// This runner works with actual WIT types, converting between storage format
/// and typed `Val` values.
///
/// # Example
///
/// Using the builder pattern (recommended):
///
/// ```ignore
/// use wit_kv::TypedRunner;
///
/// let runner = TypedRunner::builder()
///     .component("filter.wasm")
///     .wit("types.wit")
///     .input_type("point")
///     .build()?;
///
/// // Use the runner for filter/transform operations
/// let passes = runner.call_filter(&stored_value)?;
/// ```
///
/// Using the direct constructor:
///
/// ```ignore
/// let runner = TypedRunner::new(
///     Path::new("filter.wasm"),
///     Path::new("types.wit"),
///     "point",
///     None,
/// )?;
/// ```
pub struct TypedRunner {
    engine: Engine,
    store: Store<()>,
    instance: Instance,
    resolve: Resolve,
    input_type_id: TypeId,
    output_type_id: TypeId,
}

impl TypedRunner {
    /// Create a builder for constructing a TypedRunner with a fluent API.
    ///
    /// This is the recommended way to create a TypedRunner as it provides
    /// better ergonomics and clearer intent.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let runner = TypedRunner::builder()
    ///     .component("filter.wasm")
    ///     .wit("types.wit")
    ///     .input_type("point")
    ///     .build()?;
    /// ```
    pub fn builder() -> TypedRunnerBuilder {
        TypedRunnerBuilder::new()
    }

    /// Create a new TypedRunner by loading a component.
    ///
    /// For a more ergonomic API, consider using [`TypedRunner::builder()`] instead.
    ///
    /// # Arguments
    /// * `module_path` - Path to the WASM component
    /// * `wit_path` - Path to the WIT file defining types
    /// * `input_type_name` - Name of the input type (e.g., "point")
    /// * `output_type_name` - Name of the output type (defaults to input type)
    pub fn new(
        module_path: impl AsRef<Path>,
        wit_path: impl AsRef<Path>,
        input_type_name: &str,
        output_type_name: Option<&str>,
    ) -> Result<Self, WasmError> {
        // Load WIT definitions
        let mut resolve = Resolve::new();
        resolve.push_path(wit_path)?;

        // Find input type
        let input_type_id = find_type_by_name(&resolve, input_type_name).ok_or_else(|| {
            WasmError::TypeMismatch {
                keyspace_type: format!("input type '{}' not found in WIT", input_type_name),
            }
        })?;

        // Find output type (defaults to input type)
        let output_type_name = output_type_name.unwrap_or(input_type_name);
        let output_type_id = find_type_by_name(&resolve, output_type_name).ok_or_else(|| {
            WasmError::TypeMismatch {
                keyspace_type: format!("output type '{}' not found in WIT", output_type_name),
            }
        })?;

        // Create wasmtime engine
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)?;

        // Load component
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
            resolve,
            input_type_id,
            output_type_id,
        })
    }

    /// Create a TypedRunner from pre-loaded parts.
    ///
    /// This is used internally by the builder when loading from bytes or text.
    ///
    /// # Arguments
    /// * `component_bytes` - The WASM component bytes
    /// * `resolve` - Pre-loaded WIT resolver with type definitions
    /// * `input_type_name` - Name of the input type (e.g., "point")
    /// * `output_type_name` - Name of the output type (defaults to input type)
    pub fn from_parts(
        component_bytes: Vec<u8>,
        resolve: Resolve,
        input_type_name: &str,
        output_type_name: Option<&str>,
    ) -> Result<Self, WasmError> {
        debug!(
            component_size = component_bytes.len(),
            input_type = input_type_name,
            output_type = output_type_name,
            "creating TypedRunner"
        );

        // Find input type
        let input_type_id = find_type_by_name(&resolve, input_type_name).ok_or_else(|| {
            error!(type_name = input_type_name, "input type not found in WIT");
            WasmError::TypeMismatch {
                keyspace_type: format!("input type '{}' not found in WIT", input_type_name),
            }
        })?;

        // Find output type (defaults to input type)
        let output_type_name = output_type_name.unwrap_or(input_type_name);
        let output_type_id = find_type_by_name(&resolve, output_type_name).ok_or_else(|| {
            error!(type_name = output_type_name, "output type not found in WIT");
            WasmError::TypeMismatch {
                keyspace_type: format!("output type '{}' not found in WIT", output_type_name),
            }
        })?;

        trace!("creating wasmtime engine with component model");

        // Create wasmtime engine
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)?;

        // Load component from bytes
        trace!(bytes = component_bytes.len(), "loading WASM component");
        let component = Component::new(&engine, &component_bytes)?;

        // Create linker and store
        let linker: Linker<()> = Linker::new(&engine);
        let mut store = Store::new(&engine, ());

        // Instantiate the component
        trace!("instantiating component");
        let instance = linker.instantiate(&mut store, &component)?;

        info!(
            input_type = input_type_name,
            output_type = output_type_name,
            "TypedRunner created"
        );

        Ok(Self {
            engine,
            store,
            instance,
            resolve,
            input_type_id,
            output_type_id,
        })
    }

    /// Get the input type wave representation.
    pub fn input_wave_type(&self) -> Result<wasm_wave::value::Type, WasmError> {
        crate::resolve_wit_type(&self.resolve, self.input_type_id).map_err(|e| {
            WasmError::TypeMismatch {
                keyspace_type: format!("failed to resolve input type: {}", e),
            }
        })
    }

    /// Get the output type wave representation.
    pub fn output_wave_type(&self) -> Result<wasm_wave::value::Type, WasmError> {
        crate::resolve_wit_type(&self.resolve, self.output_type_id).map_err(|e| {
            WasmError::TypeMismatch {
                keyspace_type: format!("failed to resolve output type: {}", e),
            }
        })
    }

    /// Convert a StoredValue (output type) to a WAVE-encoded string.
    ///
    /// This performs the full conversion pipeline:
    /// 1. Lifts binary data to wasmtime Val
    /// 2. Converts Val to wasm_wave Value
    /// 3. Serializes to WAVE string format
    ///
    /// This is useful for displaying transform/reduce results.
    pub fn stored_to_wave_string(&self, stored: &StoredValue) -> Result<String, WasmError> {
        let wave_type = self.output_wave_type()?;
        let abi = CanonicalAbi::new(&self.resolve);
        let memory = LinearMemory::from_optional(stored.memory.as_ref());

        let (val, _) = abi.lift_to_val(
            &stored.value,
            &wit_parser::Type::Id(self.output_type_id),
            None,
            &memory,
        )?;

        let wave_value = val_to_wave(&val, &wave_type)?;

        wasm_wave::to_string(&wave_value).map_err(|e| WasmError::TypeMismatch {
            keyspace_type: format!("failed to encode WAVE string: {}", e),
        })
    }

    /// Convert a StoredValue to a typed Val for function calls.
    /// Uses direct binary -> Val conversion (hot path, bypasses wasm_wave::Value).
    pub fn stored_to_val(
        &self,
        stored: &StoredValue,
        func_param_type: &types::Type,
    ) -> Result<Val, WasmError> {
        let memory = LinearMemory::from_optional(stored.memory.as_ref());

        let abi = CanonicalAbi::new(&self.resolve);
        let (val, _) = abi.lift_to_val(
            &stored.value,
            &wit_parser::Type::Id(self.input_type_id),
            Some(func_param_type),
            &memory,
        )?;

        Ok(val)
    }

    /// Convert a typed Val result back to StoredValue.
    /// Uses direct Val -> binary conversion (hot path, bypasses wasm_wave::Value).
    pub fn val_to_stored(
        &self,
        val: &Val,
        type_version: SemanticVersion,
    ) -> Result<StoredValue, WasmError> {
        let mut memory = LinearMemory::new();
        let abi = CanonicalAbi::new(&self.resolve);
        let buffer =
            abi.lower_from_val(val, &wit_parser::Type::Id(self.output_type_id), &mut memory)?;

        Ok(StoredValue::new(
            type_version,
            buffer,
            if memory.is_empty() {
                None
            } else {
                Some(memory.into_bytes())
            },
        ))
    }

    /// Get a function by name from the component instance.
    fn get_func(&mut self, name: &str) -> Result<Func, WasmError> {
        self.instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| WasmError::FunctionNotFound(name.to_string()))
    }

    /// Call the `filter` function with a typed value.
    ///
    /// The filter function should have signature: `filter(value: T) -> bool`
    pub fn call_filter(&mut self, stored: &StoredValue) -> Result<bool, WasmError> {
        debug!("calling filter function");
        let func = self.get_func("filter")?;

        // Get function type to determine parameter type
        let func_type = func.ty(&self.store);
        let (_, param_type) =
            func_type
                .params()
                .next()
                .ok_or_else(|| WasmError::InvalidReturnType {
                    expected: "filter function should have 1 parameter".to_string(),
                })?;

        // Convert stored value to wasmtime Val
        trace!("converting stored value to Val");
        let input_val = self.stored_to_val(stored, &param_type)?;

        // Call function
        let mut results = vec![Val::Bool(false)];
        func.call(&mut self.store, &[input_val], &mut results)
            .map_err(|e| {
                error!(error = %e, "filter function trap");
                WasmError::Trap(e.to_string())
            })?;

        func.post_return(&mut self.store).map_err(|e| {
            error!(error = %e, "filter post_return failed");
            WasmError::Trap(format!("post_return failed: {}", e))
        })?;

        match results.first() {
            Some(Val::Bool(b)) => {
                debug!(result = *b, "filter function completed");
                Ok(*b)
            }
            Some(other) => {
                error!(result = ?other, "filter returned unexpected type");
                Err(WasmError::InvalidReturnType {
                    expected: format!("bool, got {:?}", other),
                })
            }
            None => {
                error!("filter returned no result");
                Err(WasmError::InvalidReturnType {
                    expected: "bool, got no result".to_string(),
                })
            }
        }
    }

    /// Call the `transform` function with a typed value.
    ///
    /// The transform function should have signature: `transform(value: T) -> T1`
    pub fn call_transform(
        &mut self,
        stored: &StoredValue,
        type_version: SemanticVersion,
    ) -> Result<StoredValue, WasmError> {
        debug!("calling transform function");
        let func = self.get_func("transform")?;

        // Get function type
        let func_type = func.ty(&self.store);
        let (_, param_type) =
            func_type
                .params()
                .next()
                .ok_or_else(|| WasmError::InvalidReturnType {
                    expected: "transform function should have 1 parameter".to_string(),
                })?;

        let result_type =
            func_type
                .results()
                .next()
                .ok_or_else(|| WasmError::InvalidReturnType {
                    expected: "transform function should have 1 result".to_string(),
                })?;

        // Convert input
        trace!("converting stored value to Val");
        let input_val = self.stored_to_val(stored, &param_type)?;

        // Create result placeholder
        let mut results = vec![create_placeholder_val(&result_type)?];

        // Call function
        func.call(&mut self.store, &[input_val], &mut results)
            .map_err(|e| {
                error!(error = %e, "transform function trap");
                WasmError::Trap(e.to_string())
            })?;

        // Convert result to StoredValue
        let result_val = results.first().ok_or_else(|| {
            error!("transform returned no result");
            WasmError::InvalidReturnType {
                expected: "transform function should return a value".to_string(),
            }
        })?;
        trace!("converting result Val to StoredValue");
        let output = self.val_to_stored(result_val, type_version)?;

        func.post_return(&mut self.store).map_err(|e| {
            error!(error = %e, "transform post_return failed");
            WasmError::Trap(format!("post_return failed: {}", e))
        })?;

        debug!("transform function completed");
        Ok(output)
    }

    /// Get a reference to the engine (useful for type introspection).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Call the `init-state` function to get the initial reduce state.
    ///
    /// The init-state function should have signature: `init-state() -> StateType`
    pub fn call_init_state(
        &mut self,
        type_version: SemanticVersion,
    ) -> Result<StoredValue, WasmError> {
        debug!("calling init-state function");
        let func = self.get_func("init-state")?;

        // Get function type
        let func_type = func.ty(&self.store);
        let result_type =
            func_type
                .results()
                .next()
                .ok_or_else(|| WasmError::InvalidReturnType {
                    expected: "init-state function should have 1 result".to_string(),
                })?;

        // Create result placeholder
        let mut results = vec![create_placeholder_val(&result_type)?];

        // Call function (no parameters)
        func.call(&mut self.store, &[], &mut results).map_err(|e| {
            error!(error = %e, "init-state function trap");
            WasmError::Trap(e.to_string())
        })?;

        // Convert result to StoredValue using output_type (which is the state type)
        let result_val = results.first().ok_or_else(|| {
            error!("init-state returned no result");
            WasmError::InvalidReturnType {
                expected: "init-state function should return a value".to_string(),
            }
        })?;
        trace!("converting result Val to StoredValue");
        let output = self.val_to_stored(result_val, type_version)?;

        func.post_return(&mut self.store).map_err(|e| {
            error!(error = %e, "init-state post_return failed");
            WasmError::Trap(format!("post_return failed: {}", e))
        })?;

        debug!("init-state function completed");
        Ok(output)
    }

    /// Convert a state StoredValue to a typed Val for reduce calls.
    /// Uses direct binary -> Val conversion (hot path, bypasses wasm_wave::Value).
    ///
    /// This uses the output_type_id (state type) for conversion.
    pub fn state_to_val(
        &self,
        stored: &StoredValue,
        func_param_type: &types::Type,
    ) -> Result<Val, WasmError> {
        let memory = LinearMemory::from_optional(stored.memory.as_ref());

        let abi = CanonicalAbi::new(&self.resolve);
        let (val, _) = abi.lift_to_val(
            &stored.value,
            &wit_parser::Type::Id(self.output_type_id), // Use output_type for state
            Some(func_param_type),
            &memory,
        )?;

        Ok(val)
    }

    /// Call the `reduce` function to fold a value into the state.
    ///
    /// The reduce function should have signature: `reduce(state: StateType, value: T) -> StateType`
    pub fn call_reduce(
        &mut self,
        state: &StoredValue,
        value: &StoredValue,
        type_version: SemanticVersion,
    ) -> Result<StoredValue, WasmError> {
        debug!("calling reduce function");
        let func = self.get_func("reduce")?;

        // Get function type
        let func_type = func.ty(&self.store);
        let mut params = func_type.params();

        let (_, state_param_type) = params.next().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "reduce function should have 2 parameters (state, value)".to_string(),
        })?;

        let (_, value_param_type) = params.next().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "reduce function should have 2 parameters (state, value)".to_string(),
        })?;

        let result_type =
            func_type
                .results()
                .next()
                .ok_or_else(|| WasmError::InvalidReturnType {
                    expected: "reduce function should have 1 result".to_string(),
                })?;

        // Convert state and value to wasmtime Vals
        trace!("converting state and value to Vals");
        let state_val = self.state_to_val(state, &state_param_type)?;
        let value_val = self.stored_to_val(value, &value_param_type)?;

        // Create result placeholder
        let mut results = vec![create_placeholder_val(&result_type)?];

        // Call function
        func.call(&mut self.store, &[state_val, value_val], &mut results)
            .map_err(|e| {
                error!(error = %e, "reduce function trap");
                WasmError::Trap(e.to_string())
            })?;

        // Convert result to StoredValue
        let result_val = results.first().ok_or_else(|| {
            error!("reduce returned no result");
            WasmError::InvalidReturnType {
                expected: "reduce function should return a value".to_string(),
            }
        })?;
        trace!("converting result Val to StoredValue");
        let output = self.val_to_stored(result_val, type_version)?;

        func.post_return(&mut self.store).map_err(|e| {
            error!(error = %e, "reduce post_return failed");
            WasmError::Trap(format!("post_return failed: {}", e))
        })?;

        debug!("reduce function completed");
        Ok(output)
    }
}
