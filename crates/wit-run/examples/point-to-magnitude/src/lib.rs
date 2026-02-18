//! Point-to-magnitude transformation component.
//!
//! This component demonstrates T -> T1 type transformation:
//! - Input: `point { x: s32, y: s32 }`
//! - Output: `magnitude { distance-squared: u64, quadrant: u8 }`
//!
//! Shows how map operations can produce a different output type than input.

wit_bindgen::generate!({
    world: "typed-map-module",
    path: "wit",
});

struct PointToMagnitude;

impl Guest for PointToMagnitude {
    /// Filter: exclude the origin point (0, 0).
    fn filter(value: Point) -> bool {
        value.x != 0 || value.y != 0
    }

    /// Transform a point into magnitude information.
    ///
    /// Computes:
    /// - distance_squared: x² + y² (avoids floating point sqrt)
    /// - quadrant: 1 (++), 2 (-+), 3 (--), 4 (+-)
    fn transform(value: Point) -> Magnitude {
        let x = value.x as i64;
        let y = value.y as i64;

        let distance_squared = (x * x + y * y) as u64;

        // Determine quadrant based on sign of coordinates
        let quadrant = match (value.x >= 0, value.y >= 0) {
            (true, true) => 1,   // Q1: +x, +y
            (false, true) => 2,  // Q2: -x, +y
            (false, false) => 3, // Q3: -x, -y
            (true, false) => 4,  // Q4: +x, -y
        };

        Magnitude {
            distance_squared,
            quadrant,
        }
    }
}

export!(PointToMagnitude);
