//! Conversion between wasm_wave::Value and our WIT-defined value-tree types.
//!
//! This module provides bidirectional conversion:
//! - `wave_to_value_tree`: wasm_wave::Value -> value-tree (for use after lifting)
//! - `value_tree_to_wave`: value-tree -> wasm_wave::Value (for WAVE formatting)

use std::borrow::Cow;

use wasm_wave::value::{Type as WaveType, Value as WaveValue};
use wasm_wave::wasm::{WasmType, WasmTypeKind, WasmValue};

use crate::exports::wit_kv::wit_ast::types::{
    FieldRef, PrimitiveValue, ValueTree, VariantRef, WitValueNode,
};

/// Convert a wasm_wave::Value to our value-tree representation.
/// This builds a flat array of nodes with the root at index 0.
pub fn wave_to_value_tree(value: &WaveValue) -> ValueTree {
    let mut nodes = Vec::new();
    build_node(value, &mut nodes);
    ValueTree { nodes }
}

/// Recursively build nodes from a wave value.
/// Returns the index of the created node.
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
            // Reserve space for this node first
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
            if let Some(node) = nodes.get_mut(idx as usize) {
                *node = WitValueNode::RecordVal(field_refs);
            }
            idx
        }
        WasmTypeKind::Tuple => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let indices: Vec<_> = value
                .unwrap_tuple()
                .map(|v| build_node(v.as_ref(), nodes))
                .collect();
            if let Some(node) = nodes.get_mut(idx as usize) {
                *node = WitValueNode::TupleVal(indices);
            }
            idx
        }
        WasmTypeKind::List | WasmTypeKind::FixedSizeList => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let indices: Vec<_> = value
                .unwrap_list()
                .map(|v| build_node(v.as_ref(), nodes))
                .collect();
            if let Some(node) = nodes.get_mut(idx as usize) {
                *node = WitValueNode::ListVal(indices);
            }
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
            if let Some(node) = nodes.get_mut(idx as usize) {
                *node = WitValueNode::VariantVal(VariantRef {
                    name: name.to_string(),
                    payload_idx,
                });
            }
            idx
        }
        WasmTypeKind::Option => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::BoolVal(false))); // placeholder
            let inner_idx = value.unwrap_option().map(|v| build_node(v.as_ref(), nodes));
            if let Some(node) = nodes.get_mut(idx as usize) {
                *node = WitValueNode::OptionVal(inner_idx);
            }
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
            if let Some(node) = nodes.get_mut(idx as usize) {
                *node = WitValueNode::ResultVal(result_val);
            }
            idx
        }
        WasmTypeKind::Flags => {
            let flags: Vec<_> = value.unwrap_flags().map(|s| s.to_string()).collect();
            nodes.push(WitValueNode::FlagsVal(flags));
            idx
        }
        // Unsupported types
        _ => {
            nodes.push(WitValueNode::Primitive(PrimitiveValue::StringVal(
                format!("<unsupported type {:?}>", value.kind()),
            )));
            idx
        }
    }
}

/// Convert our value-tree representation back to wasm_wave::Value.
/// Requires the wave type to reconstruct properly typed values.
pub fn value_tree_to_wave(tree: &ValueTree, wave_ty: &WaveType) -> WaveValue {
    if tree.nodes.is_empty() {
        // Empty tree, return a unit value
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
    let Some(node) = nodes.get(idx as usize) else {
        return WaveValue::make_bool(false);
    };

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
