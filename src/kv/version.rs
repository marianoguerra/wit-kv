//! Semantic versioning support for WIT type versions.

/// Semantic version following WIT package versioning convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Initial version for new types (0.1.0).
    pub const INITIAL: Self = Self::new(0, 1, 0);

    /// Check if this version is compatible with reading data from `stored_version`.
    ///
    /// Compatibility rules:
    /// - Same major version (for major >= 1): compatible if current >= stored
    /// - Pre-1.0 (major = 0): compatible only if minor matches and current.patch >= stored.patch
    pub fn can_read_from(&self, stored: &Self) -> bool {
        if self.major == 0 && stored.major == 0 {
            // Pre-1.0: strict minor version matching
            self.minor == stored.minor && self.patch >= stored.patch
        } else if self.major == stored.major {
            // Same major: current must be >= stored
            (self.minor, self.patch) >= (stored.minor, stored.patch)
        } else {
            // Different major versions are incompatible
            false
        }
    }

    /// Check if data written with this version can be read by `reader_version`.
    pub fn can_be_read_by(&self, reader: &Self) -> bool {
        reader.can_read_from(self)
    }

    /// Parse from string like "1.2.3" or "0.1.0".
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        // Ensure no extra parts
        if parts.next().is_some() {
            return None;
        }
        Some(Self { major, minor, patch })
    }
}

impl std::fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Default for SemanticVersion {
    fn default() -> Self {
        Self::INITIAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        assert_eq!(SemanticVersion::parse("0.1.0"), Some(SemanticVersion::new(0, 1, 0)));
        assert_eq!(SemanticVersion::parse("1.2.3"), Some(SemanticVersion::new(1, 2, 3)));
        assert_eq!(SemanticVersion::parse("invalid"), None);
        assert_eq!(SemanticVersion::parse("1.2"), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(SemanticVersion::new(0, 1, 0).to_string(), "0.1.0");
        assert_eq!(SemanticVersion::new(1, 2, 3).to_string(), "1.2.3");
    }

    #[test]
    fn test_compatibility_pre_1_0() {
        let v010 = SemanticVersion::new(0, 1, 0);
        let v011 = SemanticVersion::new(0, 1, 1);
        let v020 = SemanticVersion::new(0, 2, 0);

        // Same minor, higher patch can read lower
        assert!(v011.can_read_from(&v010));
        // Same version can read itself
        assert!(v010.can_read_from(&v010));
        // Lower patch cannot read higher
        assert!(!v010.can_read_from(&v011));
        // Different minor versions are incompatible in pre-1.0
        assert!(!v020.can_read_from(&v010));
        assert!(!v010.can_read_from(&v020));
    }

    #[test]
    fn test_compatibility_post_1_0() {
        let v100 = SemanticVersion::new(1, 0, 0);
        let v110 = SemanticVersion::new(1, 1, 0);
        let v200 = SemanticVersion::new(2, 0, 0);

        // Higher minor can read lower
        assert!(v110.can_read_from(&v100));
        // Lower cannot read higher
        assert!(!v100.can_read_from(&v110));
        // Different major versions are incompatible
        assert!(!v200.can_read_from(&v100));
        assert!(!v100.can_read_from(&v200));
    }
}
