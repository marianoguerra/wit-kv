//! Canonical ABI lowering and lifting for WIT values.
//!
//! This module implements the canonical ABI memory layout for lowering values
//! to binary and lifting binary data back to values.

use thiserror::Error;
use wasm_wave::value::{Type as WaveType, Value};
use wasm_wave::wasm::{WasmType, WasmValue};
use wit_parser::{FlagsRepr, Int, Resolve, SizeAlign, Type, TypeDefKind};

/// Align a value up to the nearest multiple of alignment.
fn align_to(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

#[derive(Error, Debug)]
pub enum CanonicalAbiError {
    #[error("Buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall { needed: usize, available: usize },

    #[error("Invalid UTF-8 in string")]
    InvalidUtf8,

    #[error("Invalid discriminant {discriminant} for variant with {num_cases} cases")]
    InvalidDiscriminant { discriminant: u32, num_cases: usize },

    #[error("Invalid bool value: {0}")]
    InvalidBool(u8),

    #[error("Invalid char value: {0}")]
    InvalidChar(u32),

    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },

    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Linear memory required for variable-length type: {0}")]
    LinearMemoryRequired(String),

    #[error("Invalid memory pointer: {ptr} with length {len} exceeds memory size {memory_size}")]
    InvalidMemoryPointer {
        ptr: u32,
        len: u32,
        memory_size: usize,
    },
}

/// Simulated linear memory for variable-length types (strings and lists).
///
/// This struct provides a simple bump allocator for allocating data that
/// would normally live in WebAssembly linear memory.
#[derive(Default, Clone)]
pub struct LinearMemory {
    data: Vec<u8>,
}

impl LinearMemory {
    /// Create a new empty linear memory.
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Create a linear memory from existing bytes.
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Allocate space in linear memory and return the pointer (offset).
    /// Aligns the allocation to the specified alignment.
    pub fn alloc(&mut self, size: usize, align: usize) -> u32 {
        let current_len = self.data.len();
        let aligned_offset = align_to(current_len, align);

        // Add padding if needed
        if aligned_offset > current_len {
            self.data.resize(aligned_offset, 0);
        }

        // Allocate the space
        let ptr = self.data.len() as u32;
        self.data.resize(self.data.len() + size, 0);
        ptr
    }

    /// Write bytes at a specific offset in memory.
    pub fn write(&mut self, offset: u32, bytes: &[u8]) {
        let start = offset as usize;
        let end = start + bytes.len();
        if end > self.data.len() {
            self.data.resize(end, 0);
        }
        self.data[start..end].copy_from_slice(bytes);
    }

    /// Read bytes from a specific offset in memory.
    pub fn read(&self, offset: u32, len: u32) -> Result<&[u8], CanonicalAbiError> {
        let start = offset as usize;
        let end = start + len as usize;
        if end > self.data.len() {
            return Err(CanonicalAbiError::InvalidMemoryPointer {
                ptr: offset,
                len,
                memory_size: self.data.len(),
            });
        }
        Ok(&self.data[start..end])
    }

    /// Get the raw bytes of the linear memory.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Consume the linear memory and return the raw bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Check if the memory is empty (no allocations made).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Canonical ABI implementation for lowering and lifting values.
pub struct CanonicalAbi<'a> {
    resolve: &'a Resolve,
    sizes: SizeAlign,
}

