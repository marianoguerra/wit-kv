use std::collections::HashMap;

use wit_core::{ResolvedType, load_wit_type_from_string};

use crate::error::{FsError, VALIDATION_ERROR_WIT};

/// Cached parsed schema for a directory.
pub struct ParsedSchema {
    pub resolved: ResolvedType,
    pub wit_content: String,
}

/// Schema cache for directories.
pub struct SchemaCache {
    schemas: HashMap<String, ParsedSchema>,
    error_schema: Option<ParsedSchema>,
}

impl SchemaCache {
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
            error_schema: None,
        }
    }

    /// Parse and cache a schema for a directory.
    pub fn set_schema(&mut self, dir: &str, wit_content: &str) -> Result<(), FsError> {
        let resolved = load_wit_type_from_string(wit_content, None).map_err(|e| {
            FsError::Schema(format!("Failed to parse .type.wit: {e}"))
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

    /// Get the error schema (lazily parsed).
    pub fn get_error_schema(&mut self) -> Result<&ParsedSchema, FsError> {
        if self.error_schema.is_none() {
            let resolved =
                load_wit_type_from_string(VALIDATION_ERROR_WIT, Some("validation-error"))
                    .map_err(|e| FsError::Schema(format!("Failed to parse error schema: {e}")))?;
            self.error_schema = Some(ParsedSchema {
                resolved,
                wit_content: VALIDATION_ERROR_WIT.to_string(),
            });
        }
        // We just set it above, so this is safe
        Ok(self.error_schema.as_ref().unwrap_or_else(|| unreachable!()))
    }

    /// Get the `.type.error.wit` content.
    pub fn error_wit_content(&self) -> &str {
        VALIDATION_ERROR_WIT
    }
}
