use crate::error::Result;
use crate::insert_nested;
use std::collections::HashMap;
use std::path::Path;

/// A configuration layer that contributes values to the stack.
pub trait Layer {
    /// The name of this layer (for error reporting).
    fn name(&self) -> &str;

    /// Return this layer's configuration as a JSON value tree.
    fn json(&self) -> Result<serde_json::Value>;
}

/// Convert a `toml::Value` to a `serde_json::Value`, preserving types.
fn toml_to_json(value: toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number(i.into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => {
            let map: serde_json::Map<String, serde_json::Value> = table
                .into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Layer backed by environment variables.
///
/// Supports optional prefix filtering and key separator for creating
/// nested configuration structures from flat environment variable names.
pub struct EnvLayer {
    vars: HashMap<String, String>,
    prefix: Option<String>,
    separator: String,
}

impl EnvLayer {
    /// Create a new `EnvLayer` from the current process environment.
    pub fn from_env() -> Self {
        Self {
            vars: std::env::vars().collect(),
            prefix: None,
            separator: "__".to_string(),
        }
    }

    /// Create an `EnvLayer` from an explicit map of variables.
    pub fn from_map(vars: HashMap<String, String>) -> Self {
        Self {
            vars,
            prefix: None,
            separator: "__".to_string(),
        }
    }

    /// Only include environment variables that start with this prefix.
    /// The prefix is stripped from the resulting keys.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set the separator used to split flat env var names into nested JSON paths.
    /// Default is `"__"` (double underscore).
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }
}

impl Layer for EnvLayer {
    fn name(&self) -> &str {
        "env"
    }

    fn json(&self) -> Result<serde_json::Value> {
        let mut root = serde_json::Map::new();

        for (key, value) in &self.vars {
            let effective_key = if let Some(prefix) = &self.prefix {
                match key.strip_prefix(prefix.as_str()) {
                    Some(stripped) if !stripped.is_empty() => stripped,
                    _ => continue,
                }
            } else {
                key.as_str()
            };

            let json_value = serde_json::from_str(value)
                .unwrap_or_else(|_| serde_json::Value::String(value.clone()));

            let parts: Vec<String> = effective_key
                .split(self.separator.as_str())
                .map(|s| s.to_lowercase())
                .collect();
            let parts: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
            insert_nested(&mut root, &parts, json_value);
        }

        Ok(serde_json::Value::Object(root))
    }
}

/// Layer backed by a TOML file or string.
pub struct TomlLayer {
    value: toml::Value,
}

impl TomlLayer {
    /// Load a TOML file from the given path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let value: toml::Value = toml::from_str(&content)?;
        Ok(Self { value })
    }

    /// Parse a TOML string directly.
    pub fn from_str(content: &str) -> Result<Self> {
        let value: toml::Value = toml::from_str(content)?;
        Ok(Self { value })
    }
}

impl Layer for TomlLayer {
    fn name(&self) -> &str {
        "toml"
    }

    fn json(&self) -> Result<serde_json::Value> {
        Ok(toml_to_json(self.value.clone()))
    }
}

/// Layer that provides default values as a JSON tree.
pub struct DefaultsLayer {
    values: serde_json::Value,
}

impl DefaultsLayer {
    /// Create a new `DefaultsLayer` from a JSON value.
    pub fn new(values: serde_json::Value) -> Self {
        Self { values }
    }

    /// Create a `DefaultsLayer` from a flat map (keys are dot-separated paths).
    pub fn from_map(map: HashMap<String, serde_json::Value>) -> Self {
        let mut root = serde_json::Map::new();
        for (path, value) in map {
            let parts: Vec<&str> = path.split('.').collect();
            insert_nested(&mut root, &parts, value);
        }
        Self {
            values: serde_json::Value::Object(root),
        }
    }
}

impl Layer for DefaultsLayer {
    fn name(&self) -> &str {
        "defaults"
    }

    fn json(&self) -> Result<serde_json::Value> {
        Ok(self.values.clone())
    }
}
