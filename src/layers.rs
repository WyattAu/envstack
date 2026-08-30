use crate::error::{ConfigError, Result};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::Path;

/// A configuration layer that contributes values to the stack.
pub trait Layer {
    /// The name of this layer (for error reporting).
    fn name(&self) -> &str;

    /// Attempt to extract a value for the given key.
    fn get(&self, key: &str) -> Option<String>;
}

/// Layer backed by environment variables.
pub struct EnvLayer {
    vars: HashMap<String, String>,
}

impl EnvLayer {
    /// Create a new `EnvLayer` from the current process environment.
    pub fn from_env() -> Self {
        Self {
            vars: std::env::vars().collect(),
        }
    }

    /// Create a `EnvLayer` from an explicit map of variables.
    pub fn from_map(vars: HashMap<String, String>) -> Self {
        Self { vars }
    }
}

impl Layer for EnvLayer {
    fn name(&self) -> &str {
        "env"
    }

    fn get(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }
}

/// Layer backed by a TOML file.
pub struct TomlLayer {
    data: toml::Value,
}

impl TomlLayer {
    /// Load a TOML file from the given path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let data: toml::Value = toml::from_str(&content)?;
        Ok(Self { data })
    }

    /// Parse a TOML string directly.
    pub fn from_str(content: &str) -> Result<Self> {
        let data: toml::Value = toml::from_str(content)?;
        Ok(Self { data })
    }

    /// Resolve a dotted key path (e.g., `"server.host"`) against the TOML value.
    fn resolve(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &self.data;

        for part in &parts {
            current = current.get(part)?;
        }

        match current {
            toml::Value::String(s) => Some(s.clone()),
            toml::Value::Integer(i) => Some(i.to_string()),
            toml::Value::Float(f) => Some(f.to_string()),
            toml::Value::Boolean(b) => Some(b.to_string()),
            _ => Some(current.to_string()),
        }
    }
}

impl Layer for TomlLayer {
    fn name(&self) -> &str {
        "toml"
    }

    fn get(&self, key: &str) -> Option<String> {
        self.resolve(key)
    }
}

/// Layer that provides default values.
pub struct DefaultsLayer {
    defaults: HashMap<String, String>,
}

impl DefaultsLayer {
    /// Create a new `DefaultsLayer` from a map of default values.
    pub fn new(defaults: HashMap<String, String>) -> Self {
        Self { defaults }
    }
}

impl Layer for DefaultsLayer {
    fn name(&self) -> &str {
        "defaults"
    }

    fn get(&self, key: &str) -> Option<String> {
        self.defaults.get(key).cloned()
    }
}
