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
use wasm_wave::value::Value;
use wasm_wave::wasm::{WasmType, WasmValue};
use wit_parser::{Resolve, TypeId};

use super::error::WasmError;
use crate::abi::{CanonicalAbi, LinearMemory};
use crate::find_type_by_name;
use crate::kv::{SemanticVersion, StoredValue};

/// Convert a wasm_wave::Value to a wasmtime::component::Val based on the target type.
pub fn wave_to_val(wave: &Value, target_type: &types::Type) -> Result<Val, WasmError> {
    match target_type {
        types::Type::Bool => Ok(Val::Bool(wave.unwrap_bool())),
        types::Type::U8 => Ok(Val::U8(wave.unwrap_u8())),
        types::Type::S8 => Ok(Val::S8(wave.unwrap_s8())),
        types::Type::U16 => Ok(Val::U16(wave.unwrap_u16())),
        types::Type::S16 => Ok(Val::S16(wave.unwrap_s16())),
        types::Type::U32 => Ok(Val::U32(wave.unwrap_u32())),
        types::Type::S32 => Ok(Val::S32(wave.unwrap_s32())),
        types::Type::U64 => Ok(Val::U64(wave.unwrap_u64())),
        types::Type::S64 => Ok(Val::S64(wave.unwrap_s64())),
        types::Type::Float32 => Ok(Val::Float32(wave.unwrap_f32())),
        types::Type::Float64 => Ok(Val::Float64(wave.unwrap_f64())),
        types::Type::Char => Ok(Val::Char(wave.unwrap_char())),
        types::Type::String => Ok(Val::String(wave.unwrap_string().to_string())),

        types::Type::Record(record_type) => {
            let wave_fields: Vec<_> = wave.unwrap_record().collect();
            let mut val_fields = Vec::new();

            for field in record_type.fields() {
                let wave_field = wave_fields
                    .iter()
                    .find(|(name, _)| name.as_ref() == field.name)
                    .ok_or_else(|| WasmError::TypeMismatch {
                        keyspace_type: format!("field '{}' not found", field.name),
                    })?;

                let field_val = wave_to_val(&wave_field.1, &field.ty)?;
                val_fields.push((field.name.to_string(), field_val));
            }

            Ok(Val::Record(val_fields))
        }

        types::Type::List(list_type) => {
            let elements: Result<Vec<Val>, _> = wave
                .unwrap_list()
                .map(|elem| wave_to_val(&elem, &list_type.ty()))
                .collect();
            Ok(Val::List(elements?))
        }

        types::Type::Option(option_type) => match wave.unwrap_option() {
            Some(inner) => {
                let inner_val = wave_to_val(&inner, &option_type.ty())?;
                Ok(Val::Option(Some(Box::new(inner_val))))
            }
            None => Ok(Val::Option(None)),
        },

        types::Type::Tuple(tuple_type) => {
            let elements: Result<Vec<Val>, _> = wave
                .unwrap_tuple()
                .zip(tuple_type.types())
                .map(|(elem, ty)| wave_to_val(&elem, &ty))
                .collect();
            Ok(Val::Tuple(elements?))
        }

        types::Type::Enum(_) => Ok(Val::Enum(wave.unwrap_enum().to_string())),

        types::Type::Variant(variant_type) => {
            let (case_name, payload) = wave.unwrap_variant();
            let case = variant_type
                .cases()
                .find(|c| c.name == case_name.as_ref())
                .ok_or_else(|| WasmError::TypeMismatch {
                    keyspace_type: format!("variant case '{}' not found", case_name),
                })?;

            let payload_val = match (payload, &case.ty) {
                (Some(p), Some(ty)) => Some(Box::new(wave_to_val(&p, ty)?)),
                (None, None) => None,
                _ => {
                    return Err(WasmError::TypeMismatch {
                        keyspace_type: "variant payload mismatch".to_string(),
                    })
                }
            };

            Ok(Val::Variant(case_name.to_string(), payload_val))
        }

        types::Type::Flags(_) => {
            let active: Vec<String> = wave.unwrap_flags().map(|s| s.to_string()).collect();
            Ok(Val::Flags(active))
        }

        types::Type::Result(result_type) => match wave.unwrap_result() {
            Ok(ok_val) => {
                let ok_inner = match (ok_val, result_type.ok()) {
                    (Some(v), Some(ty)) => Some(Box::new(wave_to_val(&v, &ty)?)),
                    (None, None) => None,
                    _ => {
                        return Err(WasmError::TypeMismatch {
                            keyspace_type: "result ok payload mismatch".to_string(),
                        })
                    }
                };
                Ok(Val::Result(Ok(ok_inner)))
            }
            Err(err_val) => {
                let err_inner = match (err_val, result_type.err()) {
                    (Some(v), Some(ty)) => Some(Box::new(wave_to_val(&v, &ty)?)),
                    (None, None) => None,
                    _ => {
                        return Err(WasmError::TypeMismatch {
                            keyspace_type: "result err payload mismatch".to_string(),
                        })
                    }
                };
                Ok(Val::Result(Err(err_inner)))
            }
        },

        _ => Err(WasmError::TypeMismatch {
            keyspace_type: "unsupported type for conversion".to_string(),
        }),
    }
}

