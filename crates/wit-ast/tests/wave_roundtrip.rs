//! Roundtrip and property-based tests for WAVE string formatting.
//!
//! These tests verify that values can be serialized to WAVE format and parsed
//! back correctly using the wasm_wave crate.
//!
//! Also includes tests for ValueTree conversion roundtrip.

use std::borrow::Cow;

use proptest::prelude::*;
use wasm_wave::value::{Type as WaveType, Value as WaveValue};
use wasm_wave::wasm::{WasmType, WasmTypeKind, WasmValue};

// ============================================================================
// ValueTree types (mirrors WIT-generated types for testing)
// ============================================================================

/// Primitive value variants (mirrors WIT primitive-value)
#[derive(Debug, Clone, PartialEq)]
enum PrimitiveValue {
    BoolVal(bool),
    U8Val(u8),
    U16Val(u16),
    U32Val(u32),
    U64Val(u64),
    S8Val(i8),
    S16Val(i16),
    S32Val(i32),
    S64Val(i64),
    F32Val(f32),
    F64Val(f64),
    CharVal(char),
    StringVal(String),
}

/// A field with name and value index (mirrors WIT field-ref)
#[derive(Debug, Clone, PartialEq)]
struct FieldRef {
    name: String,
    value_idx: u32,
}

/// A variant case with name and optional payload index (mirrors WIT variant-ref)
#[derive(Debug, Clone, PartialEq)]
struct VariantRef {
    name: String,
    payload_idx: Option<u32>,
}

/// A WIT value node (mirrors WIT wit-value-node)
#[derive(Debug, Clone, PartialEq)]
enum WitValueNode {
    Primitive(PrimitiveValue),
    RecordVal(Vec<FieldRef>),
    TupleVal(Vec<u32>),
    ListVal(Vec<u32>),
    EnumVal(String),
    VariantVal(VariantRef),
    OptionVal(Option<u32>),
    ResultVal(Result<Option<u32>, Option<u32>>),
    FlagsVal(Vec<String>),
}

/// A complete value tree (mirrors WIT value-tree)
#[derive(Debug, Clone, PartialEq)]
struct ValueTree {
    nodes: Vec<WitValueNode>,
}

// ============================================================================
// ValueTree conversion functions (mirrors src/value_convert.rs)
// ============================================================================

/// Convert a wasm_wave::Value to value-tree representation.
fn wave_to_value_tree(value: &WaveValue) -> ValueTree {
    let mut nodes = Vec::new();
    build_node(value, &mut nodes);
    ValueTree { nodes }
}

/// Recursively build nodes from a wave value.
fn build_node(value: &WaveValue, nodes: &mut Vec<WitValueNode>) -> u32 {
    let idx = nodes.len() as u32;

    match value.kind() {
        WasmTypeKind::Bool => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(
                value.unwrap_bool(),
            )));
            idx
        }
        WasmTypeKind::U8 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::U8Val(
                value.unwrap_u8(),
            )));
            idx
        }
        WasmTypeKind::U16 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::U16Val(
                value.unwrap_u16(),
            )));
            idx
        }
        WasmTypeKind::U32 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::U32Val(
                value.unwrap_u32(),
            )));
            idx
        }
        WasmTypeKind::U64 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::U64Val(
                value.unwrap_u64(),
            )));
            idx
        }
        WasmTypeKind::S8 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::S8Val(
                value.unwrap_s8(),
            )));
            idx
        }
        WasmTypeKind::S16 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::S16Val(
                value.unwrap_s16(),
            )));
            idx
        }
        WasmTypeKind::S32 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::S32Val(
                value.unwrap_s32(),
            )));
            idx
        }
        WasmTypeKind::S64 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::S64Val(
                value.unwrap_s64(),
            )));
            idx
        }
        WasmTypeKind::F32 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::F32Val(
                value.unwrap_f32(),
            )));
            idx
        }
        WasmTypeKind::F64 => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::F64Val(
                value.unwrap_f64(),
            )));
            idx
        }
        WasmTypeKind::Char => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::CharVal(
                value.unwrap_char(),
            )));
            idx
        }
        WasmTypeKind::String => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::StringVal(
                value.unwrap_string().to_string(),
            )));
            idx
        }
        WasmTypeKind::Record => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let field_refs: Vec<_> = value
                .unwrap_record()
                .map(|(name, val)| {
                    let value_idx = build_node(val.as_ref(), nodes);
                    FieldRef {
                        name: name.to_string(),
                        value_idx,
                    }
                })
                .collect();
            nodes[idx as usize] = WitValueNode::RecordVal(field_refs);
            idx
        }
        WasmTypeKind::Tuple => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let indices: Vec<_> = value
                .unwrap_tuple()
                .map(|v| build_node(v.as_ref(), nodes))
                .collect();
            nodes[idx as usize] = WitValueNode::TupleVal(indices);
            idx
        }
        WasmTypeKind::List | WasmTypeKind::FixedSizeList => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let indices: Vec<_> = value
                .unwrap_list()
                .map(|v| build_node(v.as_ref(), nodes))
                .collect();
            nodes[idx as usize] = WitValueNode::ListVal(indices);
            idx
        }
        WasmTypeKind::Enum => {
            let name = value.unwrap_enum().to_string();
            nodes.push(WitValueNode::EnumVal(name));
            idx
        }
        WasmTypeKind::Variant => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let (name, payload) = value.unwrap_variant();
            let payload_idx = payload.map(|p| build_node(p.as_ref(), nodes));
            nodes[idx as usize] = WitValueNode::VariantVal(VariantRef {
                name: name.to_string(),
                payload_idx,
            });
            idx
        }
        WasmTypeKind::Option => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let inner_idx = value.unwrap_option().map(|v| build_node(v.as_ref(), nodes));
            nodes[idx as usize] = WitValueNode::OptionVal(inner_idx);
            idx
        }
        WasmTypeKind::Result => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let result_val = match value.unwrap_result() {
                Ok(ok_val) => {
                    let inner = ok_val.map(|v| build_node(v.as_ref(), nodes));
                    Ok(inner)
                }
                Err(err_val) => {
                    let inner = err_val.map(|v| build_node(v.as_ref(), nodes));
                    Err(inner)
                }
            };
            nodes[idx as usize] = WitValueNode::ResultVal(result_val);
            idx
        }
        WasmTypeKind::Flags => {
            let flags: Vec<_> = value.unwrap_flags().map(|s| s.to_string()).collect();
            nodes.push(WitValueNode::FlagsVal(flags));
            idx
        }
        _ => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::StringVal(
                format!("<unsupported type {:?}>", value.kind()),
            )));
            idx
        }
    }
}

