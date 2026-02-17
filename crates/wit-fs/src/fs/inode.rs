use std::collections::HashMap;
use std::ffi::OsStr;

/// The root inode number (FUSE convention).
pub const ROOT_INO: u64 = 1;

/// What kind of file an inode represents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InodeKind {
    /// Root directory.
    RootDir,
    /// A typed subdirectory (e.g. `points/`).
    TypedDir {
        dir_name: String,
    },
    /// The `.type.wit` schema file in a directory.
    SchemaFile {
        dir_name: String,
    },
    /// The `.type.error.wit` auto-generated error schema file.
    ErrorSchemaFile {
        dir_name: String,
    },
    /// A `.wave` view of a value.
    WaveFile {
        dir_name: String,
        value_name: String,
    },
    /// A `.witb` view of a value.
    WitbFile {
        dir_name: String,
        value_name: String,
    },
    /// A `.witerr` error file (WAVE text).
    WiterrFile {
        dir_name: String,
        value_name: String,
    },
    /// A `.witerrb` error file (binary).
    WiterrbFile {
        dir_name: String,
        value_name: String,
    },
}

impl InodeKind {
    /// Whether this inode is a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::RootDir | Self::TypedDir { .. })
    }

    /// Whether this inode is read-only.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            Self::ErrorSchemaFile { .. } | Self::WiterrFile { .. } | Self::WiterrbFile { .. }
        )
    }

    /// The file name for this inode in a directory listing.
    pub fn file_name(&self) -> Option<String> {
        match self {
            Self::RootDir => None,
            Self::TypedDir { dir_name } => Some(dir_name.clone()),
            Self::SchemaFile { .. } => Some(".type.wit".to_string()),
            Self::ErrorSchemaFile { .. } => Some(".type.error.wit".to_string()),
            Self::WaveFile { value_name, .. } => Some(format!("{value_name}.wave")),
            Self::WitbFile { value_name, .. } => Some(format!("{value_name}.witb")),
            Self::WiterrFile { value_name, .. } => Some(format!("{value_name}.witerr")),
            Self::WiterrbFile { value_name, .. } => Some(format!("{value_name}.witerrb")),
        }
    }
}

/// An entry in the inode table.
#[derive(Debug, Clone)]
pub struct InodeEntry {
    pub ino: u64,
    pub kind: InodeKind,
    pub parent: u64,
}

/// The inode table mapping inodes to entries.
pub struct InodeTable {
    next_ino: u64,
    entries: HashMap<u64, InodeEntry>,
    /// Map from (parent_ino, filename) → child inode for fast lookup.
    lookup_cache: HashMap<(u64, String), u64>,
}

impl InodeTable {
    pub fn new() -> Self {
        let mut table = Self {
            next_ino: 2, // 1 is reserved for root
            entries: HashMap::new(),
            lookup_cache: HashMap::new(),
        };
        table.entries.insert(
            ROOT_INO,
            InodeEntry {
                ino: ROOT_INO,
                kind: InodeKind::RootDir,
                parent: ROOT_INO,
            },
        );
        table
    }

    /// Allocate a new inode.
    fn alloc_ino(&mut self) -> u64 {
        let ino = self.next_ino;
        self.next_ino += 1;
        ino
    }

    /// Insert an inode entry and update the lookup cache.
    fn insert(&mut self, entry: InodeEntry) -> u64 {
        let ino = entry.ino;
        if let Some(name) = entry.kind.file_name() {
            self.lookup_cache.insert((entry.parent, name), ino);
        }
        self.entries.insert(ino, entry);
        ino
    }

    /// Get an inode entry by inode number.
    pub fn get(&self, ino: u64) -> Option<&InodeEntry> {
        self.entries.get(&ino)
    }

    /// Look up a child inode by parent and filename.
    pub fn lookup(&self, parent: u64, name: &OsStr) -> Option<u64> {
        let name = name.to_string_lossy().to_string();
        self.lookup_cache.get(&(parent, name)).copied()
    }

    /// List children of a directory inode.
    pub fn children(&self, parent: u64) -> Vec<&InodeEntry> {
        self.entries
            .values()
            .filter(|e| e.parent == parent && e.ino != parent)
            .collect()
    }

