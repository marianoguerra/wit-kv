//! Sum-scores reduce component - sums all score fields from person records.
//!
//! This component assumes values are `person` records with layout:
//! - offset 0: age (u8)
//! - offset 4: score (u32, little-endian)
//!
//! The state is a u64 representing the running total.
//! Final output is the sum of all scores as a u64.

wit_bindgen::generate!({
    world: "reduce-module",
    path: "wit",
});

struct SumScores;

impl Guest for SumScores {
    /// Initialize state to 0 (u64, little-endian).
    fn init_state() -> BinaryExport {
        let sum: u64 = 0;
        BinaryExport {
            value: sum.to_le_bytes().to_vec(),
            memory: None,
        }
    }

    /// Add the score from the person record to the running total.
    fn reduce(state: BinaryExport, value: BinaryExport) -> BinaryExport {
        // Decode current sum from state (u64, little-endian)
        let current_sum = if state.value.len() >= 8 {
            u64::from_le_bytes([
                state.value[0], state.value[1], state.value[2], state.value[3],
                state.value[4], state.value[5], state.value[6], state.value[7],
            ])
        } else {
            0
        };

        // Extract score from person record (u32 at offset 4)
        let score = if value.value.len() >= 8 {
            u32::from_le_bytes([
                value.value[4], value.value[5], value.value[6], value.value[7],
            ])
        } else {
            0
        };

        // Compute new sum
        let new_sum = current_sum + score as u64;

        BinaryExport {
            value: new_sum.to_le_bytes().to_vec(),
            memory: None,
        }
    }
}

export!(SumScores);