/// Convert value-tree back to wasm_wave::Value.
fn value_tree_to_wave(tree: &ValueTree, wave_ty: &WaveType) -> WaveValue {
    if tree.nodes.is_empty() {
        return WaveValue::make_tuple(
            &WaveType::tuple(Vec::<WaveType>::new()).unwrap_or(WaveType::BOOL),
            std::iter::empty(),
        )
        .unwrap_or_else(|_| WaveValue::make_bool(false));
    }
    reconstruct_node(&tree.nodes, 0, wave_ty)
}

/// Recursively reconstruct a wave value from nodes.
fn reconstruct_node(nodes: &[WitValueNode], idx: u32, wave_ty: &WaveType) -> WaveValue {
    let node = &nodes[idx as usize];

    match node {
        WitValueNode::Primitive(prim) => match prim {
            PrimitiveValue::BoolVal(v) => WaveValue::make_bool(*v),
            PrimitiveValue::U8Val(v) => WaveValue::make_u8(*v),
            PrimitiveValue::U16Val(v) => WaveValue::make_u16(*v),
            PrimitiveValue::U32Val(v) => WaveValue::make_u32(*v),
            PrimitiveValue::U64Val(v) => WaveValue::make_u64(*v),
            PrimitiveValue::S8Val(v) => WaveValue::make_s8(*v),
            PrimitiveValue::S16Val(v) => WaveValue::make_s16(*v),
            PrimitiveValue::S32Val(v) => WaveValue::make_s32(*v),
            PrimitiveValue::S64Val(v) => WaveValue::make_s64(*v),
            PrimitiveValue::F32Val(v) => WaveValue::make_f32(*v),
            PrimitiveValue::F64Val(v) => WaveValue::make_f64(*v),
            PrimitiveValue::CharVal(v) => WaveValue::make_char(*v),
            PrimitiveValue::StringVal(v) => WaveValue::make_string(Cow::Owned(v.clone())),
        },

        WitValueNode::RecordVal(fields) => {
            let wave_fields: Vec<_> = wave_ty.record_fields().collect();
            let field_values: Vec<_> = fields
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let field_ty = wave_fields
                        .get(i)
                        .map(|(_, ty)| ty)
                        .cloned()
                        .unwrap_or(WaveType::U8);
                    let val = reconstruct_node(nodes, f.value_idx, &field_ty);
                    (f.name.as_str(), val)
                })
                .collect();
            WaveValue::make_record(wave_ty, field_values)
                .unwrap_or_else(|_| WaveValue::make_bool(false))
        }

        WitValueNode::TupleVal(indices) => {
            let wave_types: Vec<_> = wave_ty.tuple_element_types().collect();
            let elements: Vec<_> = indices
                .iter()
                .enumerate()
                .map(|(i, idx)| {
                    let elem_ty = wave_types.get(i).cloned().unwrap_or(WaveType::U8);
                    reconstruct_node(nodes, *idx, &elem_ty)
                })
                .collect();
            WaveValue::make_tuple(wave_ty, elements)
                .unwrap_or_else(|_| WaveValue::make_bool(false))
        }

        WitValueNode::ListVal(indices) => {
            let elem_ty = wave_ty.list_element_type().unwrap_or(WaveType::U8);
            let elements: Vec<_> = indices
                .iter()
                .map(|idx| reconstruct_node(nodes, *idx, &elem_ty))
                .collect();
            WaveValue::make_list(wave_ty, elements).unwrap_or_else(|_| WaveValue::make_bool(false))
        }

        WitValueNode::EnumVal(name) => {
            WaveValue::make_enum(wave_ty, name).unwrap_or_else(|_| WaveValue::make_bool(false))
        }

        WitValueNode::VariantVal(v) => {
            let payload = v.payload_idx.map(|idx| {
                let payload_ty = wave_ty
                    .variant_cases()
                    .find(|(case_name, _)| *case_name == v.name)
                    .and_then(|(_, ty)| ty)
                    .unwrap_or(WaveType::U8);
                reconstruct_node(nodes, idx, &payload_ty)
            });
            WaveValue::make_variant(wave_ty, &v.name, payload)
                .unwrap_or_else(|_| WaveValue::make_bool(false))
        }

        WitValueNode::OptionVal(opt_idx) => {
            let inner_ty = wave_ty.option_some_type().unwrap_or(WaveType::U8);
            let inner = opt_idx.map(|idx| reconstruct_node(nodes, idx, &inner_ty));
            WaveValue::make_option(wave_ty, inner).unwrap_or_else(|_| WaveValue::make_bool(false))
        }

        WitValueNode::ResultVal(res) => {
            let (ok_ty, err_ty) = wave_ty.result_types().unwrap_or((None, None));
            match res {
                Ok(ok_idx) => {
                    let inner = ok_idx.map(|idx| {
                        let ty = ok_ty.clone().unwrap_or(WaveType::U8);
                        reconstruct_node(nodes, idx, &ty)
                    });
                    WaveValue::make_result(wave_ty, Ok(inner))
                        .unwrap_or_else(|_| WaveValue::make_bool(false))
                }
                Err(err_idx) => {
                    let inner = err_idx.map(|idx| {
                        let ty = err_ty.clone().unwrap_or(WaveType::U8);
                        reconstruct_node(nodes, idx, &ty)
                    });
                    WaveValue::make_result(wave_ty, Err(inner))
                        .unwrap_or_else(|_| WaveValue::make_bool(false))
                }
            }
        }

        WitValueNode::FlagsVal(names) => {
            let name_strs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            WaveValue::make_flags(wave_ty, name_strs)
                .unwrap_or_else(|_| WaveValue::make_bool(false))
        }
    }
}

/// Test ValueTree roundtrip: WaveValue -> ValueTree -> WaveValue
fn value_tree_roundtrip(ty: &WaveType, value: &WaveValue) -> WaveValue {
    let tree = wave_to_value_tree(value);
    value_tree_to_wave(&tree, ty)
}

// ============================================================================
// Helper functions
// ============================================================================

/// Perform a roundtrip test: value -> string -> value
fn roundtrip(ty: &WaveType, value: &WaveValue) -> Result<WaveValue, String> {
    let s = wasm_wave::to_string(value).map_err(|e| format!("to_string failed: {e}"))?;
    let parsed: WaveValue =
        wasm_wave::from_str(ty, &s).map_err(|e| format!("from_str failed: {e}"))?;
    Ok(parsed)
}