    /// Add a typed directory under root.
    pub fn add_dir(&mut self, dir_name: &str) -> u64 {
        // Check if it already exists
        if let Some(&ino) = self.lookup_cache.get(&(ROOT_INO, dir_name.to_string())) {
            return ino;
        }
        let ino = self.alloc_ino();
        self.insert(InodeEntry {
            ino,
            kind: InodeKind::TypedDir {
                dir_name: dir_name.to_string(),
            },
            parent: ROOT_INO,
        })
    }

    /// Get the inode for a typed directory, if it exists.
    pub fn get_dir_ino(&self, dir_name: &str) -> Option<u64> {
        self.lookup_cache
            .get(&(ROOT_INO, dir_name.to_string()))
            .copied()
    }

    /// Add schema file inodes for a directory.
    pub fn add_schema_files(&mut self, dir_name: &str, dir_ino: u64) {
        // .type.wit
        if !self
            .lookup_cache
            .contains_key(&(dir_ino, ".type.wit".to_string()))
        {
            let ino = self.alloc_ino();
            self.insert(InodeEntry {
                ino,
                kind: InodeKind::SchemaFile {
                    dir_name: dir_name.to_string(),
                },
                parent: dir_ino,
            });
        }

        // .type.error.wit
        if !self
            .lookup_cache
            .contains_key(&(dir_ino, ".type.error.wit".to_string()))
        {
            let ino = self.alloc_ino();
            self.insert(InodeEntry {
                ino,
                kind: InodeKind::ErrorSchemaFile {
                    dir_name: dir_name.to_string(),
                },
                parent: dir_ino,
            });
        }
    }

    /// Add value file inodes (.wave + .witb) for a value in a directory.
    pub fn add_value(&mut self, dir_name: &str, dir_ino: u64, value_name: &str) {
        // .wave
        let wave_key = (dir_ino, format!("{value_name}.wave"));
        if !self.lookup_cache.contains_key(&wave_key) {
            let ino = self.alloc_ino();
            self.insert(InodeEntry {
                ino,
                kind: InodeKind::WaveFile {
                    dir_name: dir_name.to_string(),
                    value_name: value_name.to_string(),
                },
                parent: dir_ino,
            });
        }

        // .witb
        let witb_key = (dir_ino, format!("{value_name}.witb"));
        if !self.lookup_cache.contains_key(&witb_key) {
            let ino = self.alloc_ino();
            self.insert(InodeEntry {
                ino,
                kind: InodeKind::WitbFile {
                    dir_name: dir_name.to_string(),
                    value_name: value_name.to_string(),
                },
                parent: dir_ino,
            });
        }
    }

    /// Add error file inodes (.witerr + .witerrb) for a value.
    pub fn add_error_files(&mut self, dir_name: &str, dir_ino: u64, value_name: &str) {
        // .witerr
        let witerr_key = (dir_ino, format!("{value_name}.witerr"));
        if !self.lookup_cache.contains_key(&witerr_key) {
            let ino = self.alloc_ino();
            self.insert(InodeEntry {
                ino,
                kind: InodeKind::WiterrFile {
                    dir_name: dir_name.to_string(),
                    value_name: value_name.to_string(),
                },
                parent: dir_ino,
            });
        }

        // .witerrb
        let witerrb_key = (dir_ino, format!("{value_name}.witerrb"));
        if !self.lookup_cache.contains_key(&witerrb_key) {
            let ino = self.alloc_ino();
            self.insert(InodeEntry {
                ino,
                kind: InodeKind::WiterrbFile {
                    dir_name: dir_name.to_string(),
                    value_name: value_name.to_string(),
                },
                parent: dir_ino,
            });
        }
    }

    /// Remove error file inodes for a value.
    pub fn remove_error_files(&mut self, dir_ino: u64, value_name: &str) {
        let witerr_key = (dir_ino, format!("{value_name}.witerr"));
        if let Some(ino) = self.lookup_cache.remove(&witerr_key) {
            self.entries.remove(&ino);
        }

        let witerrb_key = (dir_ino, format!("{value_name}.witerrb"));
        if let Some(ino) = self.lookup_cache.remove(&witerrb_key) {
            self.entries.remove(&ino);
        }
    }

