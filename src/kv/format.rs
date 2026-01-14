//! WIT-based binary format encoding and decoding.
//!
//! This module uses the canonical ABI to encode/decode StoredValue and KeyspaceMetadata
//! structures using the WIT types defined in kv.wit.

use std::borrow::Cow;

use once_cell::sync::Lazy;
use wasm_wave::value::{resolve_wit_type, Type as WaveType, Value};
use wasm_wave::wasm::{WasmType, WasmValue};
use wit_parser::{Resolve, Type, TypeId};

use crate::{find_type_by_name, CanonicalAbi, LinearMemory};

use super::error::KvError;
use super::types::{KeyspaceMetadata, StoredValue};
use super::version::SemanticVersion;

/// Lazily loaded KV WIT types.
struct KvTypes {
    resolve: Resolve,
    stored_value_id: TypeId,
    keyspace_metadata_id: TypeId,
    binary_export_id: TypeId,
    stored_value_wave_type: WaveType,
    keyspace_metadata_wave_type: WaveType,
    binary_export_wave_type: WaveType,
}

static KV_TYPES: Lazy<KvTypes> = Lazy::new(|| {
    // This expect is acceptable because kv.wit is a compile-time constant embedded in the binary.
    // If it fails to parse, it's a bug in the embedded WIT file, not a runtime error.
    #[allow(clippy::expect_used)]
    load_kv_types().expect("Failed to load kv.wit types - this is a bug in the embedded WIT file")
});

fn load_kv_types() -> Result<KvTypes, KvError> {
    let mut resolve = Resolve::new();

    // Load the kv.wit file from the crate root
    let kv_wit = include_str!("../../kv.wit");
    resolve.push_str("kv.wit", kv_wit)?;

    // Find the types
    let stored_value_id = find_type_by_name(&resolve, "stored-value")
        .ok_or_else(|| KvError::TypeNotFound("stored-value".to_string()))?;

    let keyspace_metadata_id = find_type_by_name(&resolve, "keyspace-metadata")
        .ok_or_else(|| KvError::TypeNotFound("keyspace-metadata".to_string()))?;

    let binary_export_id = find_type_by_name(&resolve, "binary-export")
        .ok_or_else(|| KvError::TypeNotFound("binary-export".to_string()))?;

    let stored_value_wave_type = resolve_wit_type(&resolve, stored_value_id)
        .map_err(|e| KvError::WaveParse(e.to_string()))?;

    let keyspace_metadata_wave_type = resolve_wit_type(&resolve, keyspace_metadata_id)
        .map_err(|e| KvError::WaveParse(e.to_string()))?;

    let binary_export_wave_type = resolve_wit_type(&resolve, binary_export_id)
        .map_err(|e| KvError::WaveParse(e.to_string()))?;

    Ok(KvTypes {
        resolve,
        stored_value_id,
        keyspace_metadata_id,
        binary_export_id,
        stored_value_wave_type,
        keyspace_metadata_wave_type,
        binary_export_wave_type,
    })
}

/// Helper to get a record field type by name
fn get_field_type(wave_type: &WaveType, field_name: &str) -> Option<WaveType> {
    wave_type
        .record_fields()
        .find(|(name, _)| name.as_ref() == field_name)
        .map(|(_, ty)| ty)
}

/// Type alias for record fields extracted from a Value
type RecordFields<'a> = Vec<(Cow<'a, str>, Cow<'a, Value>)>;

/// Helper to get a field value from extracted record fields
fn get_field<'a>(fields: &'a RecordFields<'_>, name: &str) -> Result<&'a Value, KvError> {
    fields
        .iter()
        .find(|(n, _)| n.as_ref() == name)
        .map(|(_, v)| v.as_ref())
        .ok_or_else(|| KvError::InvalidFormat(format!("Missing {} field", name)))
}

