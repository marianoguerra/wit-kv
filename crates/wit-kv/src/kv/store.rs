//! KV Store implementation using fjall.

use std::path::Path;

use fjall::{Keyspace, KeyspaceCreateOptions, PersistMode};
use wasm_wave::value::Value;
use wit_parser::{Resolve, Type, TypeId};

use crate::logging::{debug, error, info, trace, warn};
use crate::{find_first_named_type, find_type_by_name, CanonicalAbi, LinearMemory};
use wit_kv_abi::val_to_wave;

use super::error::KvError;
use super::types::{KeyspaceMetadata, StoredValue};

/// Key prefixes for the metadata keyspace.
const META_TYPES_PREFIX: &str = "types/";
const META_QUALIFIED_PREFIX: &str = "qualified/";
const META_CONFIG_KEY: &str = "config";

/// Data keyspace prefix.
const DATA_PREFIX: &str = "data_";

/// Current store version (1).
/// Increment this when changing the on-disk layout or metadata format.
/// The store will reject opening databases with a different version.
const STORE_VERSION: u32 = 1;

/// A typed key-value store backed by fjall.
///
/// `KvStore` provides persistent storage for WIT values, where each keyspace
/// is associated with a specific WIT type. Values are automatically encoded
/// using the canonical ABI format and type-checked on read/write.
///
/// # Example
///
/// ```ignore
/// use wit_kv::KvStore;
///
/// // Initialize a new store
/// let store = KvStore::init(".wit-kv")?;
///
/// // Register a type for a keyspace
/// // Given a types.wit file: record point { x: s32, y: s32 }
/// store.set_type("points", "types.wit", Some("point"), false)?;
///
/// // Store values using WAVE text format
/// store.set("points", "origin", "{x: 0, y: 0}")?;
/// store.set("points", "p1", "{x: 10, y: 20}")?;
///
/// // Retrieve values
/// if let Some(value) = store.get("points", "origin")? {
///     println!("Origin: {}", value); // "{x: 0, y: 0}"
/// }
///
/// // List keys
/// let keys = store.list("points", None, None, None, Some(10))?;
/// for key in keys {
///     println!("Key: {}", key);
/// }
///
/// // Delete a value
/// store.delete("points", "p1")?;
/// ```
///
/// # Keyspaces
///
/// Each keyspace has an associated WIT type that defines the schema for values
/// stored in that keyspace. The type is registered using [`set_type`](Self::set_type)
/// and values are validated against this schema.
///
/// # Persistence
///
/// The store is backed by fjall, an LSM-tree based storage engine. All write
/// operations are durably persisted before returning.
pub struct KvStore {
    db: fjall::Database,
    meta: Keyspace,
}

impl KvStore {
    /// Open an existing KV store at the given path.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wit_kv::KvStore;
    ///
    /// // Works with &str, String, &Path, PathBuf
    /// let store = KvStore::open(".wit-kv")?;
    /// let store = KvStore::open(PathBuf::from(".wit-kv"))?;
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<Self, KvError> {
        let path = path.as_ref();
        debug!(path = %path.display(), "opening KV store");

        if !path.exists() {
            error!(path = %path.display(), "store path does not exist");
            return Err(KvError::NotInitialized(path.display().to_string()));
        }

        let db = fjall::Database::builder(path).open()?;
        let meta = db.keyspace("_meta", KeyspaceCreateOptions::default)?;

        // Verify store version
        if let Some(config) = meta.get(META_CONFIG_KEY)? {
            let version = u32::from_le_bytes(
                config
                    .as_ref()
                    .try_into()
                    .map_err(|_| KvError::InvalidFormat("Invalid config format".to_string()))?,
            );
            if version != STORE_VERSION {
                error!(
                    stored_version = version,
                    expected_version = STORE_VERSION,
                    "store version mismatch"
                );
                return Err(KvError::InvalidFormat(format!(
                    "Store version mismatch: expected {}, got {}",
                    STORE_VERSION, version
                )));
            }
            trace!(version = version, "store version verified");
        } else {
            error!(path = %path.display(), "store not initialized - no config found");
            return Err(KvError::NotInitialized(path.display().to_string()));
        }

        info!(path = %path.display(), "KV store opened");
        Ok(Self { db, meta })
    }

