//! Serde adapter for `wasm_wave::Value`.
//!
//! Provides automatic conversion between Rust types (via serde) and
//! WIT values, following the apache-avro pattern of `from_value` / `to_value`.

mod de;
mod error;
mod ser;

pub use error::WitSerdeError;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Value, WaveType};

/// Deserialize a `wasm_wave::Value` into a Rust type.
///
/// The Value carries type info (record field names, variant case names, etc.)
/// so no schema is needed for deserialization.
///
/// # Example
///
/// ```ignore
/// use wit_core::serde::from_value;
///
/// #[derive(serde::Deserialize)]
/// struct Point { x: u32, y: u32 }
///
/// let value: Value = wave_from_str(&wave_type, "{x: 1, y: 2}")?;
/// let point: Point = from_value(&value)?;
/// assert_eq!(point.x, 1);
/// ```
pub fn from_value<T: DeserializeOwned>(value: &Value) -> Result<T, WitSerdeError> {
    de::deserialize_value(value)
}

/// Serialize a Rust type into a `wasm_wave::Value`.
///
/// Requires a `WaveType` (schema) because `Value::make_record()` and other
/// container constructors need it to produce properly-typed values.
///
/// # Example
///
/// ```ignore
/// use wit_core::serde::to_value;
///
/// #[derive(serde::Serialize)]
/// struct Point { x: u32, y: u32 }
///
/// let point = Point { x: 1, y: 2 };
/// let value = to_value(&point, &wave_type)?;
/// ```
pub fn to_value<T: Serialize>(value: &T, wave_type: &WaveType) -> Result<Value, WitSerdeError> {
    ser::serialize_value(value, wave_type)
}
