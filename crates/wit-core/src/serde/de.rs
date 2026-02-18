use serde::de::{
    self, DeserializeSeed, Deserializer, IntoDeserializer, MapAccess, SeqAccess, Visitor,
};
use wasm_wave::wasm::{WasmTypeKind, WasmValue};

use crate::Value;

use super::error::WitSerdeError;

/// Deserializer adapter from `wasm_wave::Value` into serde types.
///
/// Uses owned `Value` throughout to avoid lifetime issues with serde's
/// consuming `Deserializer` trait. Since our public API uses `DeserializeOwned`,
/// this is the appropriate approach.
struct ValueDeserializer {
    value: Value,
}

impl ValueDeserializer {
    fn new(value: Value) -> Self {
        Self { value }
    }

    fn kind(&self) -> WasmTypeKind {
        self.value.kind()
    }
}

impl<'de> de::Deserializer<'de> for ValueDeserializer {
    type Error = WitSerdeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.kind() {
            WasmTypeKind::Bool => visitor.visit_bool(self.value.unwrap_bool()),
            WasmTypeKind::S8 => visitor.visit_i8(self.value.unwrap_s8()),
            WasmTypeKind::S16 => visitor.visit_i16(self.value.unwrap_s16()),
            WasmTypeKind::S32 => visitor.visit_i32(self.value.unwrap_s32()),
            WasmTypeKind::S64 => visitor.visit_i64(self.value.unwrap_s64()),
            WasmTypeKind::U8 => visitor.visit_u8(self.value.unwrap_u8()),
            WasmTypeKind::U16 => visitor.visit_u16(self.value.unwrap_u16()),
            WasmTypeKind::U32 => visitor.visit_u32(self.value.unwrap_u32()),
            WasmTypeKind::U64 => visitor.visit_u64(self.value.unwrap_u64()),
            WasmTypeKind::F32 => visitor.visit_f32(self.value.unwrap_f32()),
            WasmTypeKind::F64 => visitor.visit_f64(self.value.unwrap_f64()),
            WasmTypeKind::Char => visitor.visit_char(self.value.unwrap_char()),
            WasmTypeKind::String => {
                let s = self.value.unwrap_string();
                visitor.visit_string(s.into_owned())
            }
            WasmTypeKind::List | WasmTypeKind::FixedSizeList => {
                self.deserialize_seq(visitor)
            }
            WasmTypeKind::Record => self.deserialize_map(visitor),
            WasmTypeKind::Tuple => self.deserialize_seq(visitor),
            WasmTypeKind::Option => self.deserialize_option(visitor),
            WasmTypeKind::Enum => {
                let case = self.value.unwrap_enum();
                visitor.visit_string(case.into_owned())
            }
            WasmTypeKind::Variant => {
                let (case_name, payload) = self.value.unwrap_variant();
                visitor.visit_enum(VariantDeserializer {
                    case_name: case_name.into_owned(),
                    payload: payload.map(|v| v.into_owned()),
                })
            }
            WasmTypeKind::Result => self.deserialize_enum("Result", &["Ok", "Err"], visitor),
            WasmTypeKind::Flags => {
                let flags: Vec<String> = self
                    .value
                    .unwrap_flags()
                    .map(|s| s.into_owned())
                    .collect();
                visitor.visit_seq(StringSeqDeserializer {
                    iter: flags.into_iter(),
                })
            }
            _ => Err(WitSerdeError::TypeMismatch {
                expected: "a known WIT type",
                actual: format!("{:?}", self.kind()),
            }),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_bool(self.value.unwrap_bool())
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_i8(self.value.unwrap_s8())
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_i16(self.value.unwrap_s16())
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_i32(self.value.unwrap_s32())
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_i64(self.value.unwrap_s64())
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_u8(self.value.unwrap_u8())
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_u16(self.value.unwrap_u16())
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_u32(self.value.unwrap_u32())
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_u64(self.value.unwrap_u64())
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_f32(self.value.unwrap_f32())
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_f64(self.value.unwrap_f64())
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_char(self.value.unwrap_char())
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let s = self.value.unwrap_string();
        visitor.visit_string(s.into_owned())
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let s = self.value.unwrap_string();
        visitor.visit_string(s.into_owned())
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let vals: Vec<u8> = self.value.unwrap_list().map(|v| v.unwrap_u8()).collect();
        visitor.visit_byte_buf(vals)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let vals: Vec<u8> = self.value.unwrap_list().map(|v| v.unwrap_u8()).collect();
        visitor.visit_byte_buf(vals)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value.unwrap_option() {
            Some(v) => visitor.visit_some(ValueDeserializer::new(v.into_owned())),
            None => visitor.visit_none(),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let elements: Vec<Value> = match self.kind() {
            WasmTypeKind::Tuple => {
                self.value.unwrap_tuple().map(|v| v.into_owned()).collect()
            }
            _ => {
                self.value.unwrap_list().map(|v| v.into_owned()).collect()
            }
        };
        visitor.visit_seq(ValueSeqDeserializer {
            iter: elements.into_iter(),
        })
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let fields: Vec<(String, Value)> = self
            .value
            .unwrap_record()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        visitor.visit_map(RecordMapDeserializer {
            iter: fields.into_iter(),
            current_value: None,
        })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.kind() {
            WasmTypeKind::Enum => {
                let case = self.value.unwrap_enum();
                visitor.visit_enum(case.into_owned().into_deserializer())
            }
            WasmTypeKind::Result => {
                let result = self.value.unwrap_result();
                let (case_name, payload) = match result {
                    Ok(v) => ("Ok", v.map(|v| v.into_owned())),
                    Err(v) => ("Err", v.map(|v| v.into_owned())),
                };
                visitor.visit_enum(ResultEnumDeserializer {
                    case_name,
                    payload,
                })
            }
            WasmTypeKind::Option => {
                let opt = self.value.unwrap_option();
                let (case_name, payload) = match opt {
                    Some(v) => ("Some", Some(v.into_owned())),
                    None => ("None", None),
                };
                visitor.visit_enum(ResultEnumDeserializer {
                    case_name,
                    payload,
                })
            }
            _ => {
                // Variant
                let (case_name, payload) = self.value.unwrap_variant();
                visitor.visit_enum(VariantDeserializer {
                    case_name: case_name.into_owned(),
                    payload: payload.map(|v| v.into_owned()),
                })
            }
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }
}

// ---------------------------------------------------------------------------
// SeqAccess for lists and tuples
// ---------------------------------------------------------------------------

struct ValueSeqDeserializer {
    iter: std::vec::IntoIter<Value>,
}

impl<'de> SeqAccess<'de> for ValueSeqDeserializer {
    type Error = WitSerdeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        match self.iter.next() {
            Some(value) => seed
                .deserialize(ValueDeserializer::new(value))
                .map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

// ---------------------------------------------------------------------------
// SeqAccess for flags (strings)
// ---------------------------------------------------------------------------

struct StringSeqDeserializer {
    iter: std::vec::IntoIter<String>,
}

impl<'de> SeqAccess<'de> for StringSeqDeserializer {
    type Error = WitSerdeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        match self.iter.next() {
            Some(s) => seed.deserialize(s.into_deserializer()).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

// ---------------------------------------------------------------------------
// MapAccess for records
// ---------------------------------------------------------------------------

struct RecordMapDeserializer {
    iter: std::vec::IntoIter<(String, Value)>,
    current_value: Option<Value>,
}

impl<'de> MapAccess<'de> for RecordMapDeserializer {
    type Error = WitSerdeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        match self.iter.next() {
            Some((key, value)) => {
                self.current_value = Some(value);
                seed.deserialize(key.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        let value = self.current_value.take().ok_or_else(|| {
            WitSerdeError::Message("called next_value_seed before next_key_seed".to_string())
        })?;
        seed.deserialize(ValueDeserializer::new(value))
    }
}

// ---------------------------------------------------------------------------
// EnumAccess for WIT variants
// ---------------------------------------------------------------------------

struct VariantDeserializer {
    case_name: String,
    payload: Option<Value>,
}

impl<'de> de::EnumAccess<'de> for VariantDeserializer {
    type Error = WitSerdeError;
    type Variant = VariantPayloadDeserializer;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        let variant = seed.deserialize(self.case_name.into_deserializer())?;
        Ok((
            variant,
            VariantPayloadDeserializer {
                payload: self.payload,
            },
        ))
    }
}

struct VariantPayloadDeserializer {
    payload: Option<Value>,
}

impl<'de> de::VariantAccess<'de> for VariantPayloadDeserializer {
    type Error = WitSerdeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Self::Error> {
        match self.payload {
            Some(value) => seed.deserialize(ValueDeserializer::new(value)),
            None => Err(WitSerdeError::TypeMismatch {
                expected: "newtype variant with payload",
                actual: "unit variant".to_string(),
            }),
        }
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.payload {
            Some(value) => ValueDeserializer::new(value).deserialize_seq(visitor),
            None => Err(WitSerdeError::TypeMismatch {
                expected: "tuple variant with payload",
                actual: "unit variant".to_string(),
            }),
        }
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.payload {
            Some(value) => ValueDeserializer::new(value).deserialize_map(visitor),
            None => Err(WitSerdeError::TypeMismatch {
                expected: "struct variant with payload",
                actual: "unit variant".to_string(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// EnumAccess for WIT result<T, E> and option<T> as enum
// ---------------------------------------------------------------------------

struct ResultEnumDeserializer {
    case_name: &'static str,
    payload: Option<Value>,
}

impl<'de> de::EnumAccess<'de> for ResultEnumDeserializer {
    type Error = WitSerdeError;
    type Variant = VariantPayloadDeserializer;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        let variant = seed.deserialize(self.case_name.into_deserializer())?;
        Ok((
            variant,
            VariantPayloadDeserializer {
                payload: self.payload,
            },
        ))
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub(crate) fn deserialize_value<T: serde::de::DeserializeOwned>(
    value: &Value,
) -> Result<T, WitSerdeError> {
    T::deserialize(ValueDeserializer::new(value.clone()))
}
