/// Errors that can occur when working with `envstack`.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A required configuration field is missing.
    #[error("missing required field: `{field}`")]
    MissingField {
        /// Name of the missing field.
        field: String,
    },

    /// Failed to parse a configuration value.
    #[error("parse error in `{field}`: {message}")]
    ParseError {
        /// Field that failed to parse.
        field: String,
        /// Parse error message.
        message: String,
    },

    /// A configuration value failed validation.
    #[error("validation failed for `{field}`: {message}")]
    ValidationError {
        /// Field that failed validation.
        field: String,
        /// Validation error message.
        message: String,
    },

    /// A configuration layer failed to load.
    #[error("layer `{layer}` failed: {message}")]
    LayerError {
        /// Name of the failing layer.
        layer: String,
        /// Error message.
        message: String,
    },

    /// An I/O error occurred while reading a config file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A TOML parsing error.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// A JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A YAML parsing error.
    #[cfg(feature = "yaml")]
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// A dotenv parsing error.
    #[cfg(feature = "dotenv")]
    #[error("dotenv error: {0}")]
    Dotenv(#[from] dotenvy::Error),

    /// A custom error message.
    #[error("{0}")]
    Custom(String),
}

/// A specialized `Result` type for `envstack` operations.
pub type Result<T> = std::result::Result<T, ConfigError>;
