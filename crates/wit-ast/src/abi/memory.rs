//! Simulated linear memory for variable-length types.

use super::buffer::align_to;
use super::CanonicalAbiError;

/// Simulated linear memory for variable-length types (strings and lists).
#[derive(Default, Clone, Debug, PartialEq, Eq)]
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

    /// Create a linear memory from an optional reference to bytes (clones if Some).
    pub fn from_optional(data: Option<&Vec<u8>>) -> Self {
        match data {
            Some(d) => Self { data: d.clone() },
            None => Self::new(),
        }
    }

    /// Create a linear memory from an optional owned bytes (no clone).
    pub fn from_option(data: Option<Vec<u8>>) -> Self {
        match data {
            Some(d) => Self { data: d },
            None => Self::new(),
        }
    }

    /// Create a linear memory from a slice (clones the data).
    pub fn from_slice(data: &[u8]) -> Self {
        Self { data: data.to_vec() }
    }

    /// Allocate space in linear memory and return the pointer (offset).
    pub fn alloc(&mut self, size: usize, align: usize) -> u32 {
        let current_len = self.data.len();
        let aligned_offset = align_to(current_len, align);

        if aligned_offset > current_len {
            self.data.resize(aligned_offset, 0);
        }

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
        if let Some(slice) = self.data.get_mut(start..end) {
            slice.copy_from_slice(bytes);
        }
    }

    /// Read bytes from a specific offset in memory.
    pub fn read(&self, offset: u32, len: u32) -> Result<&[u8], CanonicalAbiError> {
        let start = offset as usize;
        let end = start + len as usize;
        self.data
            .get(start..end)
            .ok_or(CanonicalAbiError::InvalidMemoryPointer {
                ptr: offset,
                len,
                memory_size: self.data.len(),
            })
    }

    /// Get the raw bytes of the linear memory.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Consume the linear memory and return the raw bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Check if the memory is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the length of the memory in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl From<Vec<u8>> for LinearMemory {
    fn from(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl From<&[u8]> for LinearMemory {
    fn from(data: &[u8]) -> Self {
        Self { data: data.to_vec() }
    }
}

impl AsRef<[u8]> for LinearMemory {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl std::ops::Deref for LinearMemory {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
