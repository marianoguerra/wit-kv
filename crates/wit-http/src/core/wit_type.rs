//! Rust struct <-> WIT value bridge via serde.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::de::DeserializeOwned;
use serde::Serialize;
use wit_core::ResolvedType;

use super::error::ResourceError;

/// Trait for Rust types that correspond to a WIT type definition.
///
/// Implementors declare their WIT schema and derive `Serialize` + `Deserialize`.
/// The framework handles conversion to/from `wasm_wave::Value` via serde,
/// plus binary encoding, content negotiation, and HTTP serialization.
///
/// # Example
///
/// ```ignore
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct User {
///     name: String,
///     email: String,
///     age: u32,
/// }
///
/// impl WitType for User {
///     fn wit_definition() -> &'static str {
///         r#"
///         package app:types;
///         interface types {
///             record user { name: string, email: string, age: u32 }
///         }
///         "#
///     }
///
///     fn type_name() -> &'static str { "user" }
/// }
/// ```
pub trait WitType: Send + Sync + Sized + 'static + Serialize + DeserializeOwned {
    /// The WIT definition text that describes this type.
    fn wit_definition() -> &'static str;

    /// The type name within the WIT definition.
    fn type_name() -> &'static str;

    /// Get the resolved type (lazily cached per concrete type).
    ///
    /// The default implementation loads from `wit_definition()` + `type_name()`
    /// and caches the result globally, keyed by the concrete type's `TypeId`.
    fn resolved_type() -> Result<Arc<ResolvedType>, ResourceError> {
        static CACHE: OnceLock<Mutex<HashMap<TypeId, Arc<ResolvedType>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let tid = TypeId::of::<Self>();

        {
            let guard = cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(resolved) = guard.get(&tid) {
                return Ok(resolved.clone());
            }
        }

        let resolved = Arc::new(
            wit_core::load_wit_type_from_string(Self::wit_definition(), Some(Self::type_name()))
                .map_err(|e| ResourceError::TypeError(e.to_string()))?,
        );

        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        Ok(guard.entry(tid).or_insert(resolved).clone())
    }
}
