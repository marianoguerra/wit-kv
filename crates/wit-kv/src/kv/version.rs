//! Semantic versioning support for WIT type versions.

use std::fmt;
use std::str::FromStr;

/// Error returned when parsing a [`SemanticVersion`] from a string fails.
///
/// # Example
///
/// ```ignore
/// use wit_kv::SemanticVersion;
/// use std::str::FromStr;
///
/// let err = SemanticVersion::from_str("invalid").unwrap_err();
/// println!("Parse error: {}", err);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseVersionError {
    input: String,
    reason: &'static str,
}

impl fmt::Display for ParseVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid version '{}': {}", self.input, self.reason)
    }
}

impl std::error::Error for ParseVersionError {}

/// Semantic version following WIT package versioning convention.
///
/// # Example
///
/// ```ignore
/// use wit_kv::SemanticVersion;
/// use std::str::FromStr;
///
/// // Parse from string
/// let v: SemanticVersion = "1.2.3".parse()?;
///
/// // Create directly
/// let v = SemanticVersion::new(1, 2, 3);
///
/// // Display
/// println!("Version: {}", v); // "1.2.3"
/// ```
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

impl FromStr for SemanticVersion {
    type Err = ParseVersionError;

    /// Parse a semantic version from a string.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wit_kv::SemanticVersion;
    /// use std::str::FromStr;
    ///
    /// let v: SemanticVersion = "1.2.3".parse()?;
    /// assert_eq!(v, SemanticVersion::new(1, 2, 3));
    ///
    /// // Also works with from_str
    /// let v = SemanticVersion::from_str("0.1.0")?;
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');

        let major = parts
            .next()
            .ok_or_else(|| ParseVersionError {
                input: s.to_string(),
                reason: "missing major version",
            })?
            .parse()
            .map_err(|_| ParseVersionError {
                input: s.to_string(),
                reason: "invalid major version number",
            })?;

        let minor = parts
            .next()
            .ok_or_else(|| ParseVersionError {
                input: s.to_string(),
                reason: "missing minor version",
            })?
            .parse()
            .map_err(|_| ParseVersionError {
                input: s.to_string(),
                reason: "invalid minor version number",
            })?;

        let patch = parts
            .next()
            .ok_or_else(|| ParseVersionError {
                input: s.to_string(),
                reason: "missing patch version",
            })?
            .parse()
            .map_err(|_| ParseVersionError {
                input: s.to_string(),
                reason: "invalid patch version number",
            })?;

        // Ensure no extra parts
        if parts.next().is_some() {
            return Err(ParseVersionError {
                input: s.to_string(),
                reason: "too many version components",
            });
        }

        Ok(Self { major, minor, patch })
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

    #[test]
    fn test_from_str() {
        // Valid versions
        assert_eq!(
            "0.1.0".parse::<SemanticVersion>().ok(),
            Some(SemanticVersion::new(0, 1, 0))
        );
        assert_eq!(
            "1.2.3".parse::<SemanticVersion>().ok(),
            Some(SemanticVersion::new(1, 2, 3))
        );
        assert_eq!(
            "10.20.30".parse::<SemanticVersion>().ok(),
            Some(SemanticVersion::new(10, 20, 30))
        );

        // Invalid versions
        assert!("invalid".parse::<SemanticVersion>().is_err());
        assert!("1.2".parse::<SemanticVersion>().is_err());
        assert!("1".parse::<SemanticVersion>().is_err());
        assert!("1.2.3.4".parse::<SemanticVersion>().is_err());
        assert!("a.b.c".parse::<SemanticVersion>().is_err());
        assert!("-1.0.0".parse::<SemanticVersion>().is_err());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_from_str_error_messages() {
        let err = "invalid".parse::<SemanticVersion>().unwrap_err();
        assert!(err.to_string().contains("invalid"));

        let err = "1.2".parse::<SemanticVersion>().unwrap_err();
        assert!(err.to_string().contains("missing patch"));

        let err = "1.2.3.4".parse::<SemanticVersion>().unwrap_err();
        assert!(err.to_string().contains("too many"));
    }
}
