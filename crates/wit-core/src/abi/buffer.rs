//! Buffer read/write helpers for canonical ABI operations.

use super::CanonicalAbiError;

/// Align a value up to the nearest multiple of alignment.
#[inline]
pub fn align_to(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

/// Safe buffer read helper - returns error instead of panicking.
#[inline]
pub fn read_byte(buffer: &[u8], offset: usize) -> Result<u8, CanonicalAbiError> {
    buffer
        .get(offset)
        .copied()
        .ok_or(CanonicalAbiError::BufferTooSmall {
            needed: offset + 1,
            available: buffer.len(),
        })
}

/// Safe buffer write helper - returns error instead of panicking.
#[inline]
pub fn write_byte(buffer: &mut [u8], offset: usize, value: u8) -> Result<(), CanonicalAbiError> {
    let len = buffer.len();
    *buffer
        .get_mut(offset)
        .ok_or(CanonicalAbiError::BufferTooSmall {
            needed: offset + 1,
            available: len,
        })? = value;
    Ok(())
}

/// Safe buffer slice read helper.
#[inline]
pub fn read_slice(buffer: &[u8], start: usize, len: usize) -> Result<&[u8], CanonicalAbiError> {
    let buf_len = buffer.len();
    buffer
        .get(start..start + len)
        .ok_or(CanonicalAbiError::BufferTooSmall {
            needed: start + len,
            available: buf_len,
        })
}

/// Safe buffer slice write helper.
#[inline]
pub fn write_slice(buffer: &mut [u8], start: usize, data: &[u8]) -> Result<(), CanonicalAbiError> {
    let end = start + data.len();
    let len = buffer.len();
    buffer
        .get_mut(start..end)
        .ok_or(CanonicalAbiError::BufferTooSmall {
            needed: end,
            available: len,
        })?
        .copy_from_slice(data);
    Ok(())
}
