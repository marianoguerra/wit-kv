//! WAVE value lifting from binary canonical ABI format.

use wasm_wave::value::{Type as WaveType, Value};
use wasm_wave::wasm::{WasmType, WasmValue};
use wit_parser::{FlagsRepr, Int, Type, TypeDefKind};

use super::buffer::{align_to, read_byte, read_slice};
use super::{CanonicalAbi, CanonicalAbiError, LinearMemory};

impl CanonicalAbi<'_> {
    /// Lift binary data to a WAVE value according to canonical ABI.
    /// For types without variable-length data (strings/lists), no linear memory is needed.
    pub fn lift(
        &self,
        buffer: &[u8],
        wit_ty: &Type,
        wave_ty: &WaveType,
    ) -> Result<(Value, usize), CanonicalAbiError> {
        self.lift_from(buffer, wit_ty, wave_ty, 0, None)
    }

    /// Lift binary data to a WAVE value with linear memory for variable-length types.
    pub fn lift_with_memory(
        &self,
        buffer: &[u8],
        wit_ty: &Type,
        wave_ty: &WaveType,
        memory: &LinearMemory,
    ) -> Result<(Value, usize), CanonicalAbiError> {
        self.lift_from(buffer, wit_ty, wave_ty, 0, Some(memory))
    }

    /// Lift a value from a buffer at the given offset.
    fn lift_from(
        &self,
        buffer: &[u8],
        wit_ty: &Type,
        wave_ty: &WaveType,
        offset: usize,
        memory: Option<&LinearMemory>,
    ) -> Result<(Value, usize), CanonicalAbiError> {
        let size = self.sizes.size(wit_ty).size_wasm32();
        if offset + size > buffer.len() {
            return Err(CanonicalAbiError::BufferTooSmall {
                needed: offset + size,
                available: buffer.len(),
            });
        }

        let value = match wit_ty {
            Type::Bool => {
                let v = read_byte(buffer, offset)?;
                match v {
                    0 => Value::make_bool(false),
                    1 => Value::make_bool(true),
                    _ => return Err(CanonicalAbiError::InvalidBool(v)),
                }
            }
            Type::U8 => Value::make_u8(read_byte(buffer, offset)?),
            Type::S8 => Value::make_s8(read_byte(buffer, offset)? as i8),
            Type::U16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = read_slice(buffer, aligned, 2)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    }
                })?;
                Value::make_u16(u16::from_le_bytes(bytes))
            }
            Type::S16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = read_slice(buffer, aligned, 2)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    }
                })?;
                Value::make_s16(i16::from_le_bytes(bytes))
            }
            Type::U32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    }
                })?;
                Value::make_u32(u32::from_le_bytes(bytes))
            }
            Type::S32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    }
                })?;
                Value::make_s32(i32::from_le_bytes(bytes))
            }
            Type::U64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = read_slice(buffer, aligned, 8)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    }
                })?;
                Value::make_u64(u64::from_le_bytes(bytes))
            }
            Type::S64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = read_slice(buffer, aligned, 8)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    }
                })?;
                Value::make_s64(i64::from_le_bytes(bytes))
            }
            Type::F32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    }
                })?;
                Value::make_f32(f32::from_le_bytes(bytes))
            }
            Type::F64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = read_slice(buffer, aligned, 8)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    }
                })?;
                Value::make_f64(f64::from_le_bytes(bytes))
            }
            Type::Char => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    }
                })?;
                let code = u32::from_le_bytes(bytes);
                let c = char::from_u32(code).ok_or(CanonicalAbiError::InvalidChar(code))?;
                Value::make_char(c)
            }
            Type::String => {
                // Strings are stored as ptr + len in the canonical ABI
                let aligned = align_to(offset, 4);

                match memory {
                    Some(mem) => {
                        // Read ptr and len from buffer
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

                        // Read string bytes from linear memory
                        let string_bytes = mem.read(ptr, len)?;
                        let s = std::str::from_utf8(string_bytes)
                            .map_err(|_| CanonicalAbiError::InvalidUtf8)?;
                        Value::make_string(std::borrow::Cow::Owned(s.to_string()))
                    }
                    None => {
                        return Err(CanonicalAbiError::LinearMemoryRequired(
                            "string".to_string(),
                        ));
                    }
                }
            }
            Type::Id(id) => {
                return self.lift_type_id(buffer, *id, wave_ty, offset, memory);
            }
            Type::ErrorContext => {
                return Err(CanonicalAbiError::UnsupportedType(
                    "error-context".to_string(),
                ));
            }
        };

        Ok((value, size))
    }

    fn lift_type_id(
        &self,
        buffer: &[u8],
        id: wit_parser::TypeId,
        wave_ty: &WaveType,
        offset: usize,
        memory: Option<&LinearMemory>,
    ) -> Result<(Value, usize), CanonicalAbiError> {
        let ty_def = self.resolve.types.get(id).ok_or_else(|| {
            CanonicalAbiError::UnsupportedType(format!("Unknown type id: {:?}", id))
        })?;
        let size = self.sizes.size(&Type::Id(id)).size_wasm32();

        let value = match &ty_def.kind {
            TypeDefKind::Type(t) => {
                let (val, _) = self.lift_from(buffer, t, wave_ty, offset, memory)?;
                return Ok((val, size));
            }
            TypeDefKind::Record(r) => {
                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(r.fields.iter().map(|f| &f.ty))
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();
                let wave_fields: Vec<_> = wave_ty.record_fields().collect();

                let mut fields: Vec<(&str, Value)> = Vec::new();
                for (i, field_def) in r.fields.iter().enumerate() {
                    let (field_off, _) =
                        field_offsets
                            .get(i)
                            .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                                expected: format!("field offset at index {}", i),
                                got: "missing".to_string(),
                            })?;
                    let (_, wave_field_ty) =
                        wave_fields
                            .get(i)
                            .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                                expected: format!("wave field at index {}", i),
                                got: "missing".to_string(),
                            })?;
                    let (field_val, _) = self.lift_from(
                        buffer,
                        &field_def.ty,
                        wave_field_ty,
                        offset + field_off,
                        memory,
                    )?;
                    fields.push((&field_def.name, field_val));
                }

                Value::make_record(wave_ty, fields).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "record".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::Tuple(t) => {
                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(t.types.iter())
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();
                let wave_types: Vec<_> = wave_ty.tuple_element_types().collect();

                let mut elements: Vec<Value> = Vec::new();
                for (i, wit_ty) in t.types.iter().enumerate() {
                    let (field_off, _) =
                        field_offsets
                            .get(i)
                            .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                                expected: format!("tuple offset at index {}", i),
                                got: "missing".to_string(),
                            })?;
                    let wave_elem_ty =
                        wave_types
                            .get(i)
                            .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                                expected: format!("tuple wave type at index {}", i),
                                got: "missing".to_string(),
                            })?;
                    let (elem_val, _) =
                        self.lift_from(buffer, wit_ty, wave_elem_ty, offset + field_off, memory)?;
                    elements.push(elem_val);
                }

                Value::make_tuple(wave_ty, elements).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "tuple".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::Flags(f) => {
                let flag_names: Vec<_> = wave_ty.flags_names().collect();
                let flags_value =
                    match f.repr() {
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

                let active_flags: Vec<&str> = flag_names
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| (flags_value >> i) & 1 == 1)
                    .map(|(_, name)| name.as_ref())
                    .collect();

                Value::make_flags(wave_ty, active_flags).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "flags".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::Enum(e) => {
                let discriminant = self.read_discriminant(buffer, offset, e.tag())?;
                let case = e.cases.get(discriminant as usize).ok_or(
                    CanonicalAbiError::InvalidDiscriminant {
                        discriminant,
                        num_cases: e.cases.len(),
                    },
                )?;
                let case_name = &case.name;

                Value::make_enum(wave_ty, case_name).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "enum".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::Variant(v) => {
                let discriminant = self.read_discriminant(buffer, offset, v.tag())?;
                let case = v.cases.get(discriminant as usize).ok_or(
                    CanonicalAbiError::InvalidDiscriminant {
                        discriminant,
                        num_cases: v.cases.len(),
                    },
                )?;
                let payload_offset = self
                    .sizes
                    .payload_offset(v.tag(), v.cases.iter().map(|c| c.ty.as_ref()));

                let wave_cases: Vec<_> = wave_ty.variant_cases().collect();
                let payload = if let (Some(payload_ty), Some((_, Some(wave_payload_ty)))) =
                    (&case.ty, wave_cases.get(discriminant as usize))
                {
                    let (payload_val, _) = self.lift_from(
                        buffer,
                        payload_ty,
                        wave_payload_ty,
                        offset + payload_offset.size_wasm32(),
                        memory,
                    )?;
                    Some(payload_val)
                } else {
                    None
                };

                Value::make_variant(wave_ty, &case.name, payload).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "variant".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::Option(inner_ty) => {
                let discriminant = read_byte(buffer, offset)?;
                let payload_offset = self.sizes.payload_offset(Int::U8, [Some(inner_ty)]);
                let wave_inner_ty =
                    wave_ty
                        .option_some_type()
                        .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                            expected: "option".to_string(),
                            got: "non-option".to_string(),
                        })?;

                let opt_val = match discriminant {
                    0 => None,
                    1 => {
                        let (inner_val, _) = self.lift_from(
                            buffer,
                            inner_ty,
                            &wave_inner_ty,
                            offset + payload_offset.size_wasm32(),
                            memory,
                        )?;
                        Some(inner_val)
                    }
                    _ => {
                        return Err(CanonicalAbiError::InvalidDiscriminant {
                            discriminant: discriminant as u32,
                            num_cases: 2,
                        });
                    }
                };

                Value::make_option(wave_ty, opt_val).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "option".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::Result(r) => {
                let discriminant = read_byte(buffer, offset)?;
                let payload_offset = self
                    .sizes
                    .payload_offset(Int::U8, [r.ok.as_ref(), r.err.as_ref()]);
                let (wave_ok_ty, wave_err_ty) =
                    wave_ty
                        .result_types()
                        .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                            expected: "result".to_string(),
                            got: "non-result".to_string(),
                        })?;

                let result_val = match discriminant {
                    0 => {
                        let ok_val = if let (Some(ok_ty), Some(wave_ok)) = (&r.ok, wave_ok_ty) {
                            let (val, _) = self.lift_from(
                                buffer,
                                ok_ty,
                                &wave_ok,
                                offset + payload_offset.size_wasm32(),
                                memory,
                            )?;
                            Some(val)
                        } else {
                            None
                        };
                        Ok(ok_val)
                    }
                    1 => {
                        let err_val = if let (Some(err_ty), Some(wave_err)) = (&r.err, wave_err_ty)
                        {
                            let (val, _) = self.lift_from(
                                buffer,
                                err_ty,
                                &wave_err,
                                offset + payload_offset.size_wasm32(),
                                memory,
                            )?;
                            Some(val)
                        } else {
                            None
                        };
                        Err(err_val)
                    }
                    _ => {
                        return Err(CanonicalAbiError::InvalidDiscriminant {
                            discriminant: discriminant as u32,
                            num_cases: 2,
                        });
                    }
                };

                Value::make_result(wave_ty, result_val).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "result".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::List(elem_ty) => {
                // Lists are stored as ptr + len in the canonical ABI
                let aligned = align_to(offset, 4);
                let wave_elem_ty =
                    wave_ty
                        .list_element_type()
                        .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                            expected: "list".to_string(),
                            got: "non-list".to_string(),
                        })?;

                match memory {
                    Some(mem) => {
                        // Read ptr and len from buffer
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

                        // Read each element from linear memory
                        let mut elements: Vec<Value> = Vec::new();
                        for i in 0..len as usize {
                            let elem_offset = ptr as usize + i * elem_size;
                            let elem_bytes = mem.read(elem_offset as u32, elem_size as u32)?;

                            // Lift element from linear memory bytes
                            let (elem_val, _) =
                                self.lift_from(elem_bytes, elem_ty, &wave_elem_ty, 0, Some(mem))?;
                            elements.push(elem_val);
                        }

                        Value::make_list(wave_ty, elements).map_err(|e| {
                            CanonicalAbiError::TypeMismatch {
                                expected: "list".to_string(),
                                got: e.to_string(),
                            }
                        })?
                    }
                    None => {
                        return Err(CanonicalAbiError::LinearMemoryRequired("list".to_string()));
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
                let elem_size = self.sizes.size(elem_ty).size_wasm32();
                let wave_elem_ty =
                    wave_ty
                        .list_element_type()
                        .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                            expected: "list".to_string(),
                            got: "non-list".to_string(),
                        })?;

                let mut elements: Vec<Value> = Vec::new();
                for i in 0..*len as usize {
                    let (elem_val, _) = self.lift_from(
                        buffer,
                        elem_ty,
                        &wave_elem_ty,
                        offset + i * elem_size,
                        memory,
                    )?;
                    elements.push(elem_val);
                }

                Value::make_list(wave_ty, elements).map_err(|e| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "list".to_string(),
                        got: e.to_string(),
                    }
                })?
            }
            TypeDefKind::Map(_, _) => {
                return Err(CanonicalAbiError::UnsupportedType("map".to_string()));
            }
            TypeDefKind::Unknown => {
                return Err(CanonicalAbiError::UnsupportedType("unknown".to_string()));
            }
        };

        Ok((value, size))
    }

    pub(super) fn read_discriminant(
        &self,
        buffer: &[u8],
        offset: usize,
        tag: Int,
    ) -> Result<u32, CanonicalAbiError> {
        match tag {
            Int::U8 => Ok(read_byte(buffer, offset)? as u32),
            Int::U16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = read_slice(buffer, aligned, 2)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    }
                })?;
                Ok(u16::from_le_bytes(bytes) as u32)
            }
            Int::U32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = read_slice(buffer, aligned, 4)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    }
                })?;
                Ok(u32::from_le_bytes(bytes))
            }
            Int::U64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = read_slice(buffer, aligned, 8)?.try_into().map_err(|_| {
                    CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    }
                })?;
                Ok(u64::from_le_bytes(bytes) as u32)
            }
        }
    }
}