impl<'a> CanonicalAbi<'a> {
    pub fn new(resolve: &'a Resolve) -> Self {
        let mut sizes = SizeAlign::default();
        sizes.fill(resolve);
        Self { resolve, sizes }
    }

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
    fn lower_into(
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
                buffer[offset] = if v { 1 } else { 0 };
            }
            Type::U8 => {
                buffer[offset] = value.unwrap_u8();
            }
            Type::S8 => {
                buffer[offset] = value.unwrap_s8() as u8;
            }
            Type::U16 => {
                let aligned = align_to(offset, 2);
                let bytes = value.unwrap_u16().to_le_bytes();
                buffer[aligned..aligned + 2].copy_from_slice(&bytes);
            }
            Type::S16 => {
                let aligned = align_to(offset, 2);
                let bytes = value.unwrap_s16().to_le_bytes();
                buffer[aligned..aligned + 2].copy_from_slice(&bytes);
            }
            Type::U32 => {
                let aligned = align_to(offset, 4);
                let bytes = value.unwrap_u32().to_le_bytes();
                buffer[aligned..aligned + 4].copy_from_slice(&bytes);
            }
            Type::S32 => {
                let aligned = align_to(offset, 4);
                let bytes = value.unwrap_s32().to_le_bytes();
                buffer[aligned..aligned + 4].copy_from_slice(&bytes);
            }
            Type::U64 => {
                let aligned = align_to(offset, 8);
                let bytes = value.unwrap_u64().to_le_bytes();
                buffer[aligned..aligned + 8].copy_from_slice(&bytes);
            }
            Type::S64 => {
                let aligned = align_to(offset, 8);
                let bytes = value.unwrap_s64().to_le_bytes();
                buffer[aligned..aligned + 8].copy_from_slice(&bytes);
            }
            Type::F32 => {
                let aligned = align_to(offset, 4);
                let bytes = value.unwrap_f32().to_le_bytes();
                buffer[aligned..aligned + 4].copy_from_slice(&bytes);
            }
            Type::F64 => {
                let aligned = align_to(offset, 8);
                let bytes = value.unwrap_f64().to_le_bytes();
                buffer[aligned..aligned + 8].copy_from_slice(&bytes);
            }
            Type::Char => {
                let aligned = align_to(offset, 4);
                let bytes = (value.unwrap_char() as u32).to_le_bytes();
                buffer[aligned..aligned + 4].copy_from_slice(&bytes);
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
                        buffer[aligned..aligned + 4].copy_from_slice(&ptr_bytes);
                        buffer[aligned + 4..aligned + 8].copy_from_slice(&len_bytes);
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
        let ty_def = &self.resolve.types[id];
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
                    let (field_off, _) = &field_offsets[i];
                    let (_, wave_field_ty) = &wave_fields[i];
                    let (_, field_val) = &field_values[i];
                    self.lower_into(
                        &field_val.clone(),
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
                    let (field_off, _) = &field_offsets[i];
                    let wave_elem_ty = &wave_types[i];
                    let elem = &elem_values[i];
                    self.lower_into(
                        &elem.clone(),
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
                        buffer[offset] = flags_value as u8;
                    }
                    FlagsRepr::U16 => {
                        let aligned = align_to(offset, 2);
                        buffer[aligned..aligned + 2]
                            .copy_from_slice(&(flags_value as u16).to_le_bytes());
                    }
                    FlagsRepr::U32(n) => {
                        let aligned = align_to(offset, 4);
                        // For multi-word flags, this is simplified
                        for i in 0..n {
                            let word = if i == 0 { flags_value } else { 0 };
                            let word_offset = aligned + (i * 4);
                            buffer[word_offset..word_offset + 4]
                                .copy_from_slice(&word.to_le_bytes());
                        }
                    }
                }
            }
            TypeDefKind::Enum(e) => {
                let case_name = value.unwrap_enum();
                let case_idx = e
                    .cases
                    .iter()
                    .position(|c| c.name == *case_name)
                    .ok_or(CanonicalAbiError::InvalidDiscriminant {
                        discriminant: 0,
                        num_cases: e.cases.len(),
                    })?;

                self.write_discriminant(buffer, offset, e.tag(), case_idx as u32)?;
            }
            TypeDefKind::Variant(v) => {
                let (case_name, payload) = value.unwrap_variant();
                let case_idx = v
                    .cases
                    .iter()
                    .position(|c| c.name == *case_name)
                    .ok_or(CanonicalAbiError::InvalidDiscriminant {
                        discriminant: 0,
                        num_cases: v.cases.len(),
                    })?;

                self.write_discriminant(buffer, offset, v.tag(), case_idx as u32)?;

                if let Some(payload_value) = payload {
                    let payload_offset = self
                        .sizes
                        .payload_offset(v.tag(), v.cases.iter().map(|c| c.ty.as_ref()));
                    let case = &v.cases[case_idx];
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
                        buffer[offset] = 1;
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
                        buffer[offset] = 0;
                    }
                }
            }
            TypeDefKind::Result(r) => {
                let result_value = value.unwrap_result();
                let (ok_ty, err_ty) = wave_ty.result_types().ok_or_else(|| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "result".to_string(),
                        got: "non-result".to_string(),
                    }
                })?;

                let payload_offset = self
                    .sizes
                    .payload_offset(Int::U8, [r.ok.as_ref(), r.err.as_ref()]);

                match result_value {
                    Ok(ok_val) => {
                        buffer[offset] = 0;
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
                        buffer[offset] = 1;
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
                        buffer[aligned..aligned + 4].copy_from_slice(&ptr_bytes);
                        buffer[aligned + 4..aligned + 8].copy_from_slice(&len_bytes);
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
                let wave_elem_ty = wave_ty.list_element_type().ok_or_else(|| {
                    CanonicalAbiError::TypeMismatch {
                        expected: "list".to_string(),
                        got: "non-list".to_string(),
                    }
                })?;

                let elem_values: Vec<_> = value.unwrap_list().collect();
                for i in 0..*len as usize {
                    if i >= elem_values.len() {
                        break;
                    }
                    self.lower_into(
                        &elem_values[i].clone(),
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

    fn write_discriminant(
        &self,
        buffer: &mut [u8],
        offset: usize,
        tag: Int,
        value: u32,
    ) -> Result<(), CanonicalAbiError> {
        match tag {
            Int::U8 => {
                buffer[offset] = value as u8;
            }
            Int::U16 => {
                let aligned = align_to(offset, 2);
                buffer[aligned..aligned + 2].copy_from_slice(&(value as u16).to_le_bytes());
            }
            Int::U32 => {
                let aligned = align_to(offset, 4);
                buffer[aligned..aligned + 4].copy_from_slice(&value.to_le_bytes());
            }
            Int::U64 => {
                let aligned = align_to(offset, 8);
                buffer[aligned..aligned + 8].copy_from_slice(&(value as u64).to_le_bytes());
            }
        }
        Ok(())
    }

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
                let v = buffer[offset];
                match v {
                    0 => Value::make_bool(false),
                    1 => Value::make_bool(true),
                    _ => return Err(CanonicalAbiError::InvalidBool(v)),
                }
            }
            Type::U8 => Value::make_u8(buffer[offset]),
            Type::S8 => Value::make_s8(buffer[offset] as i8),
            Type::U16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = buffer[aligned..aligned + 2]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    })?;
                Value::make_u16(u16::from_le_bytes(bytes))
            }
            Type::S16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = buffer[aligned..aligned + 2]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    })?;
                Value::make_s16(i16::from_le_bytes(bytes))
            }
            Type::U32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = buffer[aligned..aligned + 4]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                Value::make_u32(u32::from_le_bytes(bytes))
            }
            Type::S32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = buffer[aligned..aligned + 4]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                Value::make_s32(i32::from_le_bytes(bytes))
            }
            Type::U64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = buffer[aligned..aligned + 8]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;
                Value::make_u64(u64::from_le_bytes(bytes))
            }
            Type::S64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = buffer[aligned..aligned + 8]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;
                Value::make_s64(i64::from_le_bytes(bytes))
            }
            Type::F32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = buffer[aligned..aligned + 4]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                Value::make_f32(f32::from_le_bytes(bytes))
            }
            Type::F64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = buffer[aligned..aligned + 8]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;
                Value::make_f64(f64::from_le_bytes(bytes))
            }
            Type::Char => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = buffer[aligned..aligned + 4]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
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
                        let ptr_bytes: [u8; 4] = buffer[aligned..aligned + 4]
                            .try_into()
                            .map_err(|_| CanonicalAbiError::BufferTooSmall {
                                needed: aligned + 4,
                                available: buffer.len(),
                            })?;
                        let len_bytes: [u8; 4] = buffer[aligned + 4..aligned + 8]
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
        let ty_def = &self.resolve.types[id];
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
                    let (field_off, _) = &field_offsets[i];
                    let (_, wave_field_ty) = &wave_fields[i];
                    let (field_val, _) = self.lift_from(
                        buffer,
                        &field_def.ty,
                        wave_field_ty,
                        offset + field_off,
                        memory,
                    )?;
                    fields.push((&field_def.name, field_val));
                }

                Value::make_record(wave_ty, fields).map_err(|e| CanonicalAbiError::TypeMismatch {
                    expected: "record".to_string(),
                    got: e.to_string(),
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
                    let (field_off, _) = &field_offsets[i];
                    let wave_elem_ty = &wave_types[i];
                    let (elem_val, _) =
                        self.lift_from(buffer, wit_ty, wave_elem_ty, offset + field_off, memory)?;
                    elements.push(elem_val);
                }

                Value::make_tuple(wave_ty, elements).map_err(|e| CanonicalAbiError::TypeMismatch {
                    expected: "tuple".to_string(),
                    got: e.to_string(),
                })?
            }
            TypeDefKind::Flags(f) => {
                let flag_names: Vec<_> = wave_ty.flags_names().collect();
                let flags_value = match f.repr() {
                    FlagsRepr::U8 => buffer[offset] as u32,
                    FlagsRepr::U16 => {
                        let aligned = align_to(offset, 2);
                        let bytes: [u8; 2] = buffer[aligned..aligned + 2]
                            .try_into()
                            .map_err(|_| CanonicalAbiError::BufferTooSmall {
                                needed: aligned + 2,
                                available: buffer.len(),
                            })?;
                        u16::from_le_bytes(bytes) as u32
                    }
                    FlagsRepr::U32(_) => {
                        let aligned = align_to(offset, 4);
                        let bytes: [u8; 4] = buffer[aligned..aligned + 4]
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
                if discriminant as usize >= e.cases.len() {
                    return Err(CanonicalAbiError::InvalidDiscriminant {
                        discriminant,
                        num_cases: e.cases.len(),
                    });
                }
                let case_name = &e.cases[discriminant as usize].name;

                Value::make_enum(wave_ty, case_name).map_err(|e| CanonicalAbiError::TypeMismatch {
                    expected: "enum".to_string(),
                    got: e.to_string(),
                })?
            }
            TypeDefKind::Variant(v) => {
                let discriminant = self.read_discriminant(buffer, offset, v.tag())?;
                if discriminant as usize >= v.cases.len() {
                    return Err(CanonicalAbiError::InvalidDiscriminant {
                        discriminant,
                        num_cases: v.cases.len(),
                    });
                }

                let case = &v.cases[discriminant as usize];
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
                let discriminant = buffer[offset];
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
                        })
                    }
                };

                Value::make_option(wave_ty, opt_val).map_err(|e| CanonicalAbiError::TypeMismatch {
                    expected: "option".to_string(),
                    got: e.to_string(),
                })?
            }
            TypeDefKind::Result(r) => {
                let discriminant = buffer[offset];
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
                        })
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
                        let ptr_bytes: [u8; 4] = buffer[aligned..aligned + 4]
                            .try_into()
                            .map_err(|_| CanonicalAbiError::BufferTooSmall {
                                needed: aligned + 4,
                                available: buffer.len(),
                            })?;
                        let len_bytes: [u8; 4] = buffer[aligned + 4..aligned + 8]
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

                Value::make_list(wave_ty, elements).map_err(|e| CanonicalAbiError::TypeMismatch {
                    expected: "list".to_string(),
                    got: e.to_string(),
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

    fn read_discriminant(
        &self,
        buffer: &[u8],
        offset: usize,
        tag: Int,
    ) -> Result<u32, CanonicalAbiError> {
        match tag {
            Int::U8 => Ok(buffer[offset] as u32),
            Int::U16 => {
                let aligned = align_to(offset, 2);
                let bytes: [u8; 2] = buffer[aligned..aligned + 2]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 2,
                        available: buffer.len(),
                    })?;
                Ok(u16::from_le_bytes(bytes) as u32)
            }
            Int::U32 => {
                let aligned = align_to(offset, 4);
                let bytes: [u8; 4] = buffer[aligned..aligned + 4]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 4,
                        available: buffer.len(),
                    })?;
                Ok(u32::from_le_bytes(bytes))
            }
            Int::U64 => {
                let aligned = align_to(offset, 8);
                let bytes: [u8; 8] = buffer[aligned..aligned + 8]
                    .try_into()
                    .map_err(|_| CanonicalAbiError::BufferTooSmall {
                        needed: aligned + 8,
                        available: buffer.len(),
                    })?;
                Ok(u64::from_le_bytes(bytes) as u32)
            }
        }
    }
}