    /// Initialize a new KV store at the given path.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wit_kv::KvStore;
    ///
    /// // Works with &str, String, &Path, PathBuf
    /// let store = KvStore::init(".wit-kv")?;
    /// let store = KvStore::init(PathBuf::from("./data/store"))?;
    /// ```
    pub fn init(path: impl AsRef<Path>) -> Result<Self, KvError> {
        let path = path.as_ref();
        debug!(path = %path.display(), "initializing KV store");

        let db = fjall::Database::builder(path).open()?;
        let meta = db.keyspace("_meta", KeyspaceCreateOptions::default)?;

        // Write store version
        meta.insert(META_CONFIG_KEY, STORE_VERSION.to_le_bytes())?;
        db.persist(PersistMode::SyncAll)?;

        info!(path = %path.display(), version = STORE_VERSION, "KV store initialized");
        Ok(Self { db, meta })
    }

    /// Register a type for a keyspace.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wit_kv::KvStore;
    ///
    /// let store = KvStore::init(".wit-kv")?;
    /// store.set_type("points", "types.wit", Some("point"), false)?;
    /// ```
    pub fn set_type(
        &self,
        keyspace: &str,
        wit_path: impl AsRef<Path>,
        type_name: Option<&str>,
        force: bool,
    ) -> Result<KeyspaceMetadata, KvError> {
        let wit_path = wit_path.as_ref();
        debug!(
            keyspace = keyspace,
            wit_path = %wit_path.display(),
            type_name = type_name,
            force = force,
            "registering type for keyspace"
        );

        // Check if keyspace already exists
        let key = format!("{}{}", META_TYPES_PREFIX, keyspace);
        if !force && self.meta.get(&key)?.is_some() {
            warn!(keyspace = keyspace, "keyspace already exists");
            return Err(KvError::KeyspaceExists(keyspace.to_string()));
        }

        // Parse the WIT file
        trace!(wit_path = %wit_path.display(), "parsing WIT file");
        let mut resolve = Resolve::new();
        resolve.push_path(wit_path)?;

        // Find the type
        let type_id = match type_name {
            Some(tn) => {
                find_type_by_name(&resolve, tn).ok_or_else(|| {
                    error!(type_name = tn, "type not found in WIT");
                    KvError::TypeNotFound(tn.to_string())
                })?
            }
            None => {
                find_first_named_type(&resolve).ok_or_else(|| {
                    error!("no named type found in WIT");
                    KvError::TypeNotFound("No named type found".to_string())
                })?
            }
        };

        let type_def = resolve
            .types
            .get(type_id)
            .ok_or_else(|| KvError::TypeNotFound(format!("Type {:?} not found", type_id)))?;
        let actual_type_name = type_def.name.clone().unwrap_or_default();

        // Build qualified name from package info
        let qualified_name = self.build_qualified_name(&resolve, type_id, &actual_type_name)?;
        trace!(
            type_name = %actual_type_name,
            qualified_name = %qualified_name,
            "resolved type"
        );

        // Read WIT file content
        let wit_definition = std::fs::read_to_string(wit_path)?;

        // Create metadata
        let metadata = KeyspaceMetadata::new(
            keyspace.to_string(),
            qualified_name.clone(),
            wit_definition,
            actual_type_name,
        );

        // Encode and store
        let (buffer, memory) = metadata.encode()?;
        self.store_with_memory(&self.meta, &key, &buffer, &memory)?;

        // Store reverse lookup
        let qualified_key = format!("{}{}", META_QUALIFIED_PREFIX, qualified_name);
        self.meta.insert(&qualified_key, keyspace.as_bytes())?;

        // Create data keyspace for this keyspace
        let data_keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let _ = self.db.keyspace(&data_keyspace_name, KeyspaceCreateOptions::default)?;

        self.db.persist(PersistMode::SyncAll)?;

        info!(
            keyspace = keyspace,
            qualified_name = %metadata.qualified_name,
            type_version = %format!("{}.{}.{}", metadata.type_version.major, metadata.type_version.minor, metadata.type_version.patch),
            "type registered"
        );
        Ok(metadata)
    }