/// Helper to create a semantic-version WAVE value
fn make_semantic_version(version: &SemanticVersion, parent_type: &WaveType) -> Result<Value, KvError> {
    let version_type = get_field_type(parent_type, "type-version")
        .ok_or_else(|| KvError::InvalidFormat("Missing type-version field type".to_string()))?;

    Value::make_record(
        &version_type,
        vec![
            ("major", Value::make_u32(version.major)),
            ("minor", Value::make_u32(version.minor)),
            ("patch", Value::make_u32(version.patch)),
        ],
    )
    .map_err(|e| KvError::WaveParse(e.to_string()))
}

/// Helper to extract a SemanticVersion from a WAVE record value
fn extract_semantic_version(value: &Value) -> Result<SemanticVersion, KvError> {
    let fields: RecordFields<'_> = value.unwrap_record().collect();

    let major = get_field(&fields, "major")?.unwrap_u32();
    let minor = get_field(&fields, "minor")?.unwrap_u32();
    let patch = get_field(&fields, "patch")?.unwrap_u32();

    Ok(SemanticVersion::new(major, minor, patch))
}

impl StoredValue {
    /// Encode the StoredValue to binary using canonical ABI.
    pub fn encode(&self) -> Result<(Vec<u8>, Vec<u8>), KvError> {
        let kv = &*KV_TYPES;
        let abi = CanonicalAbi::new(&kv.resolve);

        // Build WAVE value for stored-value record
        let wave_value = self.to_wave_value(&kv.stored_value_wave_type)?;

        let mut memory = LinearMemory::new();
        let buffer = abi.lower_with_memory(
            &wave_value,
            &Type::Id(kv.stored_value_id),
            &kv.stored_value_wave_type,
            &mut memory,
        )?;

        Ok((buffer, memory.into_bytes()))
    }

    /// Decode a StoredValue from binary using canonical ABI.
    pub fn decode(buffer: &[u8], memory: &[u8]) -> Result<Self, KvError> {
        let kv = &*KV_TYPES;
        let abi = CanonicalAbi::new(&kv.resolve);
        let mem = LinearMemory::from_slice(memory);

        let (value, _) = abi.lift_with_memory(
            buffer,
            &Type::Id(kv.stored_value_id),
            &kv.stored_value_wave_type,
            &mem,
        )?;

        Self::from_wave_value(&value)
    }

    fn to_wave_value(&self, wave_type: &WaveType) -> Result<Value, KvError> {
        // Get field types from the record type
        let value_field_type = get_field_type(wave_type, "value")
            .ok_or_else(|| KvError::InvalidFormat("Missing value field type".to_string()))?;

        let memory_field_type = get_field_type(wave_type, "memory")
            .ok_or_else(|| KvError::InvalidFormat("Missing memory field type".to_string()))?;

        // Build the semantic-version record
        let type_version_val = make_semantic_version(&self.type_version, wave_type)?;

        // Build the list<u8> for value field (pass iterator directly to avoid intermediate Vec)
        let value_val =
            Value::make_list(&value_field_type, self.value.iter().map(|&b| Value::make_u8(b)))
                .map_err(|e| KvError::WaveParse(e.to_string()))?;

        // Build the option<list<u8>> for memory field
        let memory_val = match &self.memory {
            Some(mem) => {
                // Get the inner list type from option<list<u8>>
                let inner_list_type = memory_field_type.option_some_type().ok_or_else(|| {
                    KvError::InvalidFormat("Expected option type for memory".to_string())
                })?;

                let mem_list_val =
                    Value::make_list(&inner_list_type, mem.iter().map(|&b| Value::make_u8(b)))
                        .map_err(|e| KvError::WaveParse(e.to_string()))?;

                Value::make_option(&memory_field_type, Some(mem_list_val))
                    .map_err(|e| KvError::WaveParse(e.to_string()))?
            }
            None => Value::make_option(&memory_field_type, None)
                .map_err(|e| KvError::WaveParse(e.to_string()))?,
        };

        // Build the record
        Value::make_record(
            wave_type,
            vec![
                ("version", Value::make_u8(self.version)),
                ("type-version", type_version_val),
                ("value", value_val),
                ("memory", memory_val),
            ],
        )
        .map_err(|e| KvError::WaveParse(e.to_string()))
    }

