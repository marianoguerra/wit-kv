//! High-score filter component - filters values where score >= 100.
//!
//! This component assumes the value is a `person` record with layout:
//! - offset 0: age (u8)
//! - offset 4: score (u32, little-endian)
//!
//! It filters to only include records where score >= 100.

wit_bindgen::generate!({
    world: "map-module",
    path: "wit",
});

struct HighScoreFilter;

impl Guest for HighScoreFilter {
    /// Filter: return true only if score >= 100.
    fn filter(value: BinaryExport) -> bool {
        // Extract score from canonical ABI encoded person record
        // person { age: u8, score: u32 }
        // Layout: [age:1][pad:3][score:4] = 8 bytes
        let bytes = &value.value;
        if bytes.len() < 8 {
            return false; // Invalid format
        }

        // Score is at offset 4, little-endian u32
        let score = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        score >= 100
    }

    /// Transform: pass through unchanged.
    fn transform(value: BinaryExport) -> BinaryExport {
        value
    }
}

export!(HighScoreFilter);