    /// Get the type metadata for a keyspace.
    pub fn get_type(&self, keyspace: &str) -> Result<Option<KeyspaceMetadata>, KvError> {
        let key = format!("{}{}", META_TYPES_PREFIX, keyspace);
        self.load_metadata(&key)
    }

    /// Delete a keyspace type (and optionally its data).
    pub fn delete_type(&self, keyspace: &str, delete_data: bool) -> Result<(), KvError> {
        debug!(keyspace = keyspace, delete_data = delete_data, "deleting type");
        let key = format!("{}{}", META_TYPES_PREFIX, keyspace);

        // Get metadata to find qualified name
        if let Some(metadata) = self.load_metadata(&key)? {
            // Delete qualified name lookup
            let qualified_key = format!("{}{}", META_QUALIFIED_PREFIX, metadata.qualified_name);
            self.meta.remove(&qualified_key)?;
        }

        // Delete metadata
        let memory_key = format!("{}.memory", key);
        self.meta.remove(&key)?;
        self.meta.remove(&memory_key)?;

        // Delete data keyspace if requested
        if delete_data {
            let data_keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
            if let Ok(data_keyspace) = self.db.keyspace(&data_keyspace_name, KeyspaceCreateOptions::default) {
                // Clear all keys - skip any keys that fail to read
                let keys: Vec<Vec<u8>> = data_keyspace
                    .iter()
                    .filter_map(|kv| kv.key().ok().map(|k| k.to_vec()))
                    .collect();
                trace!(key_count = keys.len(), "deleting data keys");
                for k in keys {
                    data_keyspace.remove(&k)?;
                }
            }
        }

        self.db.persist(PersistMode::SyncAll)?;
        info!(keyspace = keyspace, delete_data = delete_data, "type deleted");
        Ok(())
    }

    /// List all registered types.
    pub fn list_types(&self) -> Result<Vec<KeyspaceMetadata>, KvError> {
        let mut types = Vec::new();

        for kv in self.meta.prefix(META_TYPES_PREFIX) {
            let Ok(key_bytes) = kv.key() else {
                continue;
            };
            let key_str = String::from_utf8_lossy(&key_bytes);

            // Skip memory keys
            if key_str.ends_with(".memory") {
                continue;
            }

            if let Some(metadata) = self.load_metadata(&key_str)? {
                types.push(metadata);
            }
        }

        Ok(types)
    }

    /// Set a value in a keyspace.
    pub fn set(&self, keyspace: &str, key: &str, wave_value: &str) -> Result<(), KvError> {
        debug!(keyspace = keyspace, key = key, value_len = wave_value.len(), "setting value");

        let metadata = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        // Parse WIT type from stored definition
        let (resolve, type_id, wave_type) = self.parse_stored_type(&metadata)?;
        trace!(type_name = %metadata.type_name, "parsed WIT type for encoding");

        // Parse the WAVE value
        let value: Value = wasm_wave::from_str(&wave_type, wave_value).map_err(|e| {
            error!(keyspace = keyspace, key = key, error = %e, "failed to parse WAVE value");
            KvError::WaveParse(e.to_string())
        })?;

        // Lower to canonical ABI
        let abi = CanonicalAbi::new(&resolve);
        let mut memory = LinearMemory::new();
        let encoded = abi.lower_with_memory(&value, &Type::Id(type_id), &wave_type, &mut memory)?;
        trace!(
            buffer_size = encoded.len(),
            memory_size = memory.len(),
            "value encoded to canonical ABI"
        );

        // Create StoredValue
        let stored = StoredValue::new(
            metadata.type_version,
            encoded,
            if memory.is_empty() {
                None
            } else {
                Some(memory.into_bytes())
            },
        );

        // Encode and store
        let (buffer, mem) = stored.encode()?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        self.store_with_memory(&ks, key, &buffer, &mem)?;
        self.db.persist(PersistMode::SyncAll)?;

        debug!(keyspace = keyspace, key = key, "value set");
        Ok(())
    }