    fn from_wave_value(value: &Value) -> Result<Self, KvError> {
        let fields: RecordFields<'_> = value.unwrap_record().collect();

        let version = get_field(&fields, "version")?.unwrap_u8();
        let type_version = extract_semantic_version(get_field(&fields, "type-version")?)?;
        let value_bytes: Vec<u8> = get_field(&fields, "value")?
            .unwrap_list()
            .map(|e| e.unwrap_u8())
            .collect();
        let memory = get_field(&fields, "memory")?
            .unwrap_option()
            .map(|inner| inner.unwrap_list().map(|e| e.unwrap_u8()).collect());

        Ok(StoredValue {
            version,
            type_version,
            value: value_bytes,
            memory,
        })
    }
}

impl KeyspaceMetadata {
    /// Encode the KeyspaceMetadata to binary using canonical ABI.
    pub fn encode(&self) -> Result<(Vec<u8>, Vec<u8>), KvError> {
        let kv = &*KV_TYPES;
        let abi = CanonicalAbi::new(&kv.resolve);

        let wave_value = self.to_wave_value(&kv.keyspace_metadata_wave_type)?;

        let mut memory = LinearMemory::new();
        let buffer = abi.lower_with_memory(
            &wave_value,
            &Type::Id(kv.keyspace_metadata_id),
            &kv.keyspace_metadata_wave_type,
            &mut memory,
        )?;

        Ok((buffer, memory.into_bytes()))
    }

    /// Decode a KeyspaceMetadata from binary using canonical ABI.
    pub fn decode(buffer: &[u8], memory: &[u8]) -> Result<Self, KvError> {
        let kv = &*KV_TYPES;
        let abi = CanonicalAbi::new(&kv.resolve);
        let mem = LinearMemory::from_slice(memory);

        let (value, _) = abi.lift_with_memory(
            buffer,
            &Type::Id(kv.keyspace_metadata_id),
            &kv.keyspace_metadata_wave_type,
            &mem,
        )?;

        Self::from_wave_value(&value)
    }

    fn to_wave_value(&self, wave_type: &WaveType) -> Result<Value, KvError> {
        // Build the semantic-version record
        let type_version_val = make_semantic_version(&self.type_version, wave_type)?;

        Value::make_record(
            wave_type,
            vec![
                ("name", Value::make_string(Cow::Borrowed(&self.name))),
                (
                    "qualified-name",
                    Value::make_string(Cow::Borrowed(&self.qualified_name)),
                ),
                (
                    "wit-definition",
                    Value::make_string(Cow::Borrowed(&self.wit_definition)),
                ),
                (
                    "type-name",
                    Value::make_string(Cow::Borrowed(&self.type_name)),
                ),
                ("type-version", type_version_val),
                ("type-hash", Value::make_u32(self.type_hash)),
                ("created-at", Value::make_u64(self.created_at)),
            ],
        )
        .map_err(|e| KvError::WaveParse(e.to_string()))
    }

    fn from_wave_value(value: &Value) -> Result<Self, KvError> {
        let fields: RecordFields<'_> = value.unwrap_record().collect();

        let name = get_field(&fields, "name")?.unwrap_string().to_string();
        let qualified_name = get_field(&fields, "qualified-name")?.unwrap_string().to_string();
        let wit_definition = get_field(&fields, "wit-definition")?.unwrap_string().to_string();
        let type_name = get_field(&fields, "type-name")?.unwrap_string().to_string();
        let type_version = extract_semantic_version(get_field(&fields, "type-version")?)?;
        let type_hash = get_field(&fields, "type-hash")?.unwrap_u32();
        let created_at = get_field(&fields, "created-at")?.unwrap_u64();

        Ok(KeyspaceMetadata {
            name,
            qualified_name,
            wit_definition,
            type_name,
            type_version,
            type_hash,
            created_at,
        })
    }
}

