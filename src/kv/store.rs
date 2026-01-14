//! KV Store implementation using fjall.

use std::path::Path;

use fjall::{Keyspace, KeyspaceCreateOptions, PersistMode};
use wasm_wave::value::{resolve_wit_type, Value};
use wit_parser::{Resolve, Type, TypeId};

use crate::{CanonicalAbi, LinearMemory};

use super::error::KvError;
use super::types::{KeyspaceMetadata, StoredValue};

/// Key prefixes for the metadata keyspace.
const META_TYPES_PREFIX: &str = "types/";
const META_QUALIFIED_PREFIX: &str = "qualified/";
const META_CONFIG_KEY: &str = "config";

/// Data keyspace prefix.
const DATA_PREFIX: &str = "data_";

/// Current store version.
const STORE_VERSION: u32 = 1;

/// Key-value store backed by fjall.
pub struct KvStore {
    db: fjall::Database,
    meta: Keyspace,
}

impl KvStore {
    /// Open an existing KV store at the given path.
    pub fn open(path: &Path) -> Result<Self, KvError> {
        if !path.exists() {
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
                return Err(KvError::InvalidFormat(format!(
                    "Store version mismatch: expected {}, got {}",
                    STORE_VERSION, version
                )));
            }
        } else {
            return Err(KvError::NotInitialized(path.display().to_string()));
        }

