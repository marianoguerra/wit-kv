//! Conversion functions between wasmtime::component::Val and wasm_wave::Value.
//!
//! These functions enable converting between the two value representations,
//! useful for TypedRunner to convert between wasmtime's runtime values and
//! WAVE text format for display.

use thiserror::Error;
use wasmtime::component::{types, Val};
use wasm_wave::value::Value;
use wasm_wave::wasm::{WasmType, WasmValue};

/// Errors that can occur during Val <-> Value conversion.
#[derive(Error, Debug)]
pub enum ValConvertError {
    /// Type mismatch during conversion.
    #[error("Type mismatch: {0}")]
    TypeMismatch(String),

    /// Failed to construct a wasm_wave value.
    #[error("Failed to construct value: {0}")]
    ConstructionFailed(String),
}

/// Convert a wasm_wave::Value to a wasmtime::component::Val based on the target type.
pub fn wave_to_val(wave: &Value, target_type: &types::Type) -> Result<Val, ValConvertError> {
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
                    .ok_or_else(|| ValConvertError::TypeMismatch(
                        format!("field '{}' not found", field.name),
                    ))?;

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
                .ok_or_else(|| ValConvertError::TypeMismatch(
                    format!("variant case '{}' not found", case_name),
                ))?;

            let payload_val = match (payload, &case.ty) {
                (Some(p), Some(ty)) => Some(Box::new(wave_to_val(&p, ty)?)),
                (None, None) => None,
                _ => {
                    return Err(ValConvertError::TypeMismatch(
                        "variant payload mismatch".to_string(),
                    ))
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
                        return Err(ValConvertError::TypeMismatch(
                            "result ok payload mismatch".to_string(),
                        ))
                    }
                };
                Ok(Val::Result(Ok(ok_inner)))
            }
            Err(err_val) => {
                let err_inner = match (err_val, result_type.err()) {
                    (Some(v), Some(ty)) => Some(Box::new(wave_to_val(&v, &ty)?)),
                    (None, None) => None,
                    _ => {
                        return Err(ValConvertError::TypeMismatch(
                            "result err payload mismatch".to_string(),
                        ))
                    }
                };
                Ok(Val::Result(Err(err_inner)))
            }
        },

        _ => Err(ValConvertError::TypeMismatch(
            "unsupported type for conversion".to_string(),
        )),
    }
}