/// Binary export wrapper for transferring complete values with memory.
/// This mirrors the `binary-export` WIT type in kv.wit.
#[derive(Debug, Clone)]
pub struct BinaryExport {
    /// Canonical ABI encoded value buffer
    pub buffer: Vec<u8>,
    /// Linear memory bytes (for variable-length types)
    pub memory: Option<Vec<u8>>,
}

impl BinaryExport {
    /// Flat buffer size for the binary-export record in canonical ABI.
    /// - value: list<u8> = 8 bytes (ptr + len)
    /// - memory: option<list<u8>> = 12 bytes (discriminant + padding + ptr + len)
    ///
    /// Total: 20 bytes
    pub const FLAT_SIZE: usize = 20;

    /// Create a BinaryExport from a StoredValue reference (clones data).
    pub fn from_stored(stored: &StoredValue) -> Self {
        BinaryExport {
            buffer: stored.value.clone(),
            memory: stored.memory.clone(),
        }
    }

    /// Create a BinaryExport by consuming a StoredValue (no clone).
    pub fn from_stored_owned(stored: StoredValue) -> Self {
        BinaryExport {
            buffer: stored.value,
            memory: stored.memory,
        }
    }

    /// Encode the BinaryExport to binary using canonical ABI.
    pub fn encode(&self) -> Result<(Vec<u8>, Vec<u8>), KvError> {
        let kv = &*KV_TYPES;
        let abi = CanonicalAbi::new(&kv.resolve);

        let wave_value = self.to_wave_value(&kv.binary_export_wave_type)?;

        let mut memory = LinearMemory::new();
        let buffer = abi.lower_with_memory(
            &wave_value,
            &Type::Id(kv.binary_export_id),
            &kv.binary_export_wave_type,
            &mut memory,
        )?;

        Ok((buffer, memory.into_bytes()))
    }

    /// Decode a BinaryExport from a single byte slice.
    /// The first FLAT_SIZE bytes are the flat buffer, the rest is linear memory.
    pub fn decode_from_bytes(data: &[u8]) -> Result<Self, KvError> {
        if data.len() < Self::FLAT_SIZE {
            return Err(KvError::InvalidFormat(format!(
                "Binary export data too small: {} bytes, need at least {}",
                data.len(),
                Self::FLAT_SIZE
            )));
        }
        let (buffer, memory) = data.split_at(Self::FLAT_SIZE);
        Self::decode(buffer, memory)
    }

    /// Decode a BinaryExport from separate buffer and memory slices.
    pub fn decode(buffer: &[u8], memory: &[u8]) -> Result<Self, KvError> {
        let kv = &*KV_TYPES;
        let abi = CanonicalAbi::new(&kv.resolve);
        let mem = LinearMemory::from_slice(memory);

        let (value, _) = abi.lift_with_memory(
            buffer,
            &Type::Id(kv.binary_export_id),
            &kv.binary_export_wave_type,
            &mem,
        )?;

        Self::from_wave_value(&value)
    }

    fn to_wave_value(&self, wave_type: &WaveType) -> Result<Value, KvError> {
        let buffer_field_type = get_field_type(wave_type, "value")
            .ok_or_else(|| KvError::InvalidFormat("Missing value field type".to_string()))?;

        let memory_field_type = get_field_type(wave_type, "memory")
            .ok_or_else(|| KvError::InvalidFormat("Missing memory field type".to_string()))?;

        // Build the list<u8> for buffer field (pass iterator directly to avoid intermediate Vec)
        let buffer_val =
            Value::make_list(&buffer_field_type, self.buffer.iter().map(|&b| Value::make_u8(b)))
                .map_err(|e| KvError::WaveParse(e.to_string()))?;

        // Build the option<list<u8>> for memory field
        let memory_val = match &self.memory {
            Some(mem) => {
                let inner_list_type = memory_field_type
                    .option_some_type()
                    .ok_or_else(|| KvError::InvalidFormat("Expected option type for memory".to_string()))?;

                let mem_list_val =
                    Value::make_list(&inner_list_type, mem.iter().map(|&b| Value::make_u8(b)))
                        .map_err(|e| KvError::WaveParse(e.to_string()))?;

                Value::make_option(&memory_field_type, Some(mem_list_val))
                    .map_err(|e| KvError::WaveParse(e.to_string()))?
            }
            None => {
                Value::make_option(&memory_field_type, None)
                    .map_err(|e| KvError::WaveParse(e.to_string()))?
            }
        };

        Value::make_record(wave_type, vec![("value", buffer_val), ("memory", memory_val)])
            .map_err(|e| KvError::WaveParse(e.to_string()))
    }

