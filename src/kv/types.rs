//! Data types for the KV store module.

/// Stored value envelope - wraps the actual value with metadata.
/// This structure mirrors the `stored-value` WIT type in kv.wit.
#[derive(Debug, Clone)]
pub struct StoredValue {
    /// Format version for future compatibility
    pub version: u8,

    /// Type version at time of storage (for schema migration detection)
    pub type_version: u32,

    /// Canonical ABI encoded value bytes
    pub value: Vec<u8>,

    /// Linear memory bytes (for variable-length types: strings, lists)
    pub memory: Option<Vec<u8>>,
}

impl StoredValue {
    /// Current format version
    pub const CURRENT_VERSION: u8 = 1;

    /// Create a new StoredValue with the current format version.
    pub fn new(type_version: u32, value: Vec<u8>, memory: Option<Vec<u8>>) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            type_version,
            value,
            memory,
        }
    }
}

/// Keyspace type metadata.
/// This structure mirrors the `keyspace-metadata` WIT type in kv.wit.
#[derive(Debug, Clone)]
pub struct KeyspaceMetadata {
    /// User-visible keyspace name
    pub name: String,

    /// Full WIT qualified name (namespace:package/interface#type)
    pub qualified_name: String,

    /// Full WIT type definition text
    pub wit_definition: String,

    /// Type name within the WIT definition
    pub type_name: String,

    /// Incremented on type definition changes
    pub type_version: u32,

    /// CRC32 hash of WIT definition
    pub type_hash: u32,

    /// Unix timestamp of creation
    pub created_at: u64,
}

impl KeyspaceMetadata {
    /// Create new keyspace metadata.
    pub fn new(
        name: String,
        qualified_name: String,
        wit_definition: String,
        type_name: String,
    ) -> Self {
        let type_hash = crc32fast::hash(wit_definition.as_bytes());
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            name,
            qualified_name,
            wit_definition,
            type_name,
            type_version: 1,
            type_hash,
            created_at,
        }
    }
}
