//! Typed point filter component - filters points based on distance from origin.
//!
//! This component demonstrates the TYPED approach to map/reduce:
//! - Receives actual `point` records, not binary blobs
//! - No manual binary parsing required
//! - Type safety at the interface boundary
//!
//! Compare with `high-score-filter` which must manually parse bytes.

wit_bindgen::generate!({
    world: "typed-map-module",
    path: "wit",
});

struct TypedPointFilter;

impl Guest for TypedPointFilter {
    /// Filter: return true only if point is within distance 100 from origin.
    ///
    /// With typed interface, we directly access fields - no binary parsing!
    fn filter(value: Point) -> bool {
        // Direct field access - clean and type-safe
        let distance_squared = (value.x as i64) * (value.x as i64)
                             + (value.y as i64) * (value.y as i64);

        // Filter points within radius 100 from origin
        distance_squared <= 100 * 100
    }

    /// Transform: double the coordinates.
    ///
    /// Returns a new point with doubled coordinates.
    fn transform(value: Point) -> Point {
        Point {
            x: value.x * 2,
            y: value.y * 2,
        }
    }
}

export!(TypedPointFilter);