    fn from_wave_value(value: &Value) -> Result<Self, KvError> {
        let fields: RecordFields<'_> = value.unwrap_record().collect();

        let buffer: Vec<u8> = get_field(&fields, "value")?
            .unwrap_list()
            .map(|e| e.unwrap_u8())
            .collect();
        let memory = get_field(&fields, "memory")?
            .unwrap_option()
            .map(|inner| inner.unwrap_list().map(|e| e.unwrap_u8()).collect());

        Ok(BinaryExport { buffer, memory })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stored_value_roundtrip() {
        let original = StoredValue::new(
            SemanticVersion::new(0, 1, 0),
            vec![1, 2, 3, 4],
            Some(vec![5, 6, 7]),
        );
        let (buffer, memory) = original.encode().unwrap();
        let decoded = StoredValue::decode(&buffer, &memory).unwrap();

        assert_eq!(original.version, decoded.version);
        assert_eq!(original.type_version, decoded.type_version);
        assert_eq!(original.value, decoded.value);
        assert_eq!(original.memory, decoded.memory);
    }

    #[test]
    fn test_stored_value_without_memory() {
        let original = StoredValue::new(SemanticVersion::new(1, 2, 3), vec![1, 2, 3, 4], None);
        let (buffer, memory) = original.encode().unwrap();
        let decoded = StoredValue::decode(&buffer, &memory).unwrap();

        assert_eq!(original.version, decoded.version);
        assert_eq!(original.type_version, decoded.type_version);
        assert_eq!(original.value, decoded.value);
        assert_eq!(original.memory, decoded.memory);
    }

    #[test]
    fn test_keyspace_metadata_roundtrip() {
        let original = KeyspaceMetadata::new(
            "task".to_string(),
            "test:types/types#task".to_string(),
            "record task { name: string }".to_string(),
            "task".to_string(),
        );

        let (buffer, memory) = original.encode().unwrap();
        let decoded = KeyspaceMetadata::decode(&buffer, &memory).unwrap();

        assert_eq!(original.name, decoded.name);
        assert_eq!(original.qualified_name, decoded.qualified_name);
        assert_eq!(original.wit_definition, decoded.wit_definition);
        assert_eq!(original.type_name, decoded.type_name);
        assert_eq!(original.type_version, decoded.type_version);
        assert_eq!(original.type_hash, decoded.type_hash);
    }

    #[test]
    fn test_binary_export_roundtrip() {
        let original = BinaryExport {
            buffer: vec![1, 2, 3, 4],
            memory: Some(vec![5, 6, 7, 8]),
        };

        let (buffer, memory) = original.encode().unwrap();
        let decoded = BinaryExport::decode(&buffer, &memory).unwrap();

        assert_eq!(original.buffer, decoded.buffer);
        assert_eq!(original.memory, decoded.memory);
    }

    #[test]
    fn test_binary_export_without_memory() {
        let original = BinaryExport {
            buffer: vec![10, 20, 30],
            memory: None,
        };

        let (buffer, memory) = original.encode().unwrap();
        let decoded = BinaryExport::decode(&buffer, &memory).unwrap();

        assert_eq!(original.buffer, decoded.buffer);
        assert_eq!(original.memory, decoded.memory);
    }

    #[test]
    fn test_binary_export_from_stored() {
        let stored = StoredValue::new(
            SemanticVersion::INITIAL,
            vec![1, 2, 3],
            Some(vec![4, 5, 6]),
        );
        let export = BinaryExport::from_stored(&stored);

        assert_eq!(export.buffer, stored.value);
        assert_eq!(export.memory, stored.memory);
    }
}
