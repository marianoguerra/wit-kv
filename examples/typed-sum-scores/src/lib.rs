//! Typed sum-scores reduce component - sums all scores from person records.
//!
//! This is the TYPED equivalent of `sum-scores`.
//!
//! Compare the implementations:
//!
//! ## sum-scores (binary-export approach):
//! ```rust,ignore
//! fn init_state() -> BinaryExport {
//!     let sum: u64 = 0;
//!     BinaryExport { value: sum.to_le_bytes().to_vec(), memory: None }
//! }
//!
//! fn reduce(state: BinaryExport, value: BinaryExport) -> BinaryExport {
//!     let current_sum = u64::from_le_bytes([state.value[0], ...]);
//!     let score = u32::from_le_bytes([value.value[4], ...]);
//!     // ... manual byte manipulation
//! }
//! ```
//!
//! ## typed-sum-scores (typed approach):
//! ```rust,ignore
//! fn init_state() -> Total {
//!     Total { sum: 0, count: 0 }
//! }
//!
//! fn reduce(state: Total, value: Person) -> Total {
//!     Total {
//!         sum: state.sum + value.score as u64,
//!         count: state.count + 1,
//!     }
//! }
//! ```
//!
//! The typed approach is:
//! - Cleaner: no manual byte parsing
//! - Safer: type-checked at compile time
//! - Simpler: direct field access

wit_bindgen::generate!({
    world: "typed-reduce-module",
    path: "wit",
});

struct TypedSumScores;

impl Guest for TypedSumScores {
    /// Initialize state to zero.
    ///
    /// Compare to sum-scores which must manually encode u64 to bytes.
    fn init_state() -> Total {
        Total { sum: 0, count: 0 }
    }

    /// Add the person's score to the running total.
    ///
    /// Compare to sum-scores which must:
    /// 1. Decode u64 from state bytes
    /// 2. Extract u32 score at offset 4 from value bytes
    /// 3. Encode new u64 sum back to bytes
    ///
    /// Here we just access fields directly!
    fn reduce(state: Total, value: Person) -> Total {
        Total {
            sum: state.sum + value.score as u64,
            count: state.count + 1,
        }
    }
}

export!(TypedSumScores);
