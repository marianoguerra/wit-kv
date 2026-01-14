//! Typed sum-scores reduce component - sums all scores from person records.
//!
//! This component receives actual WIT types with direct field access.

wit_bindgen::generate!({
    world: "typed-reduce-module",
    path: "wit",
});

struct TypedSumScores;

impl Guest for TypedSumScores {
    /// Initialize state to zero.
    fn init_state() -> Total {
        Total { sum: 0, count: 0 }
    }

    /// Add the person's score to the running total.
    fn reduce(state: Total, value: Person) -> Total {
        Total {
            sum: state.sum + value.score as u64,
            count: state.count + 1,
        }
    }
}

export!(TypedSumScores);