    /// Get a value from a keyspace as WAVE text.
    pub fn get(&self, keyspace: &str, key: &str) -> Result<Option<String>, KvError> {
        debug!(keyspace = keyspace, key = key, "getting value");

        let metadata = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        // Load stored value
        let Some(stored) = self.load_stored_value(&ks, key)? else {
            trace!(keyspace = keyspace, key = key, "key not found");
            return Ok(None);
        };

        // Check type version compatibility
        if !metadata.type_version.can_read_from(&stored.type_version) {
            warn!(
                keyspace = keyspace,
                key = key,
                stored_version = %format!("{}.{}.{}", stored.type_version.major, stored.type_version.minor, stored.type_version.patch),
                current_version = %format!("{}.{}.{}", metadata.type_version.major, metadata.type_version.minor, metadata.type_version.patch),
                "type version mismatch"
            );
            return Err(KvError::TypeVersionMismatch {
                stored: stored.type_version,
                current: metadata.type_version,
            });
        }

        // Parse WIT type
        let (resolve, type_id, wave_type) = self.parse_stored_type(&metadata)?;

        // Lift from canonical ABI to Val, then convert to wasm_wave::Value for text display
        let abi = CanonicalAbi::new(&resolve);
        let memory = LinearMemory::from_option(stored.memory);

        let (val, _) = abi.lift_to_val(&stored.value, &Type::Id(type_id), None, &memory)?;
        let value = val_to_wave(&val, &wave_type).map_err(|e| KvError::WaveParse(e.to_string()))?;

        // Convert to WAVE text
        let wave_str = wasm_wave::to_string(&value).map_err(|e| KvError::WaveParse(e.to_string()))?;

        debug!(keyspace = keyspace, key = key, "value retrieved");
        Ok(Some(wave_str))
    }

    /// Get raw stored value (for --binary/--raw output).
    pub fn get_raw(&self, keyspace: &str, key: &str) -> Result<Option<StoredValue>, KvError> {
        let _ = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        self.load_stored_value(&ks, key)
    }

    /// Delete a value from a keyspace.
    pub fn delete(&self, keyspace: &str, key: &str) -> Result<(), KvError> {
        debug!(keyspace = keyspace, key = key, "deleting value");

        let _ = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        let memory_key = format!("{}.memory", key);
        ks.remove(key)?;
        ks.remove(&memory_key)?;

        self.db.persist(PersistMode::SyncAll)?;
        debug!(keyspace = keyspace, key = key, "value deleted");
        Ok(())
    }

