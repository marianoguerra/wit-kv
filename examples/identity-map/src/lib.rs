//! Identity map component - passes all values through unchanged.
//!
//! This is a simple example component that implements the map-module world.
//! - filter: always returns true (process all values)
//! - transform: returns the input unchanged (identity function)

wit_bindgen::generate!({
    world: "map-module",
    path: "wit",
});

struct IdentityMap;

impl Guest for IdentityMap {
    /// Filter: always return true to process all values.
    fn filter(_value: BinaryExport) -> bool {
        true
    }

    /// Transform: return the value unchanged (identity function).
    fn transform(value: BinaryExport) -> BinaryExport {
        value
    }
}

export!(IdentityMap);
