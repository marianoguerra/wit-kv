use std::collections::HashMap;

use wit_core::{ResolvedType, load_wit_type_from_string};

use super::error::{Error, VALIDATION_ERROR_WIT};

/// Cached parsed schema for a directory.
pub struct ParsedSchema {
    pub resolved: ResolvedType,
    pub wit_content: String,
}

/// Schema cache for directories.
pub struct SchemaCache {
    schemas: HashMap<String, ParsedSchema>,
}

impl SchemaCache {
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    /// Parse and cache a schema for a directory.
    pub fn set_schema(&mut self, dir: &str, wit_content: &str) -> Result<(), Error> {
        let resolved = load_wit_type_from_string(wit_content, None).map_err(|e| {
            Error::Schema(format!("Failed to parse .type.wit: {e}"))
        })?;
        self.schemas.insert(
            dir.to_string(),
            ParsedSchema {
                resolved,
                wit_content: wit_content.to_string(),
            },
        );
        Ok(())
    }

    /// Remove a cached schema.
    pub fn remove_schema(&mut self, dir: &str) {
        self.schemas.remove(dir);
    }

    /// Get a cached schema for a directory.
    pub fn get_schema(&self, dir: &str) -> Option<&ParsedSchema> {
        self.schemas.get(dir)
    }

    /// Check if a directory has a schema.
    pub fn has_schema(&self, dir: &str) -> bool {
        self.schemas.contains_key(dir)
    }

    /// Get the `.type.error.wit` content.
    pub fn error_wit_content(&self) -> &str {
        VALIDATION_ERROR_WIT
    }
}