/// Check if two wave values are equal (handling NaN specially for floats)
fn values_equal(a: &WaveValue, b: &WaveValue) -> bool {
    use wasm_wave::wasm::WasmTypeKind;

    match (a.kind(), b.kind()) {
        (WasmTypeKind::F32, WasmTypeKind::F32) => {
            let av = a.unwrap_f32();
            let bv = b.unwrap_f32();
            (av.is_nan() && bv.is_nan()) || av == bv
        }
        (WasmTypeKind::F64, WasmTypeKind::F64) => {
            let av = a.unwrap_f64();
            let bv = b.unwrap_f64();
            (av.is_nan() && bv.is_nan()) || av == bv
        }
        (WasmTypeKind::Record, WasmTypeKind::Record) => {
            let a_fields: Vec<_> = a.unwrap_record().collect();
            let b_fields: Vec<_> = b.unwrap_record().collect();
            if a_fields.len() != b_fields.len() {
                return false;
            }
            a_fields
                .iter()
                .zip(b_fields.iter())
                .all(|((an, av), (bn, bv))| an == bn && values_equal(av.as_ref(), bv.as_ref()))
        }
        (WasmTypeKind::Tuple, WasmTypeKind::Tuple) => {
            let a_elems: Vec<_> = a.unwrap_tuple().collect();
            let b_elems: Vec<_> = b.unwrap_tuple().collect();
            if a_elems.len() != b_elems.len() {
                return false;
            }
            a_elems
                .iter()
                .zip(b_elems.iter())
                .all(|(av, bv)| values_equal(av.as_ref(), bv.as_ref()))
        }
        (WasmTypeKind::List, WasmTypeKind::List)
        | (WasmTypeKind::FixedSizeList, WasmTypeKind::FixedSizeList)
        | (WasmTypeKind::List, WasmTypeKind::FixedSizeList)
        | (WasmTypeKind::FixedSizeList, WasmTypeKind::List) => {
            let a_elems: Vec<_> = a.unwrap_list().collect();
            let b_elems: Vec<_> = b.unwrap_list().collect();
            if a_elems.len() != b_elems.len() {
                return false;
            }
            a_elems
                .iter()
                .zip(b_elems.iter())
                .all(|(av, bv)| values_equal(av.as_ref(), bv.as_ref()))
        }
        (WasmTypeKind::Option, WasmTypeKind::Option) => {
            match (a.unwrap_option(), b.unwrap_option()) {
                (None, None) => true,
                (Some(av), Some(bv)) => values_equal(av.as_ref(), bv.as_ref()),
                _ => false,
            }
        }
        (WasmTypeKind::Result, WasmTypeKind::Result) => {
            match (a.unwrap_result(), b.unwrap_result()) {
                (Ok(None), Ok(None)) | (Err(None), Err(None)) => true,
                (Ok(Some(av)), Ok(Some(bv))) | (Err(Some(av)), Err(Some(bv))) => {
                    values_equal(av.as_ref(), bv.as_ref())
                }
                _ => false,
            }
        }
        (WasmTypeKind::Variant, WasmTypeKind::Variant) => {
            let (an, ap) = a.unwrap_variant();
            let (bn, bp) = b.unwrap_variant();
            if an != bn {
                return false;
            }
            match (ap, bp) {
                (None, None) => true,
                (Some(av), Some(bv)) => values_equal(av.as_ref(), bv.as_ref()),
                _ => false,
            }
        }
        _ => {
            // For other types, compare string representations
            let a_str = wasm_wave::to_string(a).unwrap_or_default();
            let b_str = wasm_wave::to_string(b).unwrap_or_default();
            a_str == b_str
        }
    }
}

// ============================================================================
// Unit tests for basic types
// ============================================================================

#[test]
fn test_bool_roundtrip() {
    let ty = WaveType::BOOL;
    for v in [true, false] {
        let val = WaveValue::make_bool(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "bool {v} failed roundtrip");
    }
}

#[test]
fn test_integer_roundtrip() {
    // u8
    let ty = WaveType::U8;
    for v in [0u8, 1, 127, 255] {
        let val = WaveValue::make_u8(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "u8 {v} failed roundtrip");
    }

    // s8
    let ty = WaveType::S8;
    for v in [-128i8, -1, 0, 1, 127] {
        let val = WaveValue::make_s8(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "s8 {v} failed roundtrip");
    }

    // u16
    let ty = WaveType::U16;
    for v in [0u16, 1, 32767, 65535] {
        let val = WaveValue::make_u16(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "u16 {v} failed roundtrip");
    }

    // s16
    let ty = WaveType::S16;
    for v in [-32768i16, -1, 0, 1, 32767] {
        let val = WaveValue::make_s16(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "s16 {v} failed roundtrip");
    }

    // u32
    let ty = WaveType::U32;
    for v in [0u32, 1, u32::MAX / 2, u32::MAX] {
        let val = WaveValue::make_u32(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "u32 {v} failed roundtrip");
    }

    // s32
    let ty = WaveType::S32;
    for v in [i32::MIN, -1, 0, 1, i32::MAX] {
        let val = WaveValue::make_s32(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "s32 {v} failed roundtrip");
    }

    // u64
    let ty = WaveType::U64;
    for v in [0u64, 1, u64::MAX / 2, u64::MAX] {
        let val = WaveValue::make_u64(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "u64 {v} failed roundtrip");
    }

    // s64
    let ty = WaveType::S64;
    for v in [i64::MIN, -1, 0, 1, i64::MAX] {
        let val = WaveValue::make_s64(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "s64 {v} failed roundtrip");
    }
}

#[test]
fn test_float_roundtrip() {
    // f32
    let ty = WaveType::F32;
    for v in [0.0f32, -0.0, 1.0, -1.0, f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        let val = WaveValue::make_f32(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "f32 {v} failed roundtrip");
    }

    // f64
    let ty = WaveType::F64;
    for v in [0.0f64, -0.0, 1.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let val = WaveValue::make_f64(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "f64 {v} failed roundtrip");
    }
}

#[test]
fn test_char_roundtrip() {
    let ty = WaveType::CHAR;
    for c in ['a', 'Z', '0', ' ', '\n', '\t', '\'', '"', '\\', '\u{0}', '\u{ffff}', '🎉'] {
        let val = WaveValue::make_char(c);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "char {c:?} failed roundtrip");
    }
}

#[test]
fn test_string_roundtrip() {
    let ty = WaveType::STRING;
    for s in [
        "",
        "hello",
        "hello world",
        "line1\nline2",
        "tab\there",
        "quote\"here",
        "backslash\\here",
        "unicode: 你好世界",
        "emoji: 🎉🚀",
    ] {
        let val = WaveValue::make_string(Cow::Borrowed(s));
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "string {s:?} failed roundtrip");
    }
}

#[test]
fn test_option_roundtrip() {
    let ty = WaveType::option(WaveType::U32);

    // None
    let val = WaveValue::make_option(&ty, None).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "option none failed roundtrip");

    // Some
    let val = WaveValue::make_option(&ty, Some(WaveValue::make_u32(42))).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "option some(42) failed roundtrip"
    );
}

