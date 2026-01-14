//! Typed person filter component - filters people with high scores.
//!
//! This component receives actual WIT types with direct field access.

wit_bindgen::generate!({
    world: "typed-map-module",
    path: "wit",
});

struct TypedPersonFilter;

impl Guest for TypedPersonFilter {
    /// Filter: return true only if score >= 100.
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
