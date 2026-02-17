use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::error::Error;

/// Backing store that persists values and schemas to disk.
///
/// Layout on disk:
/// ```text
/// backing/
///   <dir>/
///     .type.wit           # WIT schema (persisted as-is)
///     <name>.bin          # EncodedValue bytes (buffer + memory)
///     <name>.err.bin      # Error EncodedValue (only if error exists)
/// ```
pub struct Store {
    root: PathBuf,
}

/// Represents a stored value entry.
#[derive(Debug, Clone)]
pub struct StoredEntry {
    pub name: String,
    pub data: Vec<u8>,
}

impl Store {
    pub fn new(root: PathBuf) -> Result<Self, Error> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Get the backing path for a directory.
    fn dir_path(&self, dir: &str) -> PathBuf {
        self.root.join(dir)
    }

    /// Ensure a directory exists in the backing store.
    pub fn ensure_dir(&self, dir: &str) -> Result<(), Error> {
        fs::create_dir_all(self.dir_path(dir))?;
        Ok(())
    }

    /// Remove a directory from the backing store.
    pub fn remove_dir(&self, dir: &str) -> Result<(), Error> {
        let path = self.dir_path(dir);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    /// Read `.type.wit` for a directory.
    pub fn read_schema(&self, dir: &str) -> Result<Option<String>, Error> {
        let path = self.dir_path(dir).join(".type.wit");
        if path.exists() {
            Ok(Some(fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    /// Write `.type.wit` for a directory.
    pub fn write_schema(&self, dir: &str, content: &str) -> Result<(), Error> {
        self.ensure_dir(dir)?;
        let path = self.dir_path(dir).join(".type.wit");
        fs::write(path, content)?;
        Ok(())
    }

    /// Remove `.type.wit` for a directory.
    pub fn remove_schema(&self, dir: &str) -> Result<(), Error> {
        let path = self.dir_path(dir).join(".type.wit");
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Read a value's binary data.
    pub fn read_value(&self, dir: &str, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let path = self.dir_path(dir).join(format!("{name}.bin"));
        if path.exists() {
            Ok(Some(fs::read(path)?))
        } else {
            Ok(None)
        }
    }

    /// Write a value's binary data.
    pub fn write_value(&self, dir: &str, name: &str, data: &[u8]) -> Result<(), Error> {
        let path = self.dir_path(dir).join(format!("{name}.bin"));
        fs::write(path, data)?;
        Ok(())
    }

    /// Remove a value's binary data.
    pub fn remove_value(&self, dir: &str, name: &str) -> Result<(), Error> {
        let path = self.dir_path(dir).join(format!("{name}.bin"));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Read error binary data for a value.
    pub fn read_error(&self, dir: &str, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let path = self.dir_path(dir).join(format!("{name}.err.bin"));
        if path.exists() {
            Ok(Some(fs::read(path)?))
        } else {
            Ok(None)
        }
    }

    /// Write error binary data for a value.
    pub fn write_error(&self, dir: &str, name: &str, data: &[u8]) -> Result<(), Error> {
        let path = self.dir_path(dir).join(format!("{name}.err.bin"));
        fs::write(path, data)?;
        Ok(())
    }

    /// Write error WAVE text for a value.
    pub fn write_error_wave(&self, dir: &str, name: &str, text: &str) -> Result<(), Error> {
        let path = self.dir_path(dir).join(format!("{name}.err.wave"));
        fs::write(path, text)?;
        Ok(())
    }

    /// Read error WAVE text for a value.
    pub fn read_error_wave(&self, dir: &str, name: &str) -> Result<Option<String>, Error> {
        let path = self.dir_path(dir).join(format!("{name}.err.wave"));
        if path.exists() {
            Ok(Some(fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    /// Remove error files for a value.
    pub fn remove_error(&self, dir: &str, name: &str) -> Result<(), Error> {
        let bin_path = self.dir_path(dir).join(format!("{name}.err.bin"));
        if bin_path.exists() {
            fs::remove_file(bin_path)?;
        }
        let wave_path = self.dir_path(dir).join(format!("{name}.err.wave"));
        if wave_path.exists() {
            fs::remove_file(wave_path)?;
        }
        Ok(())
    }

    /// Check if an error exists for a value.
    pub fn has_error(&self, dir: &str, name: &str) -> bool {
        self.dir_path(dir)
            .join(format!("{name}.err.wave"))
            .exists()
    }

    /// List all value names in a directory (names without `.bin` extension).
    pub fn list_values(&self, dir: &str) -> Result<Vec<String>, Error> {
        let path = self.dir_path(dir);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if let Some(stem) = name.strip_suffix(".bin") {
                // Skip error files
                if !stem.ends_with(".err") {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// List all directories in the backing store.
    pub fn list_dirs(&self) -> Result<Vec<String>, Error> {
        let mut dirs = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                dirs.push(name);
            }
        }
        dirs.sort();
        Ok(dirs)
    }

    /// List all value names that have errors in a directory.
    pub fn list_errors(&self, dir: &str) -> Result<Vec<String>, Error> {
        let path = self.dir_path(dir);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".err.wave") {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load all values and errors for a directory into memory.
    pub fn load_directory(
        &self,
        dir: &str,
    ) -> Result<HashMap<String, (Vec<u8>, Option<String>, Option<Vec<u8>>)>, Error> {
        let mut entries = HashMap::new();
        let values = self.list_values(dir)?;
        for name in values {
            let data = self.read_value(dir, &name)?.unwrap_or_default();
            let err_wave = self.read_error_wave(dir, &name)?;
            let err_bin = self.read_error(dir, &name)?;
            entries.insert(name, (data, err_wave, err_bin));
        }
        Ok(entries)
    }

    /// Get the root path of the backing store.
    pub fn root(&self) -> &Path {
        &self.root
    }
}