#[test]
fn test_result_roundtrip() {
    let ty = WaveType::result(Some(WaveType::U32), Some(WaveType::STRING));

    // Ok with value
    let val = WaveValue::make_result(&ty, Ok(Some(WaveValue::make_u32(42)))).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "result ok(42) failed roundtrip");

    // Err with value
    let val = WaveValue::make_result(
        &ty,
        Err(Some(WaveValue::make_string(Cow::Borrowed("error")))),
    )
    .unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "result err(\"error\") failed roundtrip"
    );

    // Result with no payloads
    let ty_empty = WaveType::result(None, None);
    let val = WaveValue::make_result(&ty_empty, Ok(None)).unwrap();
    let result = roundtrip(&ty_empty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "result ok failed roundtrip");

    let val = WaveValue::make_result(&ty_empty, Err(None)).unwrap();
    let result = roundtrip(&ty_empty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "result err failed roundtrip");
}

#[test]
fn test_list_roundtrip() {
    let ty = WaveType::list(WaveType::U32);

    // Empty list
    let val = WaveValue::make_list(&ty, vec![]).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "empty list failed roundtrip");

    // Non-empty list
    let val = WaveValue::make_list(
        &ty,
        vec![
            WaveValue::make_u32(1),
            WaveValue::make_u32(2),
            WaveValue::make_u32(3),
        ],
    )
    .unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "list [1,2,3] failed roundtrip");
}

#[test]
fn test_tuple_roundtrip() {
    let ty = WaveType::tuple(vec![WaveType::U32, WaveType::STRING, WaveType::BOOL]).unwrap();

    let val = WaveValue::make_tuple(
        &ty,
        vec![
            WaveValue::make_u32(42),
            WaveValue::make_string(Cow::Borrowed("hello")),
            WaveValue::make_bool(true),
        ],
    )
    .unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "tuple (42, \"hello\", true) failed roundtrip"
    );
}

#[test]
fn test_record_roundtrip() {
    let ty = WaveType::record(vec![
        ("name".to_string(), WaveType::STRING),
        ("age".to_string(), WaveType::U32),
        ("active".to_string(), WaveType::BOOL),
    ])
    .unwrap();

    let val = WaveValue::make_record(
        &ty,
        vec![
            ("name", WaveValue::make_string(Cow::Borrowed("Alice"))),
            ("age", WaveValue::make_u32(30)),
            ("active", WaveValue::make_bool(true)),
        ],
    )
    .unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "record failed roundtrip");
}

#[test]
fn test_enum_roundtrip() {
    let ty = WaveType::enum_ty(vec!["red".to_string(), "green".to_string(), "blue".to_string()])
        .unwrap();

    for case in ["red", "green", "blue"] {
        let val = WaveValue::make_enum(&ty, case).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        assert!(values_equal(&val, &result), "enum {case} failed roundtrip");
    }
}

#[test]
fn test_variant_roundtrip() {
    let ty = WaveType::variant(vec![
        ("none".to_string(), None),
        ("some-int".to_string(), Some(WaveType::U32)),
        ("some-string".to_string(), Some(WaveType::STRING)),
    ])
    .unwrap();

    // Variant without payload
    let val = WaveValue::make_variant(&ty, "none", None).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "variant none failed roundtrip");

    // Variant with int payload
    let val = WaveValue::make_variant(&ty, "some-int", Some(WaveValue::make_u32(42))).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "variant some-int(42) failed roundtrip"
    );

    // Variant with string payload
    let val = WaveValue::make_variant(
        &ty,
        "some-string",
        Some(WaveValue::make_string(Cow::Borrowed("hello"))),
    )
    .unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "variant some-string(\"hello\") failed roundtrip"
    );
}

#[test]
fn test_flags_roundtrip() {
    let ty = WaveType::flags(vec![
        "read".to_string(),
        "write".to_string(),
        "execute".to_string(),
    ])
    .unwrap();

    // Empty flags
    let val = WaveValue::make_flags(&ty, vec![]).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "empty flags failed roundtrip");

    // Single flag
    let val = WaveValue::make_flags(&ty, vec!["read"]).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "flags {{read}} failed roundtrip"
    );

    // Multiple flags
    let val = WaveValue::make_flags(&ty, vec!["read", "write"]).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "flags {{read, write}} failed roundtrip"
    );

    // All flags
    let val = WaveValue::make_flags(&ty, vec!["read", "write", "execute"]).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "all flags failed roundtrip");
}

// ============================================================================
// Nested/deep structure tests
// ============================================================================

#[test]
fn test_nested_option_roundtrip() {
    // option<option<u32>>
    let inner_ty = WaveType::option(WaveType::U32);
    let ty = WaveType::option(inner_ty.clone());

    // none
    let val = WaveValue::make_option(&ty, None).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "nested option none failed roundtrip"
    );

    // some(none)
    let inner_none = WaveValue::make_option(&inner_ty, None).unwrap();
    let val = WaveValue::make_option(&ty, Some(inner_none)).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "nested option some(none) failed roundtrip"
    );

    // some(some(42))
    let inner_some = WaveValue::make_option(&inner_ty, Some(WaveValue::make_u32(42))).unwrap();
    let val = WaveValue::make_option(&ty, Some(inner_some)).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "nested option some(some(42)) failed roundtrip"
    );
}

#[test]
fn test_nested_list_roundtrip() {
    // list<list<u32>>
    let inner_ty = WaveType::list(WaveType::U32);
    let ty = WaveType::list(inner_ty.clone());

    // [[1, 2], [3, 4, 5], []]
    let val = WaveValue::make_list(
        &ty,
        vec![
            WaveValue::make_list(
                &inner_ty,
                vec![WaveValue::make_u32(1), WaveValue::make_u32(2)],
            )
            .unwrap(),
            WaveValue::make_list(
                &inner_ty,
                vec![
                    WaveValue::make_u32(3),
                    WaveValue::make_u32(4),
                    WaveValue::make_u32(5),
                ],
            )
            .unwrap(),
            WaveValue::make_list(&inner_ty, vec![]).unwrap(),
        ],
    )
    .unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(values_equal(&val, &result), "nested list failed roundtrip");
}

#[test]
fn test_deeply_nested_record_roundtrip() {
    // record { inner: record { value: u32 } }
    let inner_ty = WaveType::record(vec![("value".to_string(), WaveType::U32)]).unwrap();
    let ty = WaveType::record(vec![("inner".to_string(), inner_ty.clone())]).unwrap();

    let inner_val = WaveValue::make_record(&inner_ty, vec![("value", WaveValue::make_u32(42))]).unwrap();
    let val = WaveValue::make_record(&ty, vec![("inner", inner_val)]).unwrap();
    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "deeply nested record failed roundtrip"
    );
}