/// Convert a wasmtime::component::Val back to wasm_wave::Value.
pub fn val_to_wave(
    val: &Val,
    wave_type: &wasm_wave::value::Type,
) -> Result<Value, ValConvertError> {
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
                return Err(ValConvertError::TypeMismatch(
                    "expected record type".to_string(),
                ));
            }

            let wave_fields: Result<Vec<_>, ValConvertError> = fields
                .iter()
                .map(|(name, val)| {
                    // Get the field type from wave_type
                    let field_type = field_types
                        .iter()
                        .find(|(n, _): &&(std::borrow::Cow<str>, _)| n.as_ref() == name)
                        .map(|(_, ty)| ty)
                        .ok_or_else(|| ValConvertError::TypeMismatch(
                            format!("field '{}' not found in wave type", name),
                        ))?;

                    let wave_val = val_to_wave(val, field_type)?;
                    Ok((name.as_str(), wave_val))
                })
                .collect();

            Value::make_record(wave_type, wave_fields?).map_err(|e| ValConvertError::ConstructionFailed(
                format!("failed to construct record: {}", e),
            ))
        }

        Val::List(elements) => {
            let elem_type = wave_type
                .list_element_type()
                .ok_or_else(|| ValConvertError::TypeMismatch(
                    "expected list type".to_string(),
                ))?;

            let wave_elems: Result<Vec<_>, _> = elements
                .iter()
                .map(|e| val_to_wave(e, &elem_type))
                .collect();

            Value::make_list(wave_type, wave_elems?).map_err(|e| ValConvertError::ConstructionFailed(
                format!("failed to construct list: {}", e),
            ))
        }

        Val::Option(opt) => {
            let inner = match opt {
                Some(inner_val) => {
                    let inner_type = wave_type.option_some_type().ok_or_else(|| {
                        ValConvertError::TypeMismatch(
                            "expected option type".to_string(),
                        )
                    })?;
                    Some(val_to_wave(inner_val, &inner_type)?)
                }
                None => None,
            };
            Value::make_option(wave_type, inner).map_err(|e| ValConvertError::ConstructionFailed(
                format!("failed to construct option: {}", e),
            ))
        }

        Val::Tuple(elements) => {
            let tuple_types: Vec<_> = wave_type.tuple_element_types().collect();
            if tuple_types.is_empty() && !elements.is_empty() {
                return Err(ValConvertError::TypeMismatch(
                    "expected tuple type".to_string(),
                ));
            }

            let wave_elems: Result<Vec<_>, _> = elements
                .iter()
                .zip(tuple_types.iter())
                .map(|(e, ty)| val_to_wave(e, ty))
                .collect();

            Value::make_tuple(wave_type, wave_elems?).map_err(|e| ValConvertError::ConstructionFailed(
                format!("failed to construct tuple: {}", e),
            ))
        }

        Val::Enum(case_name) => {
            Value::make_enum(wave_type, case_name).map_err(|e| ValConvertError::ConstructionFailed(
                format!("failed to construct enum: {}", e),
            ))
        }

        Val::Variant(case_name, payload) => {
            // Collect variant cases first
            let variant_cases: Vec<_> = wave_type.variant_cases().collect();
            if variant_cases.is_empty() {
                return Err(ValConvertError::TypeMismatch(
                    "expected variant type".to_string(),
                ));
            }

            let payload_wave = match payload {
                Some(p) => {
                    // Get the payload type for this case
                    let case_type = variant_cases
                        .iter()
                        .find(|(name, _): &&(std::borrow::Cow<str>, _)| name.as_ref() == case_name)
                        .and_then(|(_, ty)| ty.as_ref())
                        .ok_or_else(|| ValConvertError::TypeMismatch(
                            format!("variant case '{}' has no payload type", case_name),
                        ))?;

                    Some(val_to_wave(p, case_type)?)
                }
                None => None,
            };

            Value::make_variant(wave_type, case_name, payload_wave).map_err(|e| {
                ValConvertError::ConstructionFailed(
                    format!("failed to construct variant: {}", e),
                )
            })
        }

        Val::Flags(active) => {
            let flags: Vec<&str> = active.iter().map(|s| s.as_str()).collect();
            Value::make_flags(wave_type, flags).map_err(|e| ValConvertError::ConstructionFailed(
                format!("failed to construct flags: {}", e),
            ))
        }

        Val::Result(result) => {
            let (ok_type, err_type) = wave_type.result_types().ok_or_else(|| {
                ValConvertError::TypeMismatch(
                    "expected result type".to_string(),
                )
            })?;

            match result {
                Ok(ok_val) => {
                    let ok_wave = match (ok_val, ok_type) {
                        (Some(v), Some(ty)) => Some(val_to_wave(v, &ty)?),
                        (None, None) => None,
                        _ => {
                            return Err(ValConvertError::TypeMismatch(
                                "result ok payload mismatch".to_string(),
                            ))
                        }
                    };
                    Value::make_result(wave_type, Ok(ok_wave)).map_err(|e| {
                        ValConvertError::ConstructionFailed(
                            format!("failed to construct result: {}", e),
                        )
                    })
                }
                Err(err_val) => {
                    let err_wave = match (err_val, err_type) {
                        (Some(v), Some(ty)) => Some(val_to_wave(v, &ty)?),
                        (None, None) => None,
                        _ => {
                            return Err(ValConvertError::TypeMismatch(
                                "result err payload mismatch".to_string(),
                            ))
                        }
                    };
                    Value::make_result(wave_type, Err(err_wave)).map_err(|e| {
                        ValConvertError::ConstructionFailed(
                            format!("failed to construct result: {}", e),
                        )
                    })
                }
            }
        }

        _ => Err(ValConvertError::TypeMismatch(
            "unsupported Val type for conversion".to_string(),
        )),
    }
}
