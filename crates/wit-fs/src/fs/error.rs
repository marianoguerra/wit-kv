//! Error types for the wit-fs filesystem.

use thiserror::Error as ThisError;

/// The WIT definition for the validation error type.
///
/// This is served as the content of `.type.error.wit` files and used
/// to encode/decode validation errors as WIT-typed values.
pub const VALIDATION_ERROR_WIT: &str = "\
package witfs:errors;

interface errors {
    record validation-error {
        message: string,
        timestamp: string,
        input: string,
        error-kind: error-kind,
    }

    enum error-kind {
        wave-parse,
        type-mismatch,
        schema-error,
        abi-error,
    }
}
";

/// The kind of validation error that occurred.
#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
    WaveParse,
    TypeMismatch,
    SchemaError,
    AbiError,
}

impl ErrorKind {
    fn as_wave_str(self) -> &'static str {
        match self {
            Self::WaveParse => "wave-parse",
            Self::TypeMismatch => "type-mismatch",
            Self::SchemaError => "schema-error",
            Self::AbiError => "abi-error",
        }
    }
}

/// A validation error that occurred when writing a value.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub message: String,
    pub timestamp: String,
    pub input: String,
    pub error_kind: ErrorKind,
}

impl ValidationError {
    pub fn new(message: String, input: String, error_kind: ErrorKind) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| {
                // Format as ISO 8601 manually (no chrono dependency)
                let secs = d.as_secs();
                let days = secs / 86400;
                let time_secs = secs % 86400;
                let hours = time_secs / 3600;
                let minutes = (time_secs % 3600) / 60;
                let seconds = time_secs % 60;

                // Simple date calculation from epoch days
                let (year, month, day) = days_to_date(days);
                format!(
                    "{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z"
                )
            })
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

        Self {
            message,
            timestamp,
            input,
            error_kind,
        }
    }

    /// Encode this error as WAVE text (for `.witerr` files).
    pub fn to_wave_string(&self) -> String {
        let message = wave_escape_string(&self.message);
        let timestamp = wave_escape_string(&self.timestamp);
        let input = wave_escape_string(&self.input);
        let kind = self.error_kind.as_wave_str();
        format!(
            "{{message: \"{message}\", timestamp: \"{timestamp}\", input: \"{input}\", error-kind: {kind}}}"
        )
    }

    /// Encode this error as canonical ABI binary (for `.witerrb` files).
    pub fn to_binary(&self) -> Option<Vec<u8>> {
        let resolved =
            wit_core::load_wit_type_from_string(VALIDATION_ERROR_WIT, Some("validation-error"))
                .ok()?;
        let wave_text = self.to_wave_string();
        wit_core::wave_to_binary(&wave_text, &resolved).ok()
    }
}

/// Escape a string for WAVE text format.
fn wave_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_date(days: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's chrono-compatible date library
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// A type alias for `Result` with the wit-fs [`Error`] type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors specific to the wit-fs filesystem.
#[derive(ThisError, Debug)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] wit_core::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Validation error: {0}")]
    Validation(String),
}