    /// Remove value file inodes (.wave, .witb, .witerr, .witerrb) for a value.
    pub fn remove_value(&mut self, dir_ino: u64, value_name: &str) {
        for ext in &["wave", "witb", "witerr", "witerrb"] {
            let key = (dir_ino, format!("{value_name}.{ext}"));
            if let Some(ino) = self.lookup_cache.remove(&key) {
                self.entries.remove(&ino);
            }
        }
    }

    /// Remove a directory and all its children.
    pub fn remove_dir(&mut self, dir_name: &str) {
        let dir_key = (ROOT_INO, dir_name.to_string());
        if let Some(dir_ino) = self.lookup_cache.remove(&dir_key) {
            // Remove all children
            let child_inos: Vec<u64> = self
                .entries
                .values()
                .filter(|e| e.parent == dir_ino)
                .map(|e| e.ino)
                .collect();
            for child_ino in child_inos {
                if let Some(entry) = self.entries.remove(&child_ino)
                    && let Some(name) = entry.kind.file_name()
                {
                    self.lookup_cache.remove(&(dir_ino, name));
                }
            }
            self.entries.remove(&dir_ino);
        }
    }

    /// Remove schema file inodes for a directory.
    pub fn remove_schema_files(&mut self, dir_ino: u64) {
        let schema_key = (dir_ino, ".type.wit".to_string());
        if let Some(ino) = self.lookup_cache.remove(&schema_key) {
            self.entries.remove(&ino);
        }

        let error_schema_key = (dir_ino, ".type.error.wit".to_string());
        if let Some(ino) = self.lookup_cache.remove(&error_schema_key) {
            self.entries.remove(&ino);
        }
    }

    /// Check if a value has error files.
    pub fn has_error_files(&self, dir_ino: u64, value_name: &str) -> bool {
        let witerr_key = (dir_ino, format!("{value_name}.witerr"));
        self.lookup_cache.contains_key(&witerr_key)
    }
}

/// Parse a filename into its components (stem, extension).
///
/// Returns `(stem, extension)` where extension includes the dot-prefixed format:
/// - `"foo.wave"` → `("foo", Some("wave"))`
/// - `"foo.witb"` → `("foo", Some("witb"))`
/// - `"foo.witerr"` → `("foo", Some("witerr"))`
/// - `"foo.witerrb"` → `("foo", Some("witerrb"))`
/// - `".type.wit"` → special case
/// - `".type.error.wit"` → special case
pub fn parse_filename(name: &str) -> FilenameParts {
    if name == ".type.wit" {
        return FilenameParts::SchemaFile;
    }
    if name == ".type.error.wit" {
        return FilenameParts::ErrorSchemaFile;
    }
    if let Some(stem) = name.strip_suffix(".witerrb")
        && !stem.is_empty()
    {
        return FilenameParts::Value {
            stem: stem.to_string(),
            ext: ValueExt::Witerrb,
        };
    }
    if let Some(stem) = name.strip_suffix(".witerr")
        && !stem.is_empty()
    {
        return FilenameParts::Value {
            stem: stem.to_string(),
            ext: ValueExt::Witerr,
        };
    }
    if let Some(stem) = name.strip_suffix(".wave")
        && !stem.is_empty()
    {
        return FilenameParts::Value {
            stem: stem.to_string(),
            ext: ValueExt::Wave,
        };
    }
    if let Some(stem) = name.strip_suffix(".witb")
        && !stem.is_empty()
    {
        return FilenameParts::Value {
            stem: stem.to_string(),
            ext: ValueExt::Witb,
        };
    }
    FilenameParts::Unknown
}

#[derive(Debug, PartialEq, Eq)]
pub enum FilenameParts {
    SchemaFile,
    ErrorSchemaFile,
    Value { stem: String, ext: ValueExt },
    Unknown,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueExt {
    Wave,
    Witb,
    Witerr,
    Witerrb,
}
