//! Logging initialization and configuration.

use std::fs::OpenOptions;
use std::io::{self, IsTerminal};
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

use super::config::{LogFormat, LoggingConfig};

/// Initialize the tracing subscriber based on configuration.
pub fn init(config: &LoggingConfig) -> Result<(), LoggingError> {
    let filter = EnvFilter::try_new(&config.level)
        .map_err(|e| LoggingError::InvalidFilter(e.to_string()))?;

    match config.format {
        LogFormat::Text => init_text_subscriber(config, filter),
        LogFormat::Json => init_json_subscriber(config, filter),
    }
}

fn init_text_subscriber(config: &LoggingConfig, filter: EnvFilter) -> Result<(), LoggingError> {
    match config.output.as_str() {
        "stdout" => {
            let layer = fmt::layer()
                .with_ansi(config.color && io::stdout().is_terminal())
                .with_target(config.target)
                .with_span_events(FmtSpan::NONE)
                .with_writer(io::stdout);

            if config.timestamps {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer.without_time())
                    .init();
            }
        }
        "stderr" => {
            let layer = fmt::layer()
                .with_ansi(config.color && io::stderr().is_terminal())
                .with_target(config.target)
                .with_span_events(FmtSpan::NONE)
                .with_writer(io::stderr);

            if config.timestamps {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer.without_time())
                    .init();
            }
        }
        path => {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| LoggingError::FileOpen(path.to_string(), e))?;

            let layer = fmt::layer()
                .with_ansi(false)
                .with_target(config.target)
                .with_span_events(FmtSpan::NONE)
                .with_writer(file);

            if config.timestamps {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer.without_time())
                    .init();
            }
        }
    }

    Ok(())
}

fn init_json_subscriber(config: &LoggingConfig, filter: EnvFilter) -> Result<(), LoggingError> {
    match config.output.as_str() {
        "stdout" => {
            let layer = fmt::layer()
                .json()
                .with_target(config.target)
                .with_span_events(FmtSpan::NONE)
                .with_writer(io::stdout);

            if config.timestamps {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer.without_time())
                    .init();
            }
        }
        "stderr" => {
            let layer = fmt::layer()
                .json()
                .with_target(config.target)
                .with_span_events(FmtSpan::NONE)
                .with_writer(io::stderr);

            if config.timestamps {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer.without_time())
                    .init();
            }
        }
        path => {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| LoggingError::FileOpen(path.to_string(), e))?;

            let layer = fmt::layer()
                .json()
                .with_target(config.target)
                .with_span_events(FmtSpan::NONE)
                .with_writer(file);

            if config.timestamps {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(layer.without_time())
                    .init();
            }
        }
    }

    Ok(())
}

/// Errors that can occur during logging initialization.
#[derive(Debug)]
pub enum LoggingError {
    /// Invalid log filter string.
    InvalidFilter(String),
    /// Failed to open log file.
    FileOpen(String, io::Error),
}

impl std::fmt::Display for LoggingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoggingError::InvalidFilter(msg) => write!(f, "Invalid log filter: {}", msg),
            LoggingError::FileOpen(path, e) => {
                write!(f, "Failed to open log file '{}': {}", path, e)
            }
        }
    }
}

impl std::error::Error for LoggingError {}