    /// List keys in a keyspace with optional filtering.
    ///
    /// - `prefix`: Only return keys starting with this prefix
    /// - `start`: Only return keys >= start (inclusive)
    /// - `end`: Only return keys < end (exclusive)
    /// - `limit`: Maximum number of keys to return
    pub fn list(
        &self,
        keyspace: &str,
        prefix: Option<&str>,
        start: Option<&str>,
        end: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, KvError> {
        debug!(
            keyspace = keyspace,
            prefix = prefix,
            start = start,
            end = end,
            limit = limit,
            "listing keys"
        );

        let _ = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        let mut keys = Vec::new();

        // Use range or prefix based on what's provided
        let iter: Box<dyn Iterator<Item = _>> = match (start, end) {
            (Some(s), Some(e)) => Box::new(ks.range(s..e)),
            (Some(s), None) => Box::new(ks.range(s..)),
            (None, Some(e)) => Box::new(ks.range(..e)),
            (None, None) => match prefix {
                Some(p) => Box::new(ks.prefix(p)),
                None => Box::new(ks.prefix("")),
            },
        };

        for kv in iter {
            let Ok(key_bytes) = kv.key() else {
                continue;
            };
            let key_str = String::from_utf8_lossy(&key_bytes);

            // Skip memory keys
            if key_str.ends_with(".memory") {
                continue;
            }

            // Apply prefix filter if we're using range (range doesn't filter by prefix)
            if let Some(p) = prefix
                && !key_str.starts_with(p)
            {
                continue;
            }

            keys.push(key_str.into_owned());

            if let Some(l) = limit
                && keys.len() >= l
            {
                break;
            }
        }

        debug!(keyspace = keyspace, count = keys.len(), "listed keys");
        Ok(keys)
    }

    // Helper methods

    fn build_qualified_name(
        &self,
        resolve: &Resolve,
        type_id: TypeId,
        type_name: &str,
    ) -> Result<String, KvError> {
        let type_def = resolve.types.get(type_id).ok_or_else(|| {
            KvError::TypeNotFound(format!("Type id {:?} not found", type_id))
        })?;

        // Try to find the interface and package
        match type_def.owner {
            wit_parser::TypeOwner::Interface(iface_id) => {
                let Some(iface) = resolve.interfaces.get(iface_id) else {
                    return Ok(type_name.to_string());
                };
                if let Some(pkg_id) = iface.package
                    && let Some(pkg) = resolve.packages.get(pkg_id)
                {
                    let iface_name = iface.name.as_deref().unwrap_or("types");
                    return Ok(format!(
                        "{}:{}/{}#{}",
                        pkg.name.namespace, pkg.name.name, iface_name, type_name
                    ));
                }
            }
            wit_parser::TypeOwner::World(world_id) => {
                let Some(world) = resolve.worlds.get(world_id) else {
                    return Ok(type_name.to_string());
                };
                if let Some(pkg_id) = world.package
                    && let Some(pkg) = resolve.packages.get(pkg_id)
                {
                    return Ok(format!(
                        "{}:{}#{}",
                        pkg.name.namespace, pkg.name.name, type_name
                    ));
                }
            }
            wit_parser::TypeOwner::None => {}
        }

        // Fallback to just the type name
        Ok(type_name.to_string())
    }

    fn parse_stored_type(
        &self,
        metadata: &KeyspaceMetadata,
    ) -> Result<(Resolve, TypeId, wasm_wave::value::Type), KvError> {
        crate::load_wit_type_from_string(&metadata.wit_definition, Some(&metadata.type_name))
            .map_err(|e| KvError::WaveParse(e.to_string()))
    }

    fn store_with_memory(
        &self,
        keyspace: &Keyspace,
        key: &str,
        buffer: &[u8],
        memory: &[u8],
    ) -> Result<(), KvError> {
        keyspace.insert(key, buffer)?;
        if !memory.is_empty() {
            let memory_key = format!("{}.memory", key);
            keyspace.insert(&memory_key, memory)?;
        }
        Ok(())
    }

    fn load_metadata(&self, key: &str) -> Result<Option<KeyspaceMetadata>, KvError> {
        let Some(buffer) = self.meta.get(key)? else {
            return Ok(None);
        };

        let memory_key = format!("{}.memory", key);
        let memory = self
            .meta
            .get(&memory_key)?
            .map(|v| v.to_vec())
            .unwrap_or_default();

        Ok(Some(KeyspaceMetadata::decode(&buffer, &memory)?))
    }

    fn load_stored_value(
        &self,
        keyspace: &Keyspace,
        key: &str,
    ) -> Result<Option<StoredValue>, KvError> {
        let Some(buffer) = keyspace.get(key)? else {
            return Ok(None);
        };

        let memory_key = format!("{}.memory", key);
        let memory = keyspace
            .get(&memory_key)?
            .map(|v| v.to_vec())
            .unwrap_or_default();

        Ok(Some(StoredValue::decode(&buffer, &memory)?))
    }
}
