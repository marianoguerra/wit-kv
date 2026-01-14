//! Typed person filter component - filters people with high scores.
//!
//! This is the TYPED equivalent of `high-score-filter`.
//!
//! Compare the implementations:
//!
//! ## high-score-filter (binary-export approach):
//! ```rust,ignore
//! fn filter(value: BinaryExport) -> bool {
//!     let bytes = &value.value;
//!     if bytes.len() < 8 { return false; }
//!     let score = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
//!     score >= 100
//! }
//! ```
//!
//! ## typed-person-filter (typed approach):
//! ```rust,ignore
//! fn filter(value: Person) -> bool {
//!     value.score >= 100
//! }
//! ```
//!
//! The typed approach is:
//! - Cleaner: no manual byte parsing
//! - Safer: type-checked at compile time
//! - Simpler: direct field access

wit_bindgen::generate!({
    world: "typed-map-module",
    path: "wit",
});

struct TypedPersonFilter;

impl Guest for TypedPersonFilter {
    /// Filter: return true only if score >= 100.
    ///
    /// Compare to high-score-filter which must:
    /// 1. Check byte length
    /// 2. Extract bytes at correct offset
    /// 3. Convert to u32 manually
    ///
    /// Here we just access the field directly!
    fn filter(value: Person) -> bool {
        value.score >= 100
    }

    /// Transform: increment age by 1 (birthday!).
    fn transform(value: Person) -> Person {
        Person {
            age: value.age.saturating_add(1),
            score: value.score,
        }
    }
}

export!(TypedPersonFilter);