/// Convert a wasmtime::component::Val back to wasm_wave::Value.
pub fn val_to_wave(
    val: &Val,
    wave_type: &wasm_wave::value::Type,
) -> Result<Value, WasmError> {
    match val {
        Val::Bool(b) => Ok(Value::make_bool(*b)),
        Val::U8(v) => Ok(Value::make_u8(*v)),
        Val::S8(v) => Ok(Value::make_s8(*v)),
        Val::U16(v) => Ok(Value::make_u16(*v)),
        Val::S16(v) => Ok(Value::make_s16(*v)),
        Val::U32(v) => Ok(Value::make_u32(*v)),
        Val::S32(v) => Ok(Value::make_s32(*v)),
        Val::U64(v) => Ok(Value::make_u64(*v)),
        Val::S64(v) => Ok(Value::make_s64(*v)),
        Val::Float32(v) => Ok(Value::make_f32(*v)),
        Val::Float64(v) => Ok(Value::make_f64(*v)),
        Val::Char(c) => Ok(Value::make_char(*c)),
        Val::String(s) => Ok(Value::make_string(std::borrow::Cow::Owned(s.clone()))),

        Val::Record(fields) => {
            // Collect record field types first
            let field_types: Vec<_> = wave_type.record_fields().collect();
            if field_types.is_empty() {
                return Err(WasmError::TypeMismatch {
                    keyspace_type: "expected record type".to_string(),
                });
            }

            let wave_fields: Result<Vec<_>, WasmError> = fields
                .iter()
                .map(|(name, val)| {
                    // Get the field type from wave_type
                    let field_type = field_types
                        .iter()
                        .find(|(n, _): &&(std::borrow::Cow<str>, _)| n.as_ref() == name)
                        .map(|(_, ty)| ty)
                        .ok_or_else(|| WasmError::TypeMismatch {
                            keyspace_type: format!("field '{}' not found in wave type", name),
                        })?;

                    let wave_val = val_to_wave(val, field_type)?;
                    Ok((name.as_str(), wave_val))
                })
                .collect();

            Value::make_record(wave_type, wave_fields?).map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to construct record: {}", e),
            })
        }

        Val::List(elements) => {
            let elem_type = wave_type
                .list_element_type()
                .ok_or_else(|| WasmError::TypeMismatch {
                    keyspace_type: "expected list type".to_string(),
                })?;

            let wave_elems: Result<Vec<_>, _> = elements
                .iter()
                .map(|e| val_to_wave(e, &elem_type))
                .collect();

            Value::make_list(wave_type, wave_elems?).map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to construct list: {}", e),
            })
        }

        Val::Option(opt) => {
            let inner = match opt {
                Some(inner_val) => {
                    let inner_type = wave_type.option_some_type().ok_or_else(|| {
                        WasmError::TypeMismatch {
                            keyspace_type: "expected option type".to_string(),
                        }
                    })?;
                    Some(val_to_wave(inner_val, &inner_type)?)
                }
                None => None,
            };
            Value::make_option(wave_type, inner).map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to construct option: {}", e),
            })
        }

        Val::Tuple(elements) => {
            let tuple_types: Vec<_> = wave_type.tuple_element_types().collect();
            if tuple_types.is_empty() && !elements.is_empty() {
                return Err(WasmError::TypeMismatch {
                    keyspace_type: "expected tuple type".to_string(),
                });
            }

            let wave_elems: Result<Vec<_>, _> = elements
                .iter()
                .zip(tuple_types.iter())
                .map(|(e, ty)| val_to_wave(e, ty))
                .collect();

            Value::make_tuple(wave_type, wave_elems?).map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to construct tuple: {}", e),
            })
        }

        Val::Enum(case_name) => {
            Value::make_enum(wave_type, case_name).map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to construct enum: {}", e),
            })
        }

        Val::Variant(case_name, payload) => {
            // Collect variant cases first
            let variant_cases: Vec<_> = wave_type.variant_cases().collect();
            if variant_cases.is_empty() {
                return Err(WasmError::TypeMismatch {
                    keyspace_type: "expected variant type".to_string(),
                });
            }

            let payload_wave = match payload {
                Some(p) => {
                    // Get the payload type for this case
                    let case_type = variant_cases
                        .iter()
                        .find(|(name, _): &&(std::borrow::Cow<str>, _)| name.as_ref() == case_name)
                        .and_then(|(_, ty)| ty.as_ref())
                        .ok_or_else(|| WasmError::TypeMismatch {
                            keyspace_type: format!("variant case '{}' has no payload type", case_name),
                        })?;

                    Some(val_to_wave(p, case_type)?)
                }
                None => None,
            };

            Value::make_variant(wave_type, case_name, payload_wave).map_err(|e| {
                WasmError::TypeMismatch {
                    keyspace_type: format!("failed to construct variant: {}", e),
                }
            })
        }

        Val::Flags(active) => {
            let flags: Vec<&str> = active.iter().map(|s| s.as_str()).collect();
            Value::make_flags(wave_type, flags).map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to construct flags: {}", e),
            })
        }

        Val::Result(result) => {
            let (ok_type, err_type) = wave_type.result_types().ok_or_else(|| {
                WasmError::TypeMismatch {
                    keyspace_type: "expected result type".to_string(),
                }
            })?;

            match result {
                Ok(ok_val) => {
                    let ok_wave = match (ok_val, ok_type) {
                        (Some(v), Some(ty)) => Some(val_to_wave(v, &ty)?),
                        (None, None) => None,
                        _ => {
                            return Err(WasmError::TypeMismatch {
                                keyspace_type: "result ok payload mismatch".to_string(),
                            })
                        }
                    };
                    Value::make_result(wave_type, Ok(ok_wave)).map_err(|e| {
                        WasmError::TypeMismatch {
                            keyspace_type: format!("failed to construct result: {}", e),
                        }
                    })
                }
                Err(err_val) => {
                    let err_wave = match (err_val, err_type) {
                        (Some(v), Some(ty)) => Some(val_to_wave(v, &ty)?),
                        (None, None) => None,
                        _ => {
                            return Err(WasmError::TypeMismatch {
                                keyspace_type: "result err payload mismatch".to_string(),
                            })
                        }
                    };
                    Value::make_result(wave_type, Err(err_wave)).map_err(|e| {
                        WasmError::TypeMismatch {
                            keyspace_type: format!("failed to construct result: {}", e),
                        }
                    })
                }
            }
        }

        _ => Err(WasmError::TypeMismatch {
            keyspace_type: "unsupported Val type for conversion".to_string(),
        }),
    }
}

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
    wit_path: Option<PathBuf>,
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
    /// This is required and must point to a valid `.wasm` component file.
    pub fn component(mut self, path: impl AsRef<Path>) -> Self {
        self.component_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the path to the WIT file defining the types.
    ///
    /// This is required and must point to a valid `.wit` file containing
    /// the type definitions for the component.
    pub fn wit(mut self, path: impl AsRef<Path>) -> Self {
        self.wit_path = Some(path.as_ref().to_path_buf());
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
    /// - Required fields (component, wit, input_type) are not set
    /// - The component file cannot be loaded
    /// - The WIT file cannot be parsed
    /// - The specified types are not found in the WIT file
    pub fn build(self) -> Result<TypedRunner, WasmError> {
        let component_path = self.component_path.ok_or_else(|| WasmError::TypeMismatch {
            keyspace_type: "component path is required".to_string(),
        })?;

        let wit_path = self.wit_path.ok_or_else(|| WasmError::TypeMismatch {
            keyspace_type: "WIT path is required".to_string(),
        })?;

        let input_type_name = self.input_type_name.ok_or_else(|| WasmError::TypeMismatch {
            keyspace_type: "input type name is required".to_string(),
        })?;

        TypedRunner::new(
            &component_path,
            &wit_path,
            &input_type_name,
            self.output_type_name.as_deref(),
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

    /// Get the input type wave representation.
    pub fn input_wave_type(&self) -> Result<wasm_wave::value::Type, WasmError> {
        crate::resolve_wit_type(&self.resolve, self.input_type_id)
            .map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to resolve input type: {}", e),
            })
    }

    /// Get the output type wave representation.
    pub fn output_wave_type(&self) -> Result<wasm_wave::value::Type, WasmError> {
        crate::resolve_wit_type(&self.resolve, self.output_type_id)
            .map_err(|e| WasmError::TypeMismatch {
                keyspace_type: format!("failed to resolve output type: {}", e),
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
    pub fn val_to_stored(&self, val: &Val, type_version: SemanticVersion) -> Result<StoredValue, WasmError> {
        let mut memory = LinearMemory::new();
        let abi = CanonicalAbi::new(&self.resolve);
        let buffer = abi.lower_from_val(
            val,
            &wit_parser::Type::Id(self.output_type_id),
            &mut memory,
        )?;

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
        let func = self.get_func("filter")?;

        // Get function type to determine parameter type
        let func_type = func.ty(&self.store);
        let (_, param_type) = func_type.params().next().ok_or_else(|| {
            WasmError::InvalidReturnType {
                expected: "filter function should have 1 parameter".to_string(),
            }
        })?;

        // Convert stored value to wasmtime Val
        let input_val = self.stored_to_val(stored, &param_type)?;

        // Call function
        let mut results = vec![Val::Bool(false)];
        func.call(&mut self.store, &[input_val], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

        match results.first() {
            Some(Val::Bool(b)) => Ok(*b),
            Some(other) => Err(WasmError::InvalidReturnType {
                expected: format!("bool, got {:?}", other),
            }),
            None => Err(WasmError::InvalidReturnType {
                expected: "bool, got no result".to_string(),
            }),
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
        let func = self.get_func("transform")?;

        // Get function type
        let func_type = func.ty(&self.store);
        let (_, param_type) = func_type.params().next().ok_or_else(|| {
            WasmError::InvalidReturnType {
                expected: "transform function should have 1 parameter".to_string(),
            }
        })?;

        let result_type = func_type.results().next().ok_or_else(|| {
            WasmError::InvalidReturnType {
                expected: "transform function should have 1 result".to_string(),
            }
        })?;

        // Convert input
        let input_val = self.stored_to_val(stored, &param_type)?;

        // Create result placeholder
        let mut results = vec![create_placeholder_val(&result_type)?];

        // Call function
        func.call(&mut self.store, &[input_val], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        // Convert result to StoredValue
        let result_val = results.first().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "transform function should return a value".to_string(),
        })?;
        let output = self.val_to_stored(result_val, type_version)?;

        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

        Ok(output)
    }

    /// Get a reference to the engine (useful for type introspection).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Call the `init-state` function to get the initial reduce state.
    ///
    /// The init-state function should have signature: `init-state() -> StateType`
    pub fn call_init_state(&mut self, type_version: SemanticVersion) -> Result<StoredValue, WasmError> {
        let func = self.get_func("init-state")?;

        // Get function type
        let func_type = func.ty(&self.store);
        let result_type = func_type.results().next().ok_or_else(|| {
            WasmError::InvalidReturnType {
                expected: "init-state function should have 1 result".to_string(),
            }
        })?;

        // Create result placeholder
        let mut results = vec![create_placeholder_val(&result_type)?];

        // Call function (no parameters)
        func.call(&mut self.store, &[], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        // Convert result to StoredValue using output_type (which is the state type)
        let result_val = results.first().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "init-state function should return a value".to_string(),
        })?;
        let output = self.val_to_stored(result_val, type_version)?;

        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

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
        let func = self.get_func("reduce")?;

        // Get function type
        let func_type = func.ty(&self.store);
        let mut params = func_type.params();

        let (_, state_param_type) = params.next().ok_or_else(|| {
            WasmError::InvalidReturnType {
                expected: "reduce function should have 2 parameters (state, value)".to_string(),
            }
        })?;

        let (_, value_param_type) = params.next().ok_or_else(|| {
            WasmError::InvalidReturnType {
                expected: "reduce function should have 2 parameters (state, value)".to_string(),
            }
        })?;

        let result_type = func_type.results().next().ok_or_else(|| {
            WasmError::InvalidReturnType {
                expected: "reduce function should have 1 result".to_string(),
            }
        })?;

        // Convert state and value to wasmtime Vals
        let state_val = self.state_to_val(state, &state_param_type)?;
        let value_val = self.stored_to_val(value, &value_param_type)?;

        // Create result placeholder
        let mut results = vec![create_placeholder_val(&result_type)?];

        // Call function
        func.call(&mut self.store, &[state_val, value_val], &mut results)
            .map_err(|e| WasmError::Trap(e.to_string()))?;

        // Convert result to StoredValue
        let result_val = results.first().ok_or_else(|| WasmError::InvalidReturnType {
            expected: "reduce function should return a value".to_string(),
        })?;
        let output = self.val_to_stored(result_val, type_version)?;

        func.post_return(&mut self.store)
            .map_err(|e| WasmError::Trap(format!("post_return failed: {}", e)))?;

        Ok(output)
    }
}
