//! WAVE value lowering to binary canonical ABI format.

use wasm_wave::value::{Type as WaveType, Value};
use wasm_wave::wasm::{WasmType, WasmValue};
use wit_parser::{FlagsRepr, Int, Type, TypeDefKind};

use super::buffer::{align_to, write_byte, write_slice};
use super::{CanonicalAbi, CanonicalAbiError, LinearMemory};

impl CanonicalAbi<'_> {
    /// Lower a WAVE value to binary according to canonical ABI.
    /// For types without variable-length data (strings/lists), no linear memory is needed.
    pub fn lower(
        &self,
        value: &Value,
        wit_ty: &Type,
        wave_ty: &WaveType,
    ) -> Result<Vec<u8>, CanonicalAbiError> {
        let size = self.sizes.size(wit_ty).size_wasm32();
        let mut buffer = vec![0u8; size];
        self.lower_into(value, wit_ty, wave_ty, &mut buffer, 0, None)?;
        Ok(buffer)
    }

    /// Lower a WAVE value to binary with linear memory for variable-length types.
    /// Returns the main buffer and updates the linear memory with string/list data.
    pub fn lower_with_memory(
        &self,
        value: &Value,
        wit_ty: &Type,
        wave_ty: &WaveType,
        memory: &mut LinearMemory,
    ) -> Result<Vec<u8>, CanonicalAbiError> {
        let size = self.sizes.size(wit_ty).size_wasm32();
        let mut buffer = vec![0u8; size];
        self.lower_into(value, wit_ty, wave_ty, &mut buffer, 0, Some(memory))?;
        Ok(buffer)
    }

    /// Lower a value into a buffer at the given offset.
    pub(super) fn lower_into(
        &self,
        value: &Value,
        wit_ty: &Type,
        wave_ty: &WaveType,
        buffer: &mut [u8],
        offset: usize,
        memory: Option<&mut LinearMemory>,
    ) -> Result<(), CanonicalAbiError> {
        match wit_ty {
            Type::Bool => {
                let v = value.unwrap_bool();
                write_byte(buffer, offset, if v { 1 } else { 0 })?;
            }
            Type::U8 => {
                write_byte(buffer, offset, value.unwrap_u8())?;
            }
            Type::S8 => {
                write_byte(buffer, offset, value.unwrap_s8() as u8)?;
            }
            Type::U16 => {
                let aligned = align_to(offset, 2);
                let bytes = value.unwrap_u16().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::S16 => {
                let aligned = align_to(offset, 2);
                let bytes = value.unwrap_s16().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::U32 => {
                let aligned = align_to(offset, 4);
                let bytes = value.unwrap_u32().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::S32 => {
                let aligned = align_to(offset, 4);
                let bytes = value.unwrap_s32().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::U64 => {
                let aligned = align_to(offset, 8);
                let bytes = value.unwrap_u64().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::S64 => {
                let aligned = align_to(offset, 8);
                let bytes = value.unwrap_s64().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::F32 => {
                let aligned = align_to(offset, 4);
                let bytes = value.unwrap_f32().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::F64 => {
                let aligned = align_to(offset, 8);
                let bytes = value.unwrap_f64().to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::Char => {
                let aligned = align_to(offset, 4);
                let bytes = (value.unwrap_char() as u32).to_le_bytes();
                write_slice(buffer, aligned, &bytes)?;
            }
            Type::String => {
                // Strings are stored as ptr + len in the canonical ABI
                let s = value.unwrap_string();
                let aligned = align_to(offset, 4);

                match memory {
                    Some(mem) => {
                        // Allocate space in linear memory and write the string bytes
                        let ptr = mem.alloc(s.len(), 1); // UTF-8 strings have alignment 1
                        mem.write(ptr, s.as_bytes());

                        // Store ptr and len in the buffer
                        let ptr_bytes = ptr.to_le_bytes();
                        let len_bytes = (s.len() as u32).to_le_bytes();
                        write_slice(buffer, aligned, &ptr_bytes)?;
                        write_slice(buffer, aligned + 4, &len_bytes)?;
                    }
                    None => {
                        return Err(CanonicalAbiError::LinearMemoryRequired(
                            "string".to_string(),
                        ));
                    }
                }
            }
            Type::Id(id) => {
                self.lower_type_id(value, *id, wave_ty, buffer, offset, memory)?;
            }
            Type::ErrorContext => {
                return Err(CanonicalAbiError::UnsupportedType(
                    "error-context".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn lower_type_id(
        &self,
        value: &Value,
        id: wit_parser::TypeId,
        wave_ty: &WaveType,
        buffer: &mut [u8],
        offset: usize,
        mut memory: Option<&mut LinearMemory>,
    ) -> Result<(), CanonicalAbiError> {
        let ty_def = self.resolve.types.get(id).ok_or_else(|| {
            CanonicalAbiError::UnsupportedType(format!("Unknown type id: {:?}", id))
        })?;
        match &ty_def.kind {
            TypeDefKind::Type(t) => {
                self.lower_into(value, t, wave_ty, buffer, offset, memory)?;
            }
            TypeDefKind::Record(r) => {
                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(r.fields.iter().map(|f| &f.ty))
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();
                let wave_fields: Vec<_> = wave_ty.record_fields().collect();
                let field_values: Vec<_> = value.unwrap_record().collect();

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
                    let (_, field_val) =
                        field_values
                            .get(i)
                            .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                                expected: format!("field value at index {}", i),
                                got: "missing".to_string(),
                            })?;
                    self.lower_into(
                        field_val.as_ref(),
                        &field_def.ty,
                        wave_field_ty,
                        buffer,
                        offset + field_off,
                        memory.as_deref_mut(),
                    )?;
                }
            }
            TypeDefKind::Tuple(t) => {
                let field_offsets: Vec<_> = self
                    .sizes
                    .field_offsets(t.types.iter())
                    .into_iter()
                    .map(|(off, ty)| (off.size_wasm32(), *ty))
                    .collect();
                let wave_types: Vec<_> = wave_ty.tuple_element_types().collect();
                let elem_values: Vec<_> = value.unwrap_tuple().collect();

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
                    let elem =
                        elem_values
                            .get(i)
                            .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                                expected: format!("tuple element at index {}", i),
                                got: "missing".to_string(),
                            })?;
                    self.lower_into(
                        elem.as_ref(),
                        wit_ty,
                        wave_elem_ty,
                        buffer,
                        offset + field_off,
                        memory.as_deref_mut(),
                    )?;
                }
            }
            TypeDefKind::Flags(f) => {
                let flag_names: Vec<_> = wave_ty.flags_names().collect();
                let active_flags: Vec<_> = value.unwrap_flags().collect();

                // Calculate the flags value
                let mut flags_value = 0u32;
                for flag in active_flags {
                    if let Some(pos) = flag_names.iter().position(|n| *n == *flag) {
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
                        // For multi-word flags, this is simplified
                        for i in 0..n {
                            let word = if i == 0 { flags_value } else { 0 };
                            let word_offset = aligned + (i * 4);
                            write_slice(buffer, word_offset, &word.to_le_bytes())?;
                        }
                    }
                }
            }
            TypeDefKind::Enum(e) => {
                let case_name = value.unwrap_enum();
                let case_idx = e.cases.iter().position(|c| c.name == *case_name).ok_or(
                    CanonicalAbiError::InvalidDiscriminant {
                        discriminant: 0,
                        num_cases: e.cases.len(),
                    },
                )?;

                self.write_discriminant(buffer, offset, e.tag(), case_idx as u32)?;
            }
            TypeDefKind::Variant(v) => {
                let (case_name, payload) = value.unwrap_variant();
                let case_idx = v.cases.iter().position(|c| c.name == *case_name).ok_or(
                    CanonicalAbiError::InvalidDiscriminant {
                        discriminant: 0,
                        num_cases: v.cases.len(),
                    },
                )?;

                self.write_discriminant(buffer, offset, v.tag(), case_idx as u32)?;

                if let Some(payload_value) = payload {
                    let payload_offset = self
                        .sizes
                        .payload_offset(v.tag(), v.cases.iter().map(|c| c.ty.as_ref()));
                    let case =
                        v.cases
                            .get(case_idx)
                            .ok_or(CanonicalAbiError::InvalidDiscriminant {
                                discriminant: case_idx as u32,
                                num_cases: v.cases.len(),
                            })?;
                    if let Some(payload_ty) = &case.ty {
                        let wave_cases: Vec<_> = wave_ty.variant_cases().collect();
                        if let Some((_, Some(wave_payload_ty))) = wave_cases.get(case_idx) {
                            self.lower_into(
                                &payload_value,
                                payload_ty,
                                wave_payload_ty,
                                buffer,
                                offset + payload_offset.size_wasm32(),
                                memory.as_deref_mut(),
                            )?;
                        }
                    }
                }
            }
            TypeDefKind::Option(inner_ty) => {
                let opt_value = value.unwrap_option();
                match opt_value {
                    Some(inner) => {
                        write_byte(buffer, offset, 1)?;
                        let payload_offset = self.sizes.payload_offset(Int::U8, [Some(inner_ty)]);
                        let wave_inner_ty = wave_ty.option_some_type().ok_or_else(|| {
                            CanonicalAbiError::TypeMismatch {
                                expected: "option".to_string(),
                                got: "non-option".to_string(),
                            }
                        })?;
                        self.lower_into(
                            &inner,
                            inner_ty,
                            &wave_inner_ty,
                            buffer,
                            offset + payload_offset.size_wasm32(),
                            memory,
                        )?;
                    }
                    None => {
                        write_byte(buffer, offset, 0)?;
                    }
                }
            }
            TypeDefKind::Result(r) => {
                let result_value = value.unwrap_result();
                let (ok_ty, err_ty) =
                    wave_ty
                        .result_types()
                        .ok_or_else(|| CanonicalAbiError::TypeMismatch {
                            expected: "result".to_string(),
                            got: "non-result".to_string(),
                        })?;

                let payload_offset = self
                    .sizes
                    .payload_offset(Int::U8, [r.ok.as_ref(), r.err.as_ref()]);

                match result_value {
                    Ok(ok_val) => {
                        write_byte(buffer, offset, 0)?;
                        if let (Some(val), Some(wit_ok_ty), Some(wave_ok_ty)) =
                            (ok_val, &r.ok, &ok_ty)
                        {
                            self.lower_into(
                                &val,
                                wit_ok_ty,
                                wave_ok_ty,
                                buffer,
                                offset + payload_offset.size_wasm32(),
                                memory.as_deref_mut(),
                            )?;
                        }
                    }
                    Err(err_val) => {
                        write_byte(buffer, offset, 1)?;
                        if let (Some(val), Some(wit_err_ty), Some(wave_err_ty)) =
                            (err_val, &r.err, &err_ty)
                        {
                            self.lower_into(
                                &val,
                                wit_err_ty,
                                wave_err_ty,
                                buffer,
                                offset + payload_offset.size_wasm32(),
                                memory.as_deref_mut(),
                            )?;
                        }
                    }
                }
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

                let elem_values: Vec<_> = value.unwrap_list().collect();
                let len = elem_values.len();

                match memory {
                    Some(mem) => {
                        let elem_size = self.sizes.size(elem_ty).size_wasm32();
                        let elem_align = self.sizes.align(elem_ty).align_wasm32();

                        // Allocate space for all elements
                        let ptr = mem.alloc(len * elem_size, elem_align);

                        // Lower each element into linear memory
                        for (i, elem) in elem_values.into_iter().enumerate() {
                            let elem_offset = ptr as usize + i * elem_size;
                            // Ensure the memory is large enough
                            let end = elem_offset + elem_size;
                            if end > mem.as_bytes().len() {
                                mem.write(end as u32 - 1, &[0]); // Extend memory
                            }

                            // We need to lower into linear memory, not the buffer
                            // Create a temporary buffer for the element
                            let mut elem_buf = vec![0u8; elem_size];
                            self.lower_into(
                                &elem,
                                elem_ty,
                                &wave_elem_ty,
                                &mut elem_buf,
                                0,
                                Some(mem),
                            )?;
                            mem.write(elem_offset as u32, &elem_buf);
                        }

                        // Store ptr and len in the buffer
                        let ptr_bytes = ptr.to_le_bytes();
                        let len_bytes = (len as u32).to_le_bytes();
                        write_slice(buffer, aligned, &ptr_bytes)?;
                        write_slice(buffer, aligned + 4, &len_bytes)?;
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

                let elem_values: Vec<_> = value.unwrap_list().collect();
                for i in 0..*len as usize {
                    let Some(elem) = elem_values.get(i) else {
                        break;
                    };
                    self.lower_into(
                        elem.as_ref(),
                        elem_ty,
                        &wave_elem_ty,
                        buffer,
                        offset + i * elem_size,
                        memory.as_deref_mut(),
                    )?;
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

    pub(super) fn write_discriminant(
        &self,
        buffer: &mut [u8],
        offset: usize,
        tag: Int,
        value: u32,
    ) -> Result<(), CanonicalAbiError> {
        match tag {
            Int::U8 => {
                write_byte(buffer, offset, value as u8)?;
            }
            Int::U16 => {
                let aligned = align_to(offset, 2);
                write_slice(buffer, aligned, &(value as u16).to_le_bytes())?;
            }
            Int::U32 => {
                let aligned = align_to(offset, 4);
                write_slice(buffer, aligned, &value.to_le_bytes())?;
            }
            Int::U64 => {
                let aligned = align_to(offset, 8);
                write_slice(buffer, aligned, &(value as u64).to_le_bytes())?;
            }
        }
        Ok(())
    }
}
