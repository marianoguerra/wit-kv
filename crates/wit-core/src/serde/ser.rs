use std::borrow::Cow;

use serde::ser::{self, Serialize};
use wasm_wave::wasm::{WasmType, WasmTypeKind, WasmValue};

use crate::{Value, WaveType};

use super::error::WitSerdeError;

/// Schema-aware serializer that produces `wasm_wave::Value`.
///
/// Requires a `WaveType` to construct properly-typed container values
/// (records, lists, options, etc.).
pub(crate) struct WitSerializer<'a> {
    wave_type: &'a WaveType,
}

impl<'a> WitSerializer<'a> {
    pub(crate) fn new(wave_type: &'a WaveType) -> Self {
        Self { wave_type }
    }
}

impl<'a> ser::Serializer for WitSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;
    type SerializeSeq = SeqSerializer<'a>;
    type SerializeTuple = SeqSerializer<'a>;
    type SerializeTupleStruct = SeqSerializer<'a>;
    type SerializeTupleVariant = TupleVariantSerializer<'a>;
    type SerializeMap = MapSerializer<'a>;
    type SerializeStruct = StructSerializer<'a>;
    type SerializeStructVariant = StructVariantSerializer<'a>;

    fn serialize_bool(self, v: bool) -> Result<Value, Self::Error> {
        Ok(Value::make_bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Value, Self::Error> {
        Ok(Value::make_s8(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Value, Self::Error> {
        Ok(Value::make_s16(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Value, Self::Error> {
        Ok(Value::make_s32(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Value, Self::Error> {
        Ok(Value::make_s64(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Value, Self::Error> {
        Ok(Value::make_u8(v))
    }

    fn serialize_u16(self, v: u16) -> Result<Value, Self::Error> {
        Ok(Value::make_u16(v))
    }

    fn serialize_u32(self, v: u32) -> Result<Value, Self::Error> {
        Ok(Value::make_u32(v))
    }

    fn serialize_u64(self, v: u64) -> Result<Value, Self::Error> {
        Ok(Value::make_u64(v))
    }

    fn serialize_f32(self, v: f32) -> Result<Value, Self::Error> {
        Ok(Value::make_f32(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Value, Self::Error> {
        Ok(Value::make_f64(v))
    }

    fn serialize_char(self, v: char) -> Result<Value, Self::Error> {
        Ok(Value::make_char(v))
    }

    fn serialize_str(self, v: &str) -> Result<Value, Self::Error> {
        Ok(Value::make_string(Cow::Owned(v.to_string())))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Value, Self::Error> {
        // Serialize as list<u8>
        let vals: Vec<Value> = v.iter().map(|&b| Value::make_u8(b)).collect();
        Value::make_list(self.wave_type, vals)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }

    fn serialize_none(self) -> Result<Value, Self::Error> {
        Value::make_option(self.wave_type, None)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Value, Self::Error> {
        let some_type = self.wave_type.option_some_type().ok_or_else(|| {
            WitSerdeError::TypeMismatch {
                expected: "option type",
                actual: format!("{:?}", self.wave_type.kind()),
            }
        })?;
        let inner = value.serialize(WitSerializer::new(&some_type))?;
        Value::make_option(self.wave_type, Some(inner))
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }

    fn serialize_unit(self) -> Result<Value, Self::Error> {
        // Unit maps to an empty tuple
        Value::make_tuple(self.wave_type, std::iter::empty::<Value>())
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, Self::Error> {
        match self.wave_type.kind() {
            WasmTypeKind::Enum => Value::make_enum(self.wave_type, variant)
                .map_err(|e| WitSerdeError::ValueConstruction(e.to_string())),
            WasmTypeKind::Result => {
                // Result<(), ()> variant — "Ok" or "Err" with no payload
                let result = if variant == "Ok" {
                    Ok(None)
                } else {
                    Err(None)
                };
                Value::make_result(self.wave_type, result)
                    .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
            }
            WasmTypeKind::Option => {
                // Option as enum: "None" variant
                Value::make_option(self.wave_type, None)
                    .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
            }
            _ => Value::make_variant(self.wave_type, variant, None)
                .map_err(|e| WitSerdeError::ValueConstruction(e.to_string())),
        }
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Self::Error> {
        match self.wave_type.kind() {
            WasmTypeKind::Result => {
                let (ok_type, err_type) = self
                    .wave_type
                    .result_types()
                    .ok_or_else(|| WitSerdeError::TypeMismatch {
                        expected: "result type",
                        actual: format!("{:?}", self.wave_type.kind()),
                    })?;
                if variant == "Ok" {
                    let inner_type = ok_type.ok_or_else(|| {
                        WitSerdeError::TypeMismatch {
                            expected: "result ok type",
                            actual: "result<_, E> with no ok type".to_string(),
                        }
                    })?;
                    let inner = value.serialize(WitSerializer::new(&inner_type))?;
                    Value::make_result(self.wave_type, Ok(Some(inner)))
                        .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
                } else {
                    let inner_type = err_type.ok_or_else(|| {
                        WitSerdeError::TypeMismatch {
                            expected: "result err type",
                            actual: "result<T, _> with no err type".to_string(),
                        }
                    })?;
                    let inner = value.serialize(WitSerializer::new(&inner_type))?;
                    Value::make_result(self.wave_type, Err(Some(inner)))
                        .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
                }
            }
            WasmTypeKind::Option => {
                // Option as enum: "Some" variant
                let some_type =
                    self.wave_type.option_some_type().ok_or_else(|| {
                        WitSerdeError::TypeMismatch {
                            expected: "option type",
                            actual: format!("{:?}", self.wave_type.kind()),
                        }
                    })?;
                let inner = value.serialize(WitSerializer::new(&some_type))?;
                Value::make_option(self.wave_type, Some(inner))
                    .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
            }
            _ => {
                // WIT variant with payload — find the case's type
                let case_type = find_variant_case_type(self.wave_type, variant)?;
                let inner = match &case_type {
                    Some(ty) => value.serialize(WitSerializer::new(ty))?,
                    None => return Err(WitSerdeError::TypeMismatch {
                        expected: "variant case with payload",
                        actual: format!("variant case '{variant}' has no payload type"),
                    }),
                };
                Value::make_variant(self.wave_type, variant, Some(inner))
                    .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
            }
        }
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let element_type = self.wave_type.list_element_type().ok_or_else(|| {
            WitSerdeError::TypeMismatch {
                expected: "list type",
                actual: format!("{:?}", self.wave_type.kind()),
            }
        })?;
        Ok(SeqSerializer {
            wave_type: self.wave_type,
            element_type,
            elements: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        let element_types: Vec<WaveType> = self.wave_type.tuple_element_types().collect();
        if element_types.is_empty() && len > 0 {
            // Fall back to list if the wave type isn't a tuple
            return self.serialize_seq(Some(len));
        }
        Ok(SeqSerializer {
            wave_type: self.wave_type,
            element_type: WaveType::BOOL, // placeholder, tuple uses per-element types
            elements: Vec::with_capacity(len),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_tuple(len)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let case_type = find_variant_case_type(self.wave_type, variant)?;
        Ok(TupleVariantSerializer {
            wave_type: self.wave_type,
            variant,
            case_type,
            elements: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(MapSerializer {
            wave_type: self.wave_type,
            fields: Vec::with_capacity(len.unwrap_or(0)),
            current_key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(StructSerializer {
            wave_type: self.wave_type,
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let case_type = find_variant_case_type(self.wave_type, variant)?;
        Ok(StructVariantSerializer {
            wave_type: self.wave_type,
            variant,
            case_type,
            fields: Vec::with_capacity(len),
        })
    }
}

// ---------------------------------------------------------------------------
// Helper: find variant case type by name
// ---------------------------------------------------------------------------

fn find_variant_case_type(
    wave_type: &WaveType,
    case_name: &str,
) -> Result<Option<WaveType>, WitSerdeError> {
    for (name, ty) in wave_type.variant_cases() {
        if *name == *case_name {
            return Ok(ty);
        }
    }
    Err(WitSerdeError::ValueConstruction(format!(
        "variant case '{case_name}' not found in type"
    )))
}

// ---------------------------------------------------------------------------
// Helper: find record field type by name
// ---------------------------------------------------------------------------

fn find_record_field_type(
    wave_type: &WaveType,
    field_name: &str,
) -> Result<WaveType, WitSerdeError> {
    for (name, ty) in wave_type.record_fields() {
        if *name == *field_name {
            return Ok(ty);
        }
    }
    Err(WitSerdeError::ValueConstruction(format!(
        "record field '{field_name}' not found in type"
    )))
}

// ---------------------------------------------------------------------------
// SeqSerializer for lists and tuples
// ---------------------------------------------------------------------------

pub(crate) struct SeqSerializer<'a> {
    wave_type: &'a WaveType,
    element_type: WaveType,
    elements: Vec<Value>,
}

impl<'a> ser::SerializeSeq for SeqSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let val = value.serialize(WitSerializer::new(&self.element_type))?;
        self.elements.push(val);
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        Value::make_list(self.wave_type, self.elements)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }
}

impl<'a> ser::SerializeTuple for SeqSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let element_types: Vec<WaveType> = self.wave_type.tuple_element_types().collect();
        let idx = self.elements.len();
        let elem_type = element_types.get(idx).unwrap_or(&self.element_type);
        let val = value.serialize(WitSerializer::new(elem_type))?;
        self.elements.push(val);
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        Value::make_tuple(self.wave_type, self.elements)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }
}

impl<'a> ser::SerializeTupleStruct for SeqSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        ser::SerializeTuple::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Self::Error> {
        ser::SerializeTuple::end(self)
    }
}

// ---------------------------------------------------------------------------
// TupleVariantSerializer
// ---------------------------------------------------------------------------

pub(crate) struct TupleVariantSerializer<'a> {
    wave_type: &'a WaveType,
    variant: &'static str,
    case_type: Option<WaveType>,
    elements: Vec<Value>,
}

impl<'a> ser::SerializeTupleVariant for TupleVariantSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let ty = self.case_type.as_ref().ok_or_else(|| {
            WitSerdeError::TypeMismatch {
                expected: "variant with payload",
                actual: "variant without payload type".to_string(),
            }
        })?;
        let val = value.serialize(WitSerializer::new(ty))?;
        self.elements.push(val);
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        // For a tuple variant with a single element, use it directly as payload.
        // For multiple, this would need a tuple type — but WIT variants typically
        // have a single payload type.
        let payload = if self.elements.len() == 1 {
            self.elements.into_iter().next()
        } else {
            // Multiple fields: would need tuple construction
            // This is uncommon in WIT variants
            return Err(WitSerdeError::Message(
                "WIT variants with multiple tuple fields are not supported".to_string(),
            ));
        };
        Value::make_variant(self.wave_type, self.variant, payload)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// MapSerializer for records
// ---------------------------------------------------------------------------

pub(crate) struct MapSerializer<'a> {
    wave_type: &'a WaveType,
    fields: Vec<(String, Value)>,
    current_key: Option<String>,
}

impl<'a> ser::SerializeMap for MapSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        // We need the key as a string. Use a small string-extracting serializer.
        let key_str = key.serialize(StringExtractor)?;
        self.current_key = Some(key_str);
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let key = self.current_key.take().ok_or_else(|| {
            WitSerdeError::Message("serialize_value called before serialize_key".to_string())
        })?;
        let field_type = find_record_field_type(self.wave_type, &key)?;
        let val = value.serialize(WitSerializer::new(&field_type))?;
        self.fields.push((key, val));
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        let fields: Vec<(&str, Value)> = self.fields.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        Value::make_record(self.wave_type, fields)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// StructSerializer for records
// ---------------------------------------------------------------------------

pub(crate) struct StructSerializer<'a> {
    wave_type: &'a WaveType,
    fields: Vec<(&'static str, Value)>,
}

impl<'a> ser::SerializeStruct for StructSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        let field_type = find_record_field_type(self.wave_type, key)?;
        let val = value.serialize(WitSerializer::new(&field_type))?;
        self.fields.push((key, val));
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        Value::make_record(self.wave_type, self.fields)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// StructVariantSerializer
// ---------------------------------------------------------------------------

pub(crate) struct StructVariantSerializer<'a> {
    wave_type: &'a WaveType,
    variant: &'static str,
    case_type: Option<WaveType>,
    fields: Vec<(&'static str, Value)>,
}

impl<'a> ser::SerializeStructVariant for StructVariantSerializer<'a> {
    type Ok = Value;
    type Error = WitSerdeError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        let case_type = self.case_type.as_ref().ok_or_else(|| {
            WitSerdeError::TypeMismatch {
                expected: "variant with record payload",
                actual: "variant without payload type".to_string(),
            }
        })?;
        let field_type = find_record_field_type(case_type, key)?;
        let val = value.serialize(WitSerializer::new(&field_type))?;
        self.fields.push((key, val));
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        let case_type = self.case_type.as_ref().ok_or_else(|| {
            WitSerdeError::TypeMismatch {
                expected: "variant with record payload",
                actual: "variant without payload type".to_string(),
            }
        })?;
        let payload = Value::make_record(case_type, self.fields)
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))?;
        Value::make_variant(self.wave_type, self.variant, Some(payload))
            .map_err(|e| WitSerdeError::ValueConstruction(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// StringExtractor: tiny serializer that only handles strings
// ---------------------------------------------------------------------------

struct StringExtractor;

impl ser::Serializer for StringExtractor {
    type Ok = String;
    type Error = WitSerdeError;
    type SerializeSeq = ser::Impossible<String, WitSerdeError>;
    type SerializeTuple = ser::Impossible<String, WitSerdeError>;
    type SerializeTupleStruct = ser::Impossible<String, WitSerdeError>;
    type SerializeTupleVariant = ser::Impossible<String, WitSerdeError>;
    type SerializeMap = ser::Impossible<String, WitSerdeError>;
    type SerializeStruct = ser::Impossible<String, WitSerdeError>;
    type SerializeStructVariant = ser::Impossible<String, WitSerdeError>;

    fn serialize_str(self, v: &str) -> Result<String, Self::Error> {
        Ok(v.to_string())
    }

    fn serialize_bool(self, _: bool) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_i8(self, _: i8) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_i16(self, _: i16) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_i32(self, _: i32) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_i64(self, _: i64) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_u8(self, _: u8) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_u16(self, _: u16) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_u32(self, _: u32) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_u64(self, _: u64) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_f32(self, _: f32) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_f64(self, _: f64) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_char(self, _: char) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_none(self) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_unit(self) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, _: &T) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _: &'static str, _: u32, _: &'static str, _: &T) -> Result<String, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(WitSerdeError::Message("expected string key".to_string()))
    }
}

pub(crate) fn serialize_value<T: Serialize>(
    value: &T,
    wave_type: &WaveType,
) -> Result<Value, WitSerdeError> {
    value.serialize(WitSerializer::new(wave_type))
}
