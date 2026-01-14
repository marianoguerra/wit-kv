//! Direct wasmtime::component::Val lowering to binary canonical ABI format.
//!
//! This is the hot path for TypedRunner, bypassing wasm_wave::Value.

use wasmtime::component::Val;
use wit_parser::{FlagsRepr, Int, Type, TypeDefKind};

use super::buffer::{align_to, write_byte, write_slice};
use super::{CanonicalAbi, CanonicalAbiError, LinearMemory};

impl CanonicalAbi<'_> {
    /// Lower a wasmtime::component::Val directly to binary.
    /// This is the hot path for TypedRunner, bypassing wasm_wave::Value.
    pub fn lower_from_val(
        &self,
        val: &Val,
        wit_ty: &Type,
        memory: &mut LinearMemory,
    ) -> Result<Vec<u8>, CanonicalAbiError> {
        let size = self.sizes.size(wit_ty).size_wasm32();
        let mut buffer = vec![0u8; size];
        self.lower_val_into(val, wit_ty, &mut buffer, 0, memory)?;
        Ok(buffer)
    }

    /// Lower a Val into a buffer at the given offset.
    fn lower_val_into(
        &self,
        val: &Val,
        wit_ty: &Type,
        buffer: &mut [u8],
        offset: usize,
        memory: &mut LinearMemory,
    ) -> Result<(), CanonicalAbiError> {
        match (wit_ty, val) {
            (Type::Bool, Val::Bool(v)) => {
                write_byte(buffer, offset, if *v { 1 } else { 0 })?;
            }
            (Type::U8, Val::U8(v)) => {
                write_byte(buffer, offset, *v)?;
            }
            (Type::S8, Val::S8(v)) => {
                write_byte(buffer, offset, *v as u8)?;
            }
            (Type::U16, Val::U16(v)) => {
                let aligned = align_to(offset, 2);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::S16, Val::S16(v)) => {
                let aligned = align_to(offset, 2);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::U32, Val::U32(v)) => {
                let aligned = align_to(offset, 4);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::S32, Val::S32(v)) => {
                let aligned = align_to(offset, 4);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::U64, Val::U64(v)) => {
                let aligned = align_to(offset, 8);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::S64, Val::S64(v)) => {
                let aligned = align_to(offset, 8);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::F32, Val::Float32(v)) => {
                let aligned = align_to(offset, 4);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::F64, Val::Float64(v)) => {
                let aligned = align_to(offset, 8);
                write_slice(buffer, aligned, &v.to_le_bytes())?;
            }
            (Type::Char, Val::Char(c)) => {
                let aligned = align_to(offset, 4);
                write_slice(buffer, aligned, &(*c as u32).to_le_bytes())?;
            }
            (Type::String, Val::String(s)) => {
                let aligned = align_to(offset, 4);
                let ptr = memory.alloc(s.len(), 1);
                memory.write(ptr, s.as_bytes());
                write_slice(buffer, aligned, &ptr.to_le_bytes())?;
                write_slice(buffer, aligned + 4, &(s.len() as u32).to_le_bytes())?;
            }
            (Type::Id(id), _) => {
                self.lower_val_type_id(val, *id, buffer, offset, memory)?;
            }
            (Type::ErrorContext, _) => {
                return Err(CanonicalAbiError::UnsupportedType(
                    "error-context".to_string(),
                ));
            }
            _ => {
                return Err(CanonicalAbiError::TypeMismatch {
                    expected: format!("{:?}", wit_ty),
                    got: format!("{:?}", val),
                });
            }
        }
        Ok(())
    }

    fn lower_val_type_id(
        &self,
        val: &Val,
        id: wit_parser::TypeId,
        buffer: &mut [u8],
        offset: usize,
        memory: &mut LinearMemory,
    ) -> Result<(), CanonicalAbiError> {
        let ty_def = self.resolve.types.get(id).ok_or_else(|| {
            CanonicalAbiError::UnsupportedType(format!("Unknown type id: {:?}", id))
        })?;

        match &ty_def.kind {
            TypeDefKind::Type(t) => {
                self.lower_val_into(val, t, buffer, offset, memory)?;
            }
            TypeDefKind::Record(r) => {
                let fields = match val {
                    Val::Record(f) => f,
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "record".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                };

                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(r.fields.iter().map(|f| &f.ty))
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();

                for (i, field_def) in r.fields.iter().enumerate() {
                    let (field_off, _) = field_offsets.get(i).ok_or_else(|| {
                        CanonicalAbiError::TypeMismatch {
                            expected: format!("field offset at index {}", i),
                            got: "missing".to_string(),
                        }
                    })?;
                    let (_, field_val) = fields
                        .iter()
                        .find(|(name, _)| name == &field_def.name)
                        .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                            expected: format!("field '{}'", field_def.name),
                            got: "missing".to_string(),
                        })?;
                    self.lower_val_into(field_val, &field_def.ty, buffer, offset + field_off, memory)?;
                }
            }
            TypeDefKind::Tuple(t) => {
                let elements = match val {
                    Val::Tuple(e) => e,
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "tuple".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                };

                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(t.types.iter())
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();

                for (i, wit_ty) in t.types.iter().enumerate() {
                    let (field_off, _) = field_offsets.get(i).ok_or_else(|| {
                        CanonicalAbiError::TypeMismatch {
                            expected: format!("tuple offset at index {}", i),
                            got: "missing".to_string(),
                        }
                    })?;
                    let elem = elements.get(i).ok_or_else(|| CanonicalAbiError::TypeMismatch {
                        expected: format!("tuple element at index {}", i),
                        got: "missing".to_string(),
                    })?;
                    self.lower_val_into(elem, wit_ty, buffer, offset + field_off, memory)?;
                }
            }
            TypeDefKind::List(elem_ty) => {
                let elements = match val {
                    Val::List(e) => e,
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "list".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                };

                let aligned = align_to(offset, 4);
                let elem_size = self.sizes.size(elem_ty).size_wasm32();
                let elem_align = self.sizes.align(elem_ty).align_wasm32();

                let ptr = memory.alloc(elements.len() * elem_size, elem_align);

                for (i, elem) in elements.iter().enumerate() {
                    let elem_offset = ptr as usize + i * elem_size;
                    let mut elem_buf = vec![0u8; elem_size];
                    self.lower_val_into(elem, elem_ty, &mut elem_buf, 0, memory)?;
                    memory.write(elem_offset as u32, &elem_buf);
                }

                write_slice(buffer, aligned, &ptr.to_le_bytes())?;
                write_slice(buffer, aligned + 4, &(elements.len() as u32).to_le_bytes())?;
            }
            TypeDefKind::Option(inner_ty) => {
                let payload_offset = self.sizes.payload_offset(Int::U8, [Some(inner_ty)]);

                match val {
                    Val::Option(Some(inner_val)) => {
                        write_byte(buffer, offset, 1)?;
                        self.lower_val_into(
                            inner_val,
                            inner_ty,
                            buffer,
                            offset + payload_offset.size_wasm32(),
                            memory,
                        )?;
                    }
                    Val::Option(None) => {
                        write_byte(buffer, offset, 0)?;
                    }
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "option".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                }
            }
            TypeDefKind::Result(r) => {
                let payload_offset = self
                    .sizes
                    .payload_offset(Int::U8, [r.ok.as_ref(), r.err.as_ref()]);

                match val {
                    Val::Result(Ok(ok_val)) => {
                        write_byte(buffer, offset, 0)?;
                        if let (Some(ok_ty), Some(v)) = (&r.ok, ok_val) {
                            self.lower_val_into(
                                v,
                                ok_ty,
                                buffer,
                                offset + payload_offset.size_wasm32(),
                                memory,
                            )?;
                        }
                    }
                    Val::Result(Err(err_val)) => {
                        write_byte(buffer, offset, 1)?;
                        if let (Some(err_ty), Some(v)) = (&r.err, err_val) {
                            self.lower_val_into(
                                v,
                                err_ty,
                                buffer,
                                offset + payload_offset.size_wasm32(),
                                memory,
                            )?;
                        }
                    }
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "result".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                }
            }
            TypeDefKind::Variant(v) => {
                let (case_name, payload) = match val {
                    Val::Variant(name, p) => (name, p),
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "variant".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                };

                let case_idx = v
                    .cases
                    .iter()
                    .position(|c| &c.name == case_name)
                    .ok_or(CanonicalAbiError::InvalidDiscriminant {
                        discriminant: 0,
                        num_cases: v.cases.len(),
                    })?;

                self.write_discriminant(buffer, offset, v.tag(), case_idx as u32)?;

                if let Some(payload_val) = payload {
                    let payload_offset = self
                        .sizes
                        .payload_offset(v.tag(), v.cases.iter().map(|c| c.ty.as_ref()));
                    let case = v.cases.get(case_idx).ok_or(
                        CanonicalAbiError::InvalidDiscriminant {
                            discriminant: case_idx as u32,
                            num_cases: v.cases.len(),
                        },
                    )?;
                    if let Some(payload_ty) = &case.ty {
                        self.lower_val_into(
                            payload_val,
                            payload_ty,
                            buffer,
                            offset + payload_offset.size_wasm32(),
                            memory,
                        )?;
                    }
                }
            }
            TypeDefKind::Enum(e) => {
                let case_name = match val {
                    Val::Enum(name) => name,
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "enum".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                };

                let case_idx = e
                    .cases
                    .iter()
                    .position(|c| &c.name == case_name)
                    .ok_or(CanonicalAbiError::InvalidDiscriminant {
                        discriminant: 0,
                        num_cases: e.cases.len(),
                    })?;

                self.write_discriminant(buffer, offset, e.tag(), case_idx as u32)?;
            }
            TypeDefKind::Flags(f) => {
                let active_flags = match val {
                    Val::Flags(flags) => flags,
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "flags".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                };

                let mut flags_value = 0u32;
                for flag in active_flags {
                    if let Some(pos) = f.flags.iter().position(|fl| &fl.name == flag) {
                        flags_value |= 1 << pos;
                    }
                }

                match f.repr() {
                    FlagsRepr::U8 => {
                        write_byte(buffer, offset, flags_value as u8)?;
                    }
                    FlagsRepr::U16 => {
                        let aligned = align_to(offset, 2);
                        write_slice(buffer, aligned, &(flags_value as u16).to_le_bytes())?;
                    }
                    FlagsRepr::U32(n) => {
                        let aligned = align_to(offset, 4);
                        for i in 0..n {
                            let word = if i == 0 { flags_value } else { 0 };
                            let word_offset = aligned + (i * 4);
                            write_slice(buffer, word_offset, &word.to_le_bytes())?;
                        }
                    }
                }
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
                let elements = match val {
                    Val::List(e) => e,
                    _ => {
                        return Err(CanonicalAbiError::TypeMismatch {
                            expected: "list".to_string(),
                            got: format!("{:?}", val),
                        })
                    }
                };

                let elem_size = self.sizes.size(elem_ty).size_wasm32();
                for i in 0..*len as usize {
                    if let Some(elem) = elements.get(i) {
                        self.lower_val_into(elem, elem_ty, buffer, offset + i * elem_size, memory)?;
                    }
                }
            }
            TypeDefKind::Map(_, _) => {
                return Err(CanonicalAbiError::UnsupportedType("map".to_string()));
            }
            TypeDefKind::Unknown => {
                return Err(CanonicalAbiError::UnsupportedType("unknown".to_string()));
            }
        }
        Ok(())
    }
}
