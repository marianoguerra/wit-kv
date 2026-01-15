//! Conditional logging macros for library-level tracing.
//!
//! When the `logging` feature is enabled, these macros forward to tracing.
//! When disabled, they compile to no-ops with zero runtime cost.
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::logging::{info, debug};
//!
//! info!(path = %path.display(), "opening store");
//! debug!(key = key, "getting value");
//! ```

/// Emit a trace-level log (very detailed internal operations).
#[cfg(feature = "logging")]
macro_rules! log_trace {
    ($($arg:tt)*) => { tracing::trace!($($arg)*) }
}

#[cfg(not(feature = "logging"))]
macro_rules! log_trace {
    ($($arg:tt)*) => {};
}

/// Emit a debug-level log (operation details useful for debugging).
#[cfg(feature = "logging")]
macro_rules! log_debug {
    ($($arg:tt)*) => { tracing::debug!($($arg)*) }
}

#[cfg(not(feature = "logging"))]
macro_rules! log_debug {
    ($($arg:tt)*) => {};
}

/// Emit an info-level log (high-level lifecycle events).
#[cfg(feature = "logging")]
macro_rules! log_info {
    ($($arg:tt)*) => { tracing::info!($($arg)*) }
}

#[cfg(not(feature = "logging"))]
macro_rules! log_info {
    ($($arg:tt)*) => {};
}

/// Emit a warn-level log (unexpected but handled situations).
#[cfg(feature = "logging")]
macro_rules! log_warn {
    ($($arg:tt)*) => { tracing::warn!($($arg)*) }
}

#[cfg(not(feature = "logging"))]
macro_rules! log_warn {
    ($($arg:tt)*) => {};
}

/// Emit an error-level log (failures that will propagate as errors).
#[cfg(feature = "logging")]
macro_rules! log_error {
    ($($arg:tt)*) => { tracing::error!($($arg)*) }
}

#[cfg(not(feature = "logging"))]
macro_rules! log_error {
    ($($arg:tt)*) => {};
}

pub(crate) use log_debug as debug;
pub(crate) use log_error as error;
pub(crate) use log_info as info;
pub(crate) use log_trace as trace;
pub(crate) use log_warn as warn;