        Ok(Self { db, meta })
    }

    /// Initialize a new KV store at the given path.
    pub fn init(path: &Path) -> Result<Self, KvError> {
        let db = fjall::Database::builder(path).open()?;
        let meta = db.keyspace("_meta", KeyspaceCreateOptions::default)?;

        // Write store version
        meta.insert(META_CONFIG_KEY, STORE_VERSION.to_le_bytes())?;
        db.persist(PersistMode::SyncAll)?;

        Ok(Self { db, meta })
    }

    /// Register a type for a keyspace.
    pub fn set_type(
        &self,
        name: &str,
        wit_path: &Path,
        type_name: Option<&str>,
        force: bool,
    ) -> Result<KeyspaceMetadata, KvError> {
        // Check if keyspace already exists
        let key = format!("{}{}", META_TYPES_PREFIX, name);
        if !force && self.meta.get(&key)?.is_some() {
            return Err(KvError::KeyspaceExists(name.to_string()));
        }

        // Parse the WIT file
        let mut resolve = Resolve::new();
        resolve.push_path(wit_path)?;

        // Find the type
        let (type_id, type_def) = match type_name {
            Some(tn) => resolve
                .types
                .iter()
                .find(|(_, ty)| ty.name.as_deref() == Some(tn))
                .ok_or_else(|| KvError::TypeNotFound(tn.to_string()))?,
            None => resolve
                .types
                .iter()
                .find(|(_, ty)| ty.name.is_some())
                .ok_or_else(|| KvError::TypeNotFound("No named type found".to_string()))?,
        };

        let actual_type_name = type_def.name.clone().unwrap_or_default();

        // Build qualified name from package info
        let qualified_name = self.build_qualified_name(&resolve, type_id, &actual_type_name)?;

        // Read WIT file content
        let wit_definition = std::fs::read_to_string(wit_path)?;

        // Create metadata
        let metadata = KeyspaceMetadata::new(
            name.to_string(),
            qualified_name.clone(),
            wit_definition,
            actual_type_name,
        );

        // Encode and store
        let (buffer, memory) = metadata.encode()?;
        self.store_with_memory(&self.meta, &key, &buffer, &memory)?;

        // Store reverse lookup
        let qualified_key = format!("{}{}", META_QUALIFIED_PREFIX, qualified_name);
        self.meta.insert(&qualified_key, name.as_bytes())?;

        // Create data keyspace for this keyspace
        let keyspace_name = format!("{}{}", DATA_PREFIX, name);
        let _ = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        self.db.persist(PersistMode::SyncAll)?;

        Ok(metadata)
    }

    /// Get the type metadata for a keyspace.
    pub fn get_type(&self, name: &str) -> Result<Option<KeyspaceMetadata>, KvError> {
        let key = format!("{}{}", META_TYPES_PREFIX, name);
        self.load_metadata(&key)
    }

    /// Delete a keyspace type (and optionally its data).
    pub fn delete_type(&self, name: &str, delete_data: bool) -> Result<(), KvError> {
        let key = format!("{}{}", META_TYPES_PREFIX, name);

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
            let keyspace_name = format!("{}{}", DATA_PREFIX, name);
            if let Ok(keyspace) = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default) {
                // Clear all keys - skip any keys that fail to read
                let keys: Vec<Vec<u8>> = keyspace
                    .iter()
                    .filter_map(|kv| kv.key().ok().map(|k| k.to_vec()))
                    .collect();
                for k in keys {
                    keyspace.remove(&k)?;
                }
            }
        }

        self.db.persist(PersistMode::SyncAll)?;
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
        let metadata = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        // Parse WIT type from stored definition
        let (resolve, type_id, wave_type) = self.parse_stored_type(&metadata)?;

        // Parse the WAVE value
        let value: Value = wasm_wave::from_str(&wave_type, wave_value)
            .map_err(|e| KvError::WaveParse(e.to_string()))?;

        // Lower to canonical ABI
        let abi = CanonicalAbi::new(&resolve);
        let mut memory = LinearMemory::new();
        let encoded = abi.lower_with_memory(&value, &Type::Id(type_id), &wave_type, &mut memory)?;

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

        Ok(())
    }

    /// Get a value from a keyspace as WAVE text.
    pub fn get(&self, keyspace: &str, key: &str) -> Result<Option<String>, KvError> {
        let metadata = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        // Load stored value
        let Some(stored) = self.load_stored_value(&ks, key)? else {
            return Ok(None);
        };

        // Check type version
        if stored.type_version != metadata.type_version {
            eprintln!(
                "Warning: Type version mismatch for key '{}': stored {}, current {}",
                key, stored.type_version, metadata.type_version
            );
        }

        // Parse WIT type
        let (resolve, type_id, wave_type) = self.parse_stored_type(&metadata)?;

        // Lift from canonical ABI
        let abi = CanonicalAbi::new(&resolve);
        let memory = stored
            .memory
            .map(LinearMemory::from_bytes)
            .unwrap_or_else(LinearMemory::new);

        let (value, _) =
            abi.lift_with_memory(&stored.value, &Type::Id(type_id), &wave_type, &memory)?;

        // Convert to WAVE text
        let wave_str = wasm_wave::to_string(&value).map_err(|e| KvError::WaveParse(e.to_string()))?;

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
        let _ = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        let memory_key = format!("{}.memory", key);
        ks.remove(key)?;
        ks.remove(&memory_key)?;

        self.db.persist(PersistMode::SyncAll)?;
        Ok(())
    }

    /// List keys in a keyspace.
    pub fn list(
        &self,
        keyspace: &str,
        prefix: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, KvError> {
        let _ = self
            .get_type(keyspace)?
            .ok_or_else(|| KvError::KeyspaceNotFound(keyspace.to_string()))?;

        let keyspace_name = format!("{}{}", DATA_PREFIX, keyspace);
        let ks = self.db.keyspace(&keyspace_name, KeyspaceCreateOptions::default)?;

        let mut keys = Vec::new();
        let iter = match prefix {
            Some(p) => ks.prefix(p),
            None => ks.prefix(""),
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

            keys.push(key_str.into_owned());

            if let Some(l) = limit {
                if keys.len() >= l {
                    break;
                }
            }
        }

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
                if let Some(pkg_id) = iface.package {
                    if let Some(pkg) = resolve.packages.get(pkg_id) {
                        let iface_name = iface.name.as_deref().unwrap_or("types");
                        return Ok(format!(
                            "{}:{}/{}#{}",
                            pkg.name.namespace, pkg.name.name, iface_name, type_name
                        ));
                    }
                }
            }
            wit_parser::TypeOwner::World(world_id) => {
                let Some(world) = resolve.worlds.get(world_id) else {
                    return Ok(type_name.to_string());
                };
                if let Some(pkg_id) = world.package {
                    if let Some(pkg) = resolve.packages.get(pkg_id) {
                        return Ok(format!(
                            "{}:{}#{}",
                            pkg.name.namespace, pkg.name.name, type_name
                        ));
                    }
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
        let mut resolve = Resolve::new();
        resolve.push_str("stored.wit", &metadata.wit_definition)?;

        let type_id = resolve
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some(&metadata.type_name))
            .map(|(id, _)| id)
            .ok_or_else(|| KvError::TypeNotFound(metadata.type_name.clone()))?;

        let wave_type =
            resolve_wit_type(&resolve, type_id).map_err(|e| KvError::WaveParse(e.to_string()))?;

        Ok((resolve, type_id, wave_type))
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