#[test]
fn test_complex_nested_structure() {
    // record {
    //   items: list<record { name: string, value: option<u32> }>,
    //   status: result<u32, string>
    // }
    let item_ty = WaveType::record(vec![
        ("name".to_string(), WaveType::STRING),
        ("value".to_string(), WaveType::option(WaveType::U32)),
    ])
    .unwrap();
    let items_ty = WaveType::list(item_ty.clone());
    let status_ty = WaveType::result(Some(WaveType::U32), Some(WaveType::STRING));
    let ty = WaveType::record(vec![
        ("items".to_string(), items_ty.clone()),
        ("status".to_string(), status_ty.clone()),
    ])
    .unwrap();

    let item1 = WaveValue::make_record(
        &item_ty,
        vec![
            ("name", WaveValue::make_string(Cow::Borrowed("first"))),
            (
                "value",
                WaveValue::make_option(&WaveType::option(WaveType::U32), Some(WaveValue::make_u32(1)))
                    .unwrap(),
            ),
        ],
    )
    .unwrap();
    let item2 = WaveValue::make_record(
        &item_ty,
        vec![
            ("name", WaveValue::make_string(Cow::Borrowed("second"))),
            (
                "value",
                WaveValue::make_option(&WaveType::option(WaveType::U32), None).unwrap(),
            ),
        ],
    )
    .unwrap();

    let val = WaveValue::make_record(
        &ty,
        vec![
            (
                "items",
                WaveValue::make_list(&items_ty, vec![item1, item2]).unwrap(),
            ),
            (
                "status",
                WaveValue::make_result(&status_ty, Ok(Some(WaveValue::make_u32(200)))).unwrap(),
            ),
        ],
    )
    .unwrap();

    let result = roundtrip(&ty, &val).expect("roundtrip failed");
    assert!(
        values_equal(&val, &result),
        "complex nested structure failed roundtrip"
    );
}

// ============================================================================
// Property-based tests
// ============================================================================

