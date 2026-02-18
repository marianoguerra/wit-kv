use std::fmt;

/// Error type for serde operations on `wasm_wave::Value`.
#[derive(Debug)]
pub enum WitSerdeError {
    /// A custom error message (from serde's `custom` method).
    Message(String),
    /// A type mismatch between the Value kind and the expected serde type.
    TypeMismatch {
        expected: &'static str,
        actual: String,
    },
    /// Error constructing a Value via `make_*` methods.
    ValueConstruction(String),
}

impl fmt::Display for WitSerdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(msg) => write!(f, "{msg}"),
            Self::TypeMismatch { expected, actual } => {
                write!(f, "type mismatch: expected {expected}, got {actual}")
            }
            Self::ValueConstruction(msg) => write!(f, "value construction error: {msg}"),
        }
    }
}

impl std::error::Error for WitSerdeError {}

impl serde::de::Error for WitSerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Message(msg.to_string())
    }
}

impl serde::ser::Error for WitSerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Message(msg.to_string())
    }
}
