use std::fmt;

/// Errors that can occur when working with `envstack`.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A required configuration field is missing.
    #[error("missing required field: `{field}`")]
    MissingField {
        field: String,
    },

    /// Failed to parse a configuration value.
    #[error("parse error in `{field}`: {message}")]
    ParseError {
        field: String,
        message: String,
    },

    /// A configuration value failed validation.
    #[error("validation failed for `{field}`: {message}")]
    ValidationError {
        field: String,
        message: String,
    },

    /// An I/O error occurred while reading a config file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A TOML parsing error.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// A JSON parsing error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// A specialized `Result` type for `envstack` operations.
pub type Result<T> = std::result::Result<T, ConfigError>;