/// Strategy for generating printable strings (avoiding problematic characters)
fn safe_string_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::char::range('\u{20}', '\u{7e}'), 0..50)
        .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_bool_roundtrip(v in any::<bool>()) {
        let ty = WaveType::BOOL;
        let val = WaveValue::make_bool(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_u8_roundtrip(v in any::<u8>()) {
        let ty = WaveType::U8;
        let val = WaveValue::make_u8(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_u16_roundtrip(v in any::<u16>()) {
        let ty = WaveType::U16;
        let val = WaveValue::make_u16(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_u32_roundtrip(v in any::<u32>()) {
        let ty = WaveType::U32;
        let val = WaveValue::make_u32(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_u64_roundtrip(v in any::<u64>()) {
        let ty = WaveType::U64;
        let val = WaveValue::make_u64(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_s8_roundtrip(v in any::<i8>()) {
        let ty = WaveType::S8;
        let val = WaveValue::make_s8(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_s16_roundtrip(v in any::<i16>()) {
        let ty = WaveType::S16;
        let val = WaveValue::make_s16(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_s32_roundtrip(v in any::<i32>()) {
        let ty = WaveType::S32;
        let val = WaveValue::make_s32(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_s64_roundtrip(v in any::<i64>()) {
        let ty = WaveType::S64;
        let val = WaveValue::make_s64(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_f32_roundtrip(v in any::<f32>()) {
        let ty = WaveType::F32;
        let val = WaveValue::make_f32(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_f64_roundtrip(v in any::<f64>()) {
        let ty = WaveType::F64;
        let val = WaveValue::make_f64(v);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_char_roundtrip(c in prop::char::any()) {
        let ty = WaveType::CHAR;
        let val = WaveValue::make_char(c);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_string_roundtrip(s in safe_string_strategy()) {
        let ty = WaveType::STRING;
        let val = WaveValue::make_string(Cow::Owned(s));
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_option_u32_roundtrip(v in prop::option::of(any::<u32>())) {
        let ty = WaveType::option(WaveType::U32);
        let val = WaveValue::make_option(&ty, v.map(WaveValue::make_u32)).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_list_u32_roundtrip(items in prop::collection::vec(any::<u32>(), 0..20)) {
        let ty = WaveType::list(WaveType::U32);
        let val = WaveValue::make_list(&ty, items.into_iter().map(WaveValue::make_u32)).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_tuple_roundtrip(a in any::<u32>(), b in safe_string_strategy(), c in any::<bool>()) {
        let ty = WaveType::tuple(vec![WaveType::U32, WaveType::STRING, WaveType::BOOL]).unwrap();
        let val = WaveValue::make_tuple(&ty, vec![
            WaveValue::make_u32(a),
            WaveValue::make_string(Cow::Owned(b)),
            WaveValue::make_bool(c),
        ]).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_result_roundtrip(is_ok in any::<bool>(), ok_val in any::<u32>(), err_val in safe_string_strategy()) {
        let ty = WaveType::result(Some(WaveType::U32), Some(WaveType::STRING));
        let val = if is_ok {
            WaveValue::make_result(&ty, Ok(Some(WaveValue::make_u32(ok_val)))).unwrap()
        } else {
            WaveValue::make_result(&ty, Err(Some(WaveValue::make_string(Cow::Owned(err_val))))).unwrap()
        };
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_nested_list_roundtrip(
        items in prop::collection::vec(
            prop::collection::vec(any::<u32>(), 0..5),
            0..5
        )
    ) {
        let inner_ty = WaveType::list(WaveType::U32);
        let ty = WaveType::list(inner_ty.clone());
        let val = WaveValue::make_list(&ty, items.into_iter().map(|inner| {
            WaveValue::make_list(&inner_ty, inner.into_iter().map(WaveValue::make_u32)).unwrap()
        })).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_nested_option_roundtrip(v in prop::option::of(prop::option::of(any::<u32>()))) {
        let inner_ty = WaveType::option(WaveType::U32);
        let ty = WaveType::option(inner_ty.clone());
        let val = WaveValue::make_option(&ty, v.map(|inner| {
            WaveValue::make_option(&inner_ty, inner.map(WaveValue::make_u32)).unwrap()
        })).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_record_roundtrip(name in safe_string_strategy(), age in any::<u32>(), active in any::<bool>()) {
        let ty = WaveType::record(vec![
            ("name".to_string(), WaveType::STRING),
            ("age".to_string(), WaveType::U32),
            ("active".to_string(), WaveType::BOOL),
        ]).unwrap();
        let val = WaveValue::make_record(&ty, vec![
            ("name", WaveValue::make_string(Cow::Owned(name))),
            ("age", WaveValue::make_u32(age)),
            ("active", WaveValue::make_bool(active)),
        ]).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_enum_roundtrip(case_idx in 0usize..3) {
        let cases = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        let ty = WaveType::enum_ty(cases.clone()).unwrap();
        let val = WaveValue::make_enum(&ty, &cases[case_idx]).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_flags_roundtrip(flags in prop::collection::vec(0usize..3, 0..4)) {
        let all_flags = vec!["read".to_string(), "write".to_string(), "exec".to_string()];
        let ty = WaveType::flags(all_flags.clone()).unwrap();
        let selected: Vec<&str> = flags.into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(|i| all_flags[i].as_str())
            .collect();
        let val = WaveValue::make_flags(&ty, selected).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_variant_roundtrip(case_idx in 0usize..3, payload in any::<u32>()) {
        let ty = WaveType::variant(vec![
            ("empty".to_string(), None),
            ("with-int".to_string(), Some(WaveType::U32)),
            ("with-bool".to_string(), Some(WaveType::BOOL)),
        ]).unwrap();

        let val = match case_idx {
            0 => WaveValue::make_variant(&ty, "empty", None).unwrap(),
            1 => WaveValue::make_variant(&ty, "with-int", Some(WaveValue::make_u32(payload))).unwrap(),
            _ => WaveValue::make_variant(&ty, "with-bool", Some(WaveValue::make_bool(payload % 2 == 0))).unwrap(),
        };
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }
}

// ============================================================================
// Deep nesting property tests
// ============================================================================

/// Generate a type with a given depth of nesting
fn make_nested_option_type(depth: usize) -> WaveType {
    let mut ty = WaveType::U32;
    for _ in 0..depth {
        ty = WaveType::option(ty);
    }
    ty
}

/// Generate a nested option value
fn make_nested_option_value(ty: &WaveType, depth: usize, value: Option<u32>) -> WaveValue {
    if depth == 0 {
        return value.map(WaveValue::make_u32).unwrap_or_else(|| WaveValue::make_u32(0));
    }

    let inner_ty = ty.option_some_type().unwrap();
    match value {
        None => WaveValue::make_option(ty, None).unwrap(),
        Some(v) => {
            let inner = make_nested_option_value(&inner_ty, depth - 1, Some(v));
            WaveValue::make_option(ty, Some(inner)).unwrap()
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_deeply_nested_option(depth in 1usize..6, value in prop::option::of(any::<u32>())) {
        let ty = make_nested_option_type(depth);
        let val = match value {
            None => WaveValue::make_option(&ty, None).unwrap(),
            Some(v) => make_nested_option_value(&ty, depth, Some(v)),
        };
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_deeply_nested_list(depth in 1usize..4, leaf_count in 0usize..5) {
        fn make_nested_list_type(depth: usize) -> WaveType {
            let mut ty = WaveType::U32;
            for _ in 0..depth {
                ty = WaveType::list(ty);
            }
            ty
        }

        fn make_nested_list_value(ty: &WaveType, depth: usize, leaf_count: usize) -> WaveValue {
            if depth == 0 {
                return WaveValue::make_u32(42);
            }

            let inner_ty = ty.list_element_type().unwrap();
            let items: Vec<WaveValue> = (0..leaf_count)
                .map(|_| make_nested_list_value(&inner_ty, depth - 1, leaf_count))
                .collect();
            WaveValue::make_list(ty, items).unwrap()
        }

        let ty = make_nested_list_type(depth);
        let val = make_nested_list_value(&ty, depth, leaf_count);
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_complex_record_with_depth(
        depth in 1usize..3,
        list_len in 0usize..3,
        has_option in any::<bool>(),
        has_result in any::<bool>()
    ) {
        // Build a record type with varying complexity
        let mut fields = vec![("id".to_string(), WaveType::U32)];

        if has_option {
            let opt_ty = make_nested_option_type(depth);
            fields.push(("opt".to_string(), opt_ty));
        }

        if has_result {
            let result_ty = WaveType::result(Some(WaveType::U32), Some(WaveType::STRING));
            fields.push(("res".to_string(), result_ty));
        }

        let list_ty = WaveType::list(WaveType::STRING);
        fields.push(("items".to_string(), list_ty.clone()));

        let ty = WaveType::record(fields.clone()).unwrap();

        // Build the value
        let mut field_values: Vec<(&str, WaveValue)> = vec![
            ("id", WaveValue::make_u32(123))
        ];

        if has_option {
            let opt_ty = make_nested_option_type(depth);
            let opt_val = make_nested_option_value(&opt_ty, depth, Some(42));
            field_values.push(("opt", opt_val));
        }

        if has_result {
            let result_ty = WaveType::result(Some(WaveType::U32), Some(WaveType::STRING));
            let result_val = WaveValue::make_result(&result_ty, Ok(Some(WaveValue::make_u32(200)))).unwrap();
            field_values.push(("res", result_val));
        }

        let items: Vec<WaveValue> = (0..list_len)
            .map(|i| WaveValue::make_string(Cow::Owned(format!("item{}", i))))
            .collect();
        field_values.push(("items", WaveValue::make_list(&list_ty, items).unwrap()));

        let val = WaveValue::make_record(&ty, field_values).unwrap();
        let result = roundtrip(&ty, &val).expect("roundtrip failed");
        prop_assert!(values_equal(&val, &result));
    }
}

// ============================================================================
// ValueTree conversion unit tests
// ============================================================================

#[test]
fn test_value_tree_bool_roundtrip() {
    let ty = WaveType::BOOL;
    for v in [true, false] {
        let val = WaveValue::make_bool(v);
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "bool {v} failed value_tree roundtrip");
    }
}

#[test]
fn test_value_tree_integer_roundtrip() {
    // u8
    let ty = WaveType::U8;
    for v in [0u8, 1, 127, 255] {
        let val = WaveValue::make_u8(v);
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "u8 {v} failed value_tree roundtrip");
    }

    // s32
    let ty = WaveType::S32;
    for v in [i32::MIN, -1, 0, 1, i32::MAX] {
        let val = WaveValue::make_s32(v);
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "s32 {v} failed value_tree roundtrip");
    }

    // u64
    let ty = WaveType::U64;
    for v in [0u64, 1, u64::MAX / 2, u64::MAX] {
        let val = WaveValue::make_u64(v);
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "u64 {v} failed value_tree roundtrip");
    }
}

#[test]
fn test_value_tree_float_roundtrip() {
    let ty = WaveType::F32;
    for v in [0.0f32, -0.0, 1.5, -1.5, f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        let val = WaveValue::make_f32(v);
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "f32 {v} failed value_tree roundtrip");
    }

    let ty = WaveType::F64;
    for v in [0.0f64, 1.5e100, f64::NAN] {
        let val = WaveValue::make_f64(v);
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "f64 {v} failed value_tree roundtrip");
    }
}

#[test]
fn test_value_tree_char_roundtrip() {
    let ty = WaveType::CHAR;
    for c in ['a', '\n', '\u{0}', '🎉'] {
        let val = WaveValue::make_char(c);
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "char {c:?} failed value_tree roundtrip");
    }
}

#[test]
fn test_value_tree_string_roundtrip() {
    let ty = WaveType::STRING;
    for s in ["", "hello", "unicode: 你好", "emoji: 🚀"] {
        let val = WaveValue::make_string(Cow::Borrowed(s));
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "string {s:?} failed value_tree roundtrip");
    }
}

#[test]
fn test_value_tree_option_roundtrip() {
    let ty = WaveType::option(WaveType::U32);

    // None
    let val = WaveValue::make_option(&ty, None).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "option none failed value_tree roundtrip");

    // Some
    let val = WaveValue::make_option(&ty, Some(WaveValue::make_u32(42))).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "option some(42) failed value_tree roundtrip");
}

#[test]
fn test_value_tree_result_roundtrip() {
    let ty = WaveType::result(Some(WaveType::U32), Some(WaveType::STRING));

    // Ok with value
    let val = WaveValue::make_result(&ty, Ok(Some(WaveValue::make_u32(42)))).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "result ok(42) failed value_tree roundtrip");

    // Err with value
    let val = WaveValue::make_result(&ty, Err(Some(WaveValue::make_string(Cow::Borrowed("error"))))).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "result err failed value_tree roundtrip");

    // Result with no payloads
    let ty_empty = WaveType::result(None, None);
    let val = WaveValue::make_result(&ty_empty, Ok(None)).unwrap();
    let result = value_tree_roundtrip(&ty_empty, &val);
    assert!(values_equal(&val, &result), "result ok (empty) failed value_tree roundtrip");
}

#[test]
fn test_value_tree_list_roundtrip() {
    let ty = WaveType::list(WaveType::U32);

    // Empty list
    let val = WaveValue::make_list(&ty, vec![]).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "empty list failed value_tree roundtrip");

    // Non-empty list
    let val = WaveValue::make_list(&ty, vec![
        WaveValue::make_u32(1),
        WaveValue::make_u32(2),
        WaveValue::make_u32(3),
    ]).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "list [1,2,3] failed value_tree roundtrip");
}

#[test]
fn test_value_tree_tuple_roundtrip() {
    let ty = WaveType::tuple(vec![WaveType::U32, WaveType::STRING, WaveType::BOOL]).unwrap();

    let val = WaveValue::make_tuple(&ty, vec![
        WaveValue::make_u32(42),
        WaveValue::make_string(Cow::Borrowed("hello")),
        WaveValue::make_bool(true),
    ]).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "tuple failed value_tree roundtrip");
}

#[test]
fn test_value_tree_record_roundtrip() {
    let ty = WaveType::record(vec![
        ("name".to_string(), WaveType::STRING),
        ("age".to_string(), WaveType::U32),
        ("active".to_string(), WaveType::BOOL),
    ]).unwrap();

    let val = WaveValue::make_record(&ty, vec![
        ("name", WaveValue::make_string(Cow::Borrowed("Alice"))),
        ("age", WaveValue::make_u32(30)),
        ("active", WaveValue::make_bool(true)),
    ]).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "record failed value_tree roundtrip");
}

#[test]
fn test_value_tree_enum_roundtrip() {
    let ty = WaveType::enum_ty(vec!["red".to_string(), "green".to_string(), "blue".to_string()]).unwrap();

    for case in ["red", "green", "blue"] {
        let val = WaveValue::make_enum(&ty, case).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        assert!(values_equal(&val, &result), "enum {case} failed value_tree roundtrip");
    }
}

#[test]
fn test_value_tree_variant_roundtrip() {
    let ty = WaveType::variant(vec![
        ("none".to_string(), None),
        ("some-int".to_string(), Some(WaveType::U32)),
        ("some-string".to_string(), Some(WaveType::STRING)),
    ]).unwrap();

    // Variant without payload
    let val = WaveValue::make_variant(&ty, "none", None).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "variant none failed value_tree roundtrip");

    // Variant with int payload
    let val = WaveValue::make_variant(&ty, "some-int", Some(WaveValue::make_u32(42))).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "variant some-int(42) failed value_tree roundtrip");

    // Variant with string payload
    let val = WaveValue::make_variant(&ty, "some-string", Some(WaveValue::make_string(Cow::Borrowed("hello")))).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "variant some-string failed value_tree roundtrip");
}

#[test]
fn test_value_tree_flags_roundtrip() {
    let ty = WaveType::flags(vec!["read".to_string(), "write".to_string(), "execute".to_string()]).unwrap();

    // Empty flags
    let val = WaveValue::make_flags(&ty, vec![]).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "empty flags failed value_tree roundtrip");

    // Multiple flags
    let val = WaveValue::make_flags(&ty, vec!["read", "write"]).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "flags failed value_tree roundtrip");
}

#[test]
fn test_value_tree_nested_structures() {
    // option<option<u32>>
    let inner_ty = WaveType::option(WaveType::U32);
    let ty = WaveType::option(inner_ty.clone());

    let inner_some = WaveValue::make_option(&inner_ty, Some(WaveValue::make_u32(42))).unwrap();
    let val = WaveValue::make_option(&ty, Some(inner_some)).unwrap();
    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "nested option failed value_tree roundtrip");

    // list<list<u32>>
    let inner_list_ty = WaveType::list(WaveType::U32);
    let list_ty = WaveType::list(inner_list_ty.clone());

    let val = WaveValue::make_list(&list_ty, vec![
        WaveValue::make_list(&inner_list_ty, vec![WaveValue::make_u32(1), WaveValue::make_u32(2)]).unwrap(),
        WaveValue::make_list(&inner_list_ty, vec![WaveValue::make_u32(3)]).unwrap(),
    ]).unwrap();
    let result = value_tree_roundtrip(&list_ty, &val);
    assert!(values_equal(&val, &result), "nested list failed value_tree roundtrip");
}

#[test]
fn test_value_tree_complex_structure() {
    // record { items: list<option<u32>>, status: result<string, u32> }
    let option_ty = WaveType::option(WaveType::U32);
    let items_ty = WaveType::list(option_ty.clone());
    let status_ty = WaveType::result(Some(WaveType::STRING), Some(WaveType::U32));
    let ty = WaveType::record(vec![
        ("items".to_string(), items_ty.clone()),
        ("status".to_string(), status_ty.clone()),
    ]).unwrap();

    let val = WaveValue::make_record(&ty, vec![
        ("items", WaveValue::make_list(&items_ty, vec![
            WaveValue::make_option(&option_ty, Some(WaveValue::make_u32(1))).unwrap(),
            WaveValue::make_option(&option_ty, None).unwrap(),
            WaveValue::make_option(&option_ty, Some(WaveValue::make_u32(3))).unwrap(),
        ]).unwrap()),
        ("status", WaveValue::make_result(&status_ty, Ok(Some(WaveValue::make_string(Cow::Borrowed("success"))))).unwrap()),
    ]).unwrap();

    let result = value_tree_roundtrip(&ty, &val);
    assert!(values_equal(&val, &result), "complex structure failed value_tree roundtrip");
}

#[test]
fn test_value_tree_preserves_structure() {
    // Test that the tree structure is correct
    let ty = WaveType::record(vec![
        ("a".to_string(), WaveType::U32),
        ("b".to_string(), WaveType::STRING),
    ]).unwrap();

    let val = WaveValue::make_record(&ty, vec![
        ("a", WaveValue::make_u32(42)),
        ("b", WaveValue::make_string(Cow::Borrowed("hello"))),
    ]).unwrap();

    let tree = wave_to_value_tree(&val);

    // Check tree structure
    assert_eq!(tree.nodes.len(), 3, "expected 3 nodes (record + 2 fields)");

    // Root should be a record with 2 fields
    match &tree.nodes[0] {
        WitValueNode::RecordVal(fields) => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "a");
            assert_eq!(fields[1].name, "b");
        }
        _ => panic!("expected record node at root"),
    }

    // Field values should be at correct indices
    match &tree.nodes[tree.nodes.len() - 2] {
        WitValueNode::Primitive(PrimitiveValue::U32Val(42)) => {}
        other => panic!("expected u32 primitive, got {:?}", other),
    }
    match &tree.nodes[tree.nodes.len() - 1] {
        WitValueNode::Primitive(PrimitiveValue::StringVal(s)) if s == "hello" => {}
        other => panic!("expected string primitive, got {:?}", other),
    }
}

// ============================================================================
// ValueTree property-based tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_value_tree_bool(v in any::<bool>()) {
        let ty = WaveType::BOOL;
        let val = WaveValue::make_bool(v);
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_u32(v in any::<u32>()) {
        let ty = WaveType::U32;
        let val = WaveValue::make_u32(v);
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_s64(v in any::<i64>()) {
        let ty = WaveType::S64;
        let val = WaveValue::make_s64(v);
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_f32(v in any::<f32>()) {
        let ty = WaveType::F32;
        let val = WaveValue::make_f32(v);
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_f64(v in any::<f64>()) {
        let ty = WaveType::F64;
        let val = WaveValue::make_f64(v);
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_char(c in prop::char::any()) {
        let ty = WaveType::CHAR;
        let val = WaveValue::make_char(c);
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_string(s in ".*") {
        let ty = WaveType::STRING;
        let val = WaveValue::make_string(Cow::Owned(s));
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_option(v in prop::option::of(any::<u32>())) {
        let ty = WaveType::option(WaveType::U32);
        let val = WaveValue::make_option(&ty, v.map(WaveValue::make_u32)).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_list(items in prop::collection::vec(any::<u32>(), 0..20)) {
        let ty = WaveType::list(WaveType::U32);
        let val = WaveValue::make_list(&ty, items.into_iter().map(WaveValue::make_u32)).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_tuple(a in any::<u32>(), b in any::<bool>(), c in any::<i64>()) {
        let ty = WaveType::tuple(vec![WaveType::U32, WaveType::BOOL, WaveType::S64]).unwrap();
        let val = WaveValue::make_tuple(&ty, vec![
            WaveValue::make_u32(a),
            WaveValue::make_bool(b),
            WaveValue::make_s64(c),
        ]).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_record(id in any::<u32>(), active in any::<bool>()) {
        let ty = WaveType::record(vec![
            ("id".to_string(), WaveType::U32),
            ("active".to_string(), WaveType::BOOL),
        ]).unwrap();
        let val = WaveValue::make_record(&ty, vec![
            ("id", WaveValue::make_u32(id)),
            ("active", WaveValue::make_bool(active)),
        ]).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_result(is_ok in any::<bool>(), ok_val in any::<u32>(), err_val in any::<i32>()) {
        let ty = WaveType::result(Some(WaveType::U32), Some(WaveType::S32));
        let val = if is_ok {
            WaveValue::make_result(&ty, Ok(Some(WaveValue::make_u32(ok_val)))).unwrap()
        } else {
            WaveValue::make_result(&ty, Err(Some(WaveValue::make_s32(err_val)))).unwrap()
        };
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_enum(case_idx in 0usize..4) {
        let cases = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()];
        let ty = WaveType::enum_ty(cases.clone()).unwrap();
        let val = WaveValue::make_enum(&ty, &cases[case_idx]).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_variant(case_idx in 0usize..2, payload in any::<u32>()) {
        let ty = WaveType::variant(vec![
            ("empty".to_string(), None),
            ("with-val".to_string(), Some(WaveType::U32)),
        ]).unwrap();
        let val = match case_idx {
            0 => WaveValue::make_variant(&ty, "empty", None).unwrap(),
            _ => WaveValue::make_variant(&ty, "with-val", Some(WaveValue::make_u32(payload))).unwrap(),
        };
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_flags(indices in prop::collection::vec(0usize..3, 0..4)) {
        let all_flags = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let ty = WaveType::flags(all_flags.clone()).unwrap();
        let selected: Vec<&str> = indices.into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(|i| all_flags[i].as_str())
            .collect();
        let val = WaveValue::make_flags(&ty, selected).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_nested_option(depth in 1usize..5, inner_val in prop::option::of(any::<u32>())) {
        let ty = make_nested_option_type(depth);
        let val = match inner_val {
            None => WaveValue::make_option(&ty, None).unwrap(),
            Some(v) => make_nested_option_value(&ty, depth, Some(v)),
        };
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }

    #[test]
    fn prop_value_tree_nested_list(
        items in prop::collection::vec(
            prop::collection::vec(any::<u32>(), 0..5),
            0..5
        )
    ) {
        let inner_ty = WaveType::list(WaveType::U32);
        let ty = WaveType::list(inner_ty.clone());
        let val = WaveValue::make_list(&ty, items.into_iter().map(|inner| {
            WaveValue::make_list(&inner_ty, inner.into_iter().map(WaveValue::make_u32)).unwrap()
        })).unwrap();
        let result = value_tree_roundtrip(&ty, &val);
        prop_assert!(values_equal(&val, &result));
    }
}
