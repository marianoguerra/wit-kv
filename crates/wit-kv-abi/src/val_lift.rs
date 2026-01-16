//! Direct wasmtime::component::Val lifting from binary canonical ABI format.
//!
//! This is the hot path for TypedRunner, bypassing wasm_wave::Value.

use wasmtime::component::{types as val_types, Val};
use wit_parser::{FlagsRepr, Int, Type, TypeDefKind};

use super::buffer::{align_to, read_byte, read_slice};
use super::{CanonicalAbi, CanonicalAbiError, LinearMemory};

impl CanonicalAbi<'_> {
    /// Lift binary data directly to wasmtime::component::Val.
    ///
    /// This is the hot path for TypedRunner, bypassing wasm_wave::Value.
    ///
    /// When `val_ty` is Some, it's used to validate type structure.
    /// When `val_ty` is None, the Val structure is derived purely from `wit_ty`.
    pub fn lift_to_val(
        &self,
        buffer: &[u8],
        wit_ty: &Type,
        val_ty: Option<&val_types::Type>,
        memory: &LinearMemory,
    ) -> Result<(Val, usize), CanonicalAbiError> {
        self.lift_val_from(buffer, wit_ty, val_ty, 0, memory)
    }

    /// Lift a Val from a buffer at the given offset.
    fn lift_val_from(
        &self,
        buffer: &[u8],
        wit_ty: &Type,
        val_ty: Option<&val_types::Type>,
        offset: usize,
        memory: &LinearMemory,
    ) -> Result<(Val, usize), CanonicalAbiError> {
        let size = self.sizes.size(wit_ty).size_wasm32();
        if offset + size > buffer.len() {
            return Err(CanonicalAbiError::BufferTooSmall {
                needed: offset + size,
                available: buffer.len(),
            });
        }

        let val = match wit_ty {
            Type::Bool => {
                let v = read_byte(buffer, offset)?;
                match v {
                    0 => Val::Bool(false),
                    1 => Val::Bool(true),
                    _ => return Err(CanonicalAbiError::InvalidBool(v)),
                }
            }
            Type::U8 => Val::U8(read_byte(buffer, offset)?),
            Type::S8 => Val::S8(read_byte(buffer, offset)? as i8),
            Type::U16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = read_slice(buffer, aligned, 2)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    })?;
                Val::U16(u16::from_le_bytes(bytes))
            }
            Type::S16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = read_slice(buffer, aligned, 2)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    })?;
                Val::S16(i16::from_le_bytes(bytes))
            }
            Type::U32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                Val::U32(u32::from_le_bytes(bytes))
            }
            Type::S32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                Val::S32(i32::from_le_bytes(bytes))
            }
            Type::U64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = read_slice(buffer, aligned, 8)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;
                Val::U64(u64::from_le_bytes(bytes))
            }
            Type::S64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = read_slice(buffer, aligned, 8)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;
                Val::S64(i64::from_le_bytes(bytes))
            }
            Type::F32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                Val::Float32(f32::from_le_bytes(bytes))
            }
            Type::F64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = read_slice(buffer, aligned, 8)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;
                Val::Float64(f64::from_le_bytes(bytes))
            }
            Type::Char => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                let code = u32::from_le_bytes(bytes);
                let c = char::from_u32(code).ok_or(CanonicalAbiError::InvalidChar(code))?;
                Val::Char(c)
            }
            Type::String => {
                // Strings are stored as ptr + len in the canonical ABI
                let aligned = align_to(offset, 4);
                let ptr_bytes: [u8; 4] = read_slice(buffer, aligned, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                let len_bytes: [u8; 4] = read_slice(buffer, aligned + 4, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;

                let ptr = u32::from_le_bytes(ptr_bytes);
                let len = u32::from_le_bytes(len_bytes);

                let string_bytes = memory.read(ptr, len)?;
                let s =
                    std::str::from_utf8(string_bytes).map_err(|_| CanonicalAbiError::InvalidUtf8)?;
                Val::String(s.to_string())
            }
            Type::Id(id) => {
                return self.lift_val_type_id(buffer, *id, val_ty, offset, memory);
            }
            Type::ErrorContext => {
                return Err(CanonicalAbiError::UnsupportedType(
                    "error-context".to_string(),
                ));
            }
        };

        Ok((val, size))
    }

    fn lift_val_type_id(
        &self,
        buffer: &[u8],
        id: wit_parser::TypeId,
        val_ty: Option<&val_types::Type>,
        offset: usize,
        memory: &LinearMemory,
    ) -> Result<(Val, usize), CanonicalAbiError> {
        let ty_def = self.resolve.types.get(id).ok_or_else(|| {
            CanonicalAbiError::UnsupportedType(format!("Unknown type id: {:?}", id))
        })?;
        let size = self.sizes.size(&Type::Id(id)).size_wasm32();

        let val = match &ty_def.kind {
            TypeDefKind::Type(t) => {
                let (v, _) = self.lift_val_from(buffer, t, val_ty, offset, memory)?;
                return Ok((v, size));
            }
            TypeDefKind::Record(r) => {
                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(r.fields.iter().map(|f| &f.ty))
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();

                let mut fields: Vec<(String, Val)> = Vec::with_capacity(r.fields.len());

                for (i, field_def) in r.fields.iter().enumerate() {
                    let (field_off, _) = field_offsets.get(i).ok_or_else(|| {
                        CanonicalAbiError::TypeMismatch {
                            expected: format!("field offset at index {}", i),
                            got: "missing".to_string(),
                        }
                    })?;
                    // Get inner val_ty if available
                    let inner_val_ty = val_ty.and_then(|vt| {
                        if let val_types::Type::Record(rt) = vt {
                            rt.fields()
                                .find(|f| f.name == field_def.name)
                                .map(|f| f.ty.clone())
                        } else {
                            None
                        }
                    });
                    let (field_val, _) = self.lift_val_from(
                        buffer,
                        &field_def.ty,
                        inner_val_ty.as_ref(),
                        offset + field_off,
                        memory,
                    )?;
                    fields.push((field_def.name.clone(), field_val));
                }

                Val::Record(fields)
            }
            TypeDefKind::Tuple(t) => {
                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(t.types.iter())
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();

                let mut elements: Vec<Val> = Vec::with_capacity(t.types.len());

                for (i, wit_ty) in t.types.iter().enumerate() {
                    let (field_off, _) = field_offsets.get(i).ok_or_else(|| {
                        CanonicalAbiError::TypeMismatch {
                            expected: format!("tuple offset at index {}", i),
                            got: "missing".to_string(),
                        }
                    })?;
                    // Get inner val_ty if available
                    let inner_val_ty = val_ty.and_then(|vt| {
                        if let val_types::Type::Tuple(tt) = vt {
                            tt.types().nth(i)
                        } else {
                            None
                        }
                    });
                    let (elem_val, _) = self.lift_val_from(
                        buffer,
                        wit_ty,
                        inner_val_ty.as_ref(),
                        offset + field_off,
                        memory,
                    )?;
                    elements.push(elem_val);
                }

                Val::Tuple(elements)
            }
            TypeDefKind::List(elem_ty) => {
                let aligned = align_to(offset, 4);
                let ptr_bytes: [u8; 4] = read_slice(buffer, aligned, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                let len_bytes: [u8; 4] = read_slice(buffer, aligned + 4, 4)?
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;

                let ptr = u32::from_le_bytes(ptr_bytes);
                let len = u32::from_le_bytes(len_bytes);

                let elem_size = self.sizes.size(elem_ty).size_wasm32();
                // Get inner val_ty if available
                let inner_val_ty = val_ty.and_then(|vt| {
                    if let val_types::Type::List(lt) = vt {
                        Some(lt.ty())
                    } else {
                        None
                    }
                });

                let mut elements: Vec<Val> = Vec::with_capacity(len as usize);
                for i in 0..len as usize {
                    let elem_offset = ptr as usize + i * elem_size;
                    let elem_bytes = memory.read(elem_offset as u32, elem_size as u32)?;
                    let (elem_val, _) =
                        self.lift_val_from(elem_bytes, elem_ty, inner_val_ty.as_ref(), 0, memory)?;
                    elements.push(elem_val);
                }

                Val::List(elements)
            }
            TypeDefKind::Option(inner_ty) => {
                let discriminant = read_byte(buffer, offset)?;
                let payload_offset = self.sizes.payload_offset(Int::U8, [Some(inner_ty)]);
                // Get inner val_ty if available
                let inner_val_ty = val_ty.and_then(|vt| {
                    if let val_types::Type::Option(ot) = vt {
                        Some(ot.ty())
                    } else {
                        None
                    }
                });

                match discriminant {
                    0 => Val::Option(None),
                    1 => {
                        let (inner_val, _) = self.lift_val_from(
                            buffer,
                            inner_ty,
                            inner_val_ty.as_ref(),
                            offset + payload_offset.size_wasm32(),
                            memory,
                        )?;
                        Val::Option(Some(Box::new(inner_val)))
                    }
                    _ => {
                        return Err(CanonicalAbiError::InvalidDiscriminant {
                            discriminant: discriminant as u32,
                            num_cases: 2,
                        })
                    }
                }
            }
            TypeDefKind::Result(r) => {
                let discriminant = read_byte(buffer, offset)?;
                let payload_offset = self
                    .sizes
                    .payload_offset(Int::U8, [r.ok.as_ref(), r.err.as_ref()]);

                // Get inner val_ty components if available
                let result_ty = val_ty.and_then(|vt| {
                    if let val_types::Type::Result(rt) = vt {
                        Some(rt)
                    } else {
                        None
                    }
                });

                match discriminant {
                    0 => {
                        let ok_val = if let Some(ok_ty) = &r.ok {
                            let ok_val_ty = result_ty.and_then(|rt| rt.ok());
                            let (val, _) = self.lift_val_from(
                                buffer,
                                ok_ty,
                                ok_val_ty.as_ref(),
                                offset + payload_offset.size_wasm32(),
                                memory,
                            )?;
                            Some(Box::new(val))
                        } else {
                            None
                        };
                        Val::Result(Ok(ok_val))
                    }
                    1 => {
                        let err_val = if let Some(err_ty) = &r.err {
                            let err_val_ty = result_ty.and_then(|rt| rt.err());
                            let (val, _) = self.lift_val_from(
                                buffer,
                                err_ty,
                                err_val_ty.as_ref(),
                                offset + payload_offset.size_wasm32(),
                                memory,
                            )?;
                            Some(Box::new(val))
                        } else {
                            None
                        };
                        Val::Result(Err(err_val))
                    }
                    _ => {
                        return Err(CanonicalAbiError::InvalidDiscriminant {
                            discriminant: discriminant as u32,
                            num_cases: 2,
                        })
                    }
                }
            }
            TypeDefKind::Variant(v) => {
                let discriminant = self.read_discriminant(buffer, offset, v.tag())?;
                let case = v
                    .cases
                    .get(discriminant as usize)
                    .ok_or(CanonicalAbiError::InvalidDiscriminant {
                        discriminant,
                        num_cases: v.cases.len(),
                    })?;
                let payload_offset = self
                    .sizes
                    .payload_offset(v.tag(), v.cases.iter().map(|c| c.ty.as_ref()));

                let payload = if let Some(payload_ty) = &case.ty {
                    // Get inner val_ty if available
                    let payload_val_ty = val_ty.and_then(|vt| {
                        if let val_types::Type::Variant(variant_ty) = vt {
                            variant_ty
                                .cases()
                                .find(|c| c.name == case.name)
                                .and_then(|c| c.ty.clone())
                        } else {
                            None
                        }
                    });
                    let (payload_val, _) = self.lift_val_from(
                        buffer,
                        payload_ty,
                        payload_val_ty.as_ref(),
                        offset + payload_offset.size_wasm32(),
                        memory,
                    )?;
                    Some(Box::new(payload_val))
                } else {
                    None
                };

                Val::Variant(case.name.clone(), payload)
            }
            TypeDefKind::Enum(e) => {
                let discriminant = self.read_discriminant(buffer, offset, e.tag())?;
                let case = e
                    .cases
                    .get(discriminant as usize)
                    .ok_or(CanonicalAbiError::InvalidDiscriminant {
                        discriminant,
                        num_cases: e.cases.len(),
                    })?;
                Val::Enum(case.name.clone())
            }
            TypeDefKind::Flags(f) => {
                let flag_names: Vec<_> = f.flags.iter().map(|flag| &flag.name).collect();
                let flags_value = match f.repr() {
                    FlagsRepr::U8 => read_byte(buffer, offset)? as u32,
                    FlagsRepr::U16 => {
                        let aligned = align_to(offset, 2);
                        let bytes: [u8; 2] = read_slice(buffer, aligned, 2)?
                            .try_into()
                            .map_err(|_| CanonicalAbiError::BufferTooSmall {
                                needed: aligned + 2,
                                available: buffer.len(),
                            })?;
                        u16::from_le_bytes(bytes) as u32
                    }
                    FlagsRepr::U32(_) => {
                        let aligned = align_to(offset, 4);
                        let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?
                            .try_into()
                            .map_err(|_| CanonicalAbiError::BufferTooSmall {
                                needed: aligned + 4,
                                available: buffer.len(),
                            })?;
                        u32::from_le_bytes(bytes)
                    }
                };

                let active_flags: Vec<String> = flag_names
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| (flags_value >> i) & 1 == 1)
                    .map(|(_, name)| (*name).clone())
                    .collect();

                Val::Flags(active_flags)
            }
            TypeDefKind::Handle(_) => {
                return Err(CanonicalAbiError::UnsupportedType("handle".to_string()));
            }
            TypeDefKind::Resource => {
                return Err(CanonicalAbiError::UnsupportedType("resource".to_string()));
            }
            TypeDefKind::Future(_) => {
                return Err(CanonicalAbiError::UnsupportedType("future".to_string()));
            }
            TypeDefKind::Stream(_) => {
                return Err(CanonicalAbiError::UnsupportedType("stream".to_string()));
            }
            TypeDefKind::FixedSizeList(elem_ty, len) => {
                let elem_size = self.sizes.size(elem_ty).size_wasm32();
                // Get inner val_ty if available
                let inner_val_ty = val_ty.and_then(|vt| {
                    if let val_types::Type::List(lt) = vt {
                        Some(lt.ty())
                    } else {
                        None
                    }
                });

                let mut elements: Vec<Val> = Vec::with_capacity(*len as usize);
                for i in 0..*len as usize {
                    let (elem_val, _) = self.lift_val_from(
                        buffer,
                        elem_ty,
                        inner_val_ty.as_ref(),
                        offset + i * elem_size,
                        memory,
                    )?;
                    elements.push(elem_val);
                }

                Val::List(elements)
            }
            TypeDefKind::Map(_, _) => {
                return Err(CanonicalAbiError::UnsupportedType("map".to_string()));
            }
            TypeDefKind::Unknown => {
                return Err(CanonicalAbiError::UnsupportedType("unknown".to_string()));
            }
        };

        Ok((val, size))
    }
}
