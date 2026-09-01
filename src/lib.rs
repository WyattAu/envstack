#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Layered configuration for Rust.
//!
//! `envstack` merges configuration from multiple sources — environment
//! variables, TOML files, and CLI arguments — with type-safe extraction
//! and optional validation.
//!
//! # Example
//!
//! ```rust,no_run
//! use envstack::ConfigStack;
//!
//! #[derive(serde::Deserialize)]
//! struct AppConfig {
//!     host: String,
//!     port: u16,
//! }
//!
//! let config: AppConfig = ConfigStack::new()
//!     .with_default("host", "localhost")
//!     .with_default("port", serde_json::json!(8080))
//!     .extract()
//!     .expect("failed to load config");
//! ```

/// Error types.
pub mod error;
/// Configuration layer implementations.
pub mod layers;

pub use error::{ConfigError, Result};
pub use layers::{DefaultsLayer, EnvLayer, Layer, TomlLayer};

use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt;

/// A wrapper that redacts the inner value when displayed.
///
/// # Example
///
/// ```rust
/// use envstack::Secret;
///
/// #[derive(serde::Deserialize)]
/// struct Config {
///     api_key: Secret<String>,
/// }
///
/// let secret = Secret("my-api-key".to_string());
/// assert_eq!(format!("{secret}"), "[REDACTED]");
/// ```
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Secret<T>(pub T);

impl<T: fmt::Debug> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T: fmt::Display> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T: PartialEq> PartialEq for Secret<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Eq> Eq for Secret<T> {}

impl<T> Secret<T> {
    /// Access the inner value.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

/// A stack of configuration layers, merged in priority order.
///
/// Earlier layers take precedence over later ones. Use the builder
/// methods to add layers, then call [`extract`](Self::extract) to
/// obtain a typed configuration struct.
pub struct ConfigStack {
    layers: Vec<Box<dyn Layer>>,
    validator: Option<Box<dyn Fn(&serde_json::Value) -> Result<()>>>,
}

impl Default for ConfigStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigStack {
    /// Create an empty configuration stack.
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            validator: None,
        }
    }

    /// Add environment variables as a high-priority layer.
    pub fn with_env(mut self) -> Self {
        self.layers.push(Box::new(EnvLayer::from_env()));
        self
    }

    /// Add environment variables filtered by prefix.
    ///
    /// Only variables whose names start with `prefix` are included,
    /// and the prefix is stripped from the resulting keys.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use envstack::ConfigStack;
    ///
    /// // Only reads env vars starting with "APP_"
    /// let stack = ConfigStack::new().with_env_prefix("APP_");
    /// ```
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.layers
            .push(Box::new(EnvLayer::from_env().with_prefix(prefix)));
        self
    }

    /// Add a custom environment layer from an explicit map.
    pub fn with_env_map(mut self, vars: HashMap<String, String>) -> Self {
        self.layers.push(Box::new(EnvLayer::from_map(vars)));
        self
    }

    /// Add a TOML file as a configuration layer (lenient).
    ///
    /// If the file does not exist, the layer is silently skipped.
    /// Use [`with_toml_file_strict`](Self::with_toml_file_strict)
    /// to error on missing files.
    pub fn with_toml_file(mut self, path: impl AsRef<std::path::Path>) -> Self {
        if let Ok(layer) = TomlLayer::from_file(path) {
            self.layers.push(Box::new(layer));
        }
        self
    }

    /// Add a TOML file as a configuration layer (strict).
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn with_toml_file_strict(
        mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        let layer = TomlLayer::from_file(path)?;
        self.layers.push(Box::new(layer));
        Ok(self)
    }

    /// Add a raw TOML string as a configuration layer.
    pub fn with_toml_str(mut self, content: &str) -> Result<Self> {
        let layer = TomlLayer::from_str(content)?;
        self.layers.push(Box::new(layer));
        Ok(self)
    }

    /// Add a default value at a dot-separated key path.
    ///
    /// # Example
    ///
    /// ```rust
    /// use envstack::ConfigStack;
    ///
    /// let stack = ConfigStack::new()
    ///     .with_default("server.host", "localhost")
    ///     .with_default("server.port", serde_json::json!(8080));
    /// ```
    pub fn with_default(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        let parts: Vec<&str> = key.split('.').collect();
        let mut root = serde_json::Map::new();
        insert_nested(&mut root, &parts, value.into());
        self.layers
            .push(Box::new(DefaultsLayer::new(serde_json::Value::Object(root))));
        self
    }

    /// Add a custom layer.
    pub fn with_layer(mut self, layer: impl Layer + 'static) -> Self {
        self.layers.push(Box::new(layer));
        self
    }

    /// Register a validation function that runs before extraction.
    ///
    /// The function receives the merged JSON value and should return
    /// `Ok(())` if valid, or a `ConfigError` describing the problem.
    pub fn validate(mut self, f: impl Fn(&serde_json::Value) -> Result<()> + 'static) -> Self {
        self.validator = Some(Box::new(f));
        self
    }

    /// Look up a single key, returning the first value found across layers.
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        for layer in &self.layers {
            if let Ok(value) = layer.json() {
                if let Some(v) = resolve_json(&value, key) {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Merge all layers into a single JSON value.
    pub fn merge(&self) -> Result<serde_json::Value> {
        let mut result = serde_json::Value::Object(serde_json::Map::new());

        // Layers are in priority order: first layer wins.
        // We iterate in reverse so the first layer (highest priority) overwrites.
        for layer in self.layers.iter().rev() {
            let layer_json = layer.json().map_err(|e| ConfigError::LayerError {
                layer: layer.name().to_string(),
                message: e.to_string(),
            })?;
            result = deep_merge(result, layer_json);
        }

        Ok(result)
    }

    /// Extract a typed configuration struct from the merged layers.
    ///
    /// Runs validation if a validator was registered, then deserializes
    /// the merged JSON into `T`.
    pub fn extract<T: DeserializeOwned>(&self) -> Result<T> {
        let merged = self.merge()?;

        if let Some(ref validate) = self.validator {
            validate(&merged)?;
        }

        let config: T = serde_json::from_value(merged).map_err(|e| ConfigError::ParseError {
            field: "<root>".to_string(),
            message: e.to_string(),
        })?;

        Ok(config)
    }
}

/// Deep merge two JSON values. When both values are objects, they are
/// merged recursively. Otherwise the `override` value wins.
pub fn deep_merge(base: serde_json::Value, override_val: serde_json::Value) -> serde_json::Value {
    match (base, override_val) {
        (serde_json::Value::Object(mut base_map), serde_json::Value::Object(override_map)) => {
            for (key, value) in override_map {
                let merged = match base_map.remove(&key) {
                    Some(base_value) => deep_merge(base_value, value),
                    None => value,
                };
                base_map.insert(key, merged);
            }
            serde_json::Value::Object(base_map)
        }
        (_, override_val) => override_val,
    }
}

/// Insert a value into a nested JSON map following a path of key parts.
pub(crate) fn insert_nested(
    map: &mut serde_json::Map<String, serde_json::Value>,
    parts: &[&str],
    value: serde_json::Value,
) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        map.insert(parts[0].to_string(), value);
    } else {
        let key = parts[0].to_string();
        let inner = map
            .entry(key)
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(inner_map) = inner {
            insert_nested(inner_map, &parts[1..], value);
        }
    }
}

/// Resolve a dot-separated key path against a JSON value.
fn resolve_json(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_layer_prefix_filtering() {
        let mut vars = HashMap::new();
        vars.insert("APP_HOST".into(), "127.0.0.1".into());
        vars.insert("APP_PORT".into(), "3000".into());
        vars.insert("OTHER_VAR".into(), "ignored".into());

        let layer = EnvLayer::from_map(vars).with_prefix("APP_");
        let json = layer.json().unwrap();

        assert_eq!(
            json.get("host"),
            Some(&serde_json::Value::String("127.0.0.1".into()))
        );
        // "3000" is valid JSON so it parses as a number
        assert_eq!(json.get("port"), Some(&serde_json::json!(3000)));
        assert_eq!(json.get("OTHER_VAR"), None);
    }

    #[test]
    fn env_layer_separator_creates_nesting() {
        let mut vars = HashMap::new();
        vars.insert("SERVER__HOST".into(), "localhost".into());
        vars.insert("SERVER__PORT".into(), "8080".into());

        let layer = EnvLayer::from_map(vars);
        let json = layer.json().unwrap();

        let server = json.get("server").unwrap();
        assert_eq!(
            server.get("host"),
            Some(&serde_json::Value::String("localhost".into()))
        );
        assert_eq!(server.get("port"), Some(&serde_json::json!(8080)));
    }

    #[test]
    fn env_layer_json_value_parsing() {
        let mut vars = HashMap::new();
        vars.insert("PORT".into(), "8080".into());
        vars.insert("DEBUG".into(), "true".into());
        vars.insert("NAME".into(), "test".into());

        let layer = EnvLayer::from_map(vars);
        let json = layer.json().unwrap();

        assert_eq!(json.get("port"), Some(&serde_json::json!(8080)));
        assert_eq!(json.get("debug"), Some(&serde_json::json!(true)));
        assert_eq!(
            json.get("name"),
            Some(&serde_json::Value::String("test".into()))
        );
    }

    #[test]
    fn toml_layer_nested_structures() {
        let toml_content = r#"
            [server]
            host = "0.0.0.0"
            port = 3000

            [database]
            url = "postgres://localhost/mydb"
            pool_size = 10
        "#;
        let layer = TomlLayer::from_str(toml_content).unwrap();
        let json = layer.json().unwrap();

        let server = json.get("server").unwrap();
        assert_eq!(
            server.get("host"),
            Some(&serde_json::Value::String("0.0.0.0".into()))
        );
        assert_eq!(server.get("port"), Some(&serde_json::json!(3000)));

        let db = json.get("database").unwrap();
        assert_eq!(
            db.get("url"),
            Some(&serde_json::Value::String("postgres://localhost/mydb".into()))
        );
        assert_eq!(db.get("pool_size"), Some(&serde_json::json!(10)));
    }

    #[test]
    fn toml_preserves_types() {
        let toml_content = r#"
            string_val = "hello"
            int_val = 42
            float_val = 3.14
            bool_val = true
        "#;
        let layer = TomlLayer::from_str(toml_content).unwrap();
        let json = layer.json().unwrap();

        assert!(json.get("string_val").unwrap().is_string());
        assert!(json.get("int_val").unwrap().is_number());
        assert!(json.get("float_val").unwrap().is_number());
        assert!(json.get("bool_val").unwrap().is_boolean());
    }

    #[test]
    fn deep_merge_objects() {
        let base = serde_json::json!({
            "a": 1,
            "b": { "x": 10, "y": 20 }
        });
        let override_val = serde_json::json!({
            "b": { "y": 99, "z": 30 },
            "c": 3
        });

        let merged = deep_merge(base, override_val);

        assert_eq!(merged.get("a"), Some(&serde_json::json!(1)));
        assert_eq!(merged.get("c"), Some(&serde_json::json!(3)));

        let b = merged.get("b").unwrap();
        assert_eq!(b.get("x"), Some(&serde_json::json!(10)));
        assert_eq!(b.get("y"), Some(&serde_json::json!(99)));
        assert_eq!(b.get("z"), Some(&serde_json::json!(30)));
    }

    #[test]
    fn deep_merge_non_object_overrides() {
        let base = serde_json::json!({"a": 1});
        let override_val = serde_json::json!("string");
        let merged = deep_merge(base, override_val);
        assert_eq!(merged, serde_json::Value::String("string".into()));
    }

    #[test]
    fn validation_hook_rejects() {
        let stack = ConfigStack::new()
            .with_default("port", serde_json::json!(0))
            .validate(|v| {
                let port = v.get("port").and_then(|p| p.as_u64()).unwrap_or(0);
                if port == 0 {
                    return Err(ConfigError::ValidationError {
                        field: "port".into(),
                        message: "must be > 0".into(),
                    });
                }
                Ok(())
            });

        let result: std::result::Result<serde_json::Value, _> = stack.extract();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("port"));
    }

    #[test]
    fn validation_hook_accepts() {
        let stack = ConfigStack::new()
            .with_default("port", serde_json::json!(8080))
            .validate(|v| {
                let port = v.get("port").and_then(|p| p.as_u64()).unwrap_or(0);
                if port == 0 {
                    return Err(ConfigError::ValidationError {
                        field: "port".into(),
                        message: "must be > 0".into(),
                    });
                }
                Ok(())
            });

        let result: std::result::Result<serde_json::Value, _> = stack.extract();
        assert!(result.is_ok());
    }

    #[test]
    fn secret_redaction() {
        #[derive(serde::Deserialize, Debug)]
        struct Config {
            api_key: Secret<String>,
        }

        let json = serde_json::json!({"api_key": "super-secret"});
        let config: Config = serde_json::from_value(json).unwrap();

        assert_eq!(format!("{:?}", config.api_key), "[REDACTED]");
        assert_eq!(format!("{}", config.api_key), "[REDACTED]");
        assert_eq!(config.api_key.expose(), "super-secret");
    }

    #[test]
    fn extract_with_real_struct() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct AppConfig {
            host: String,
            port: u16,
        }

        let config: AppConfig = ConfigStack::new()
            .with_default("host", "localhost")
            .with_default("port", serde_json::json!(8080))
            .extract()
            .unwrap();

        assert_eq!(
            config,
            AppConfig {
                host: "localhost".into(),
                port: 8080
            }
        );
    }

    #[test]
    fn layer_priority_env_over_default() {
        let mut vars = HashMap::new();
        vars.insert("key".into(), "env_value".into());

        let config: serde_json::Value = ConfigStack::new()
            .with_env_map(vars)
            .with_default("key", "default_value")
            .extract()
            .unwrap();

        assert_eq!(
            config.get("key"),
            Some(&serde_json::Value::String("env_value".into()))
        );
    }

    #[test]
    fn merge_preserves_toml_types() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
            port: u16,
            debug: bool,
        }

        let toml_content = r#"
            name = "myapp"
            port = 3000
            debug = true
        "#;

        let config: Config = ConfigStack::new()
            .with_toml_str(toml_content)
            .unwrap()
            .extract()
            .unwrap();

        assert_eq!(
            config,
            Config {
                name: "myapp".into(),
                port: 3000,
                debug: true,
            }
        );
    }

    #[test]
    fn nested_default_key_path() {
        let stack = ConfigStack::new()
            .with_default("server.host", "localhost")
            .with_default("server.port", serde_json::json!(8080));

        let merged = stack.merge().unwrap();
        let server = merged.get("server").unwrap();
        assert_eq!(
            server.get("host"),
            Some(&serde_json::Value::String("localhost".into()))
        );
        assert_eq!(server.get("port"), Some(&serde_json::json!(8080)));
    }

    #[test]
    fn env_prefix_with_separator() {
        let mut vars = HashMap::new();
        vars.insert("APP_SERVER__HOST".into(), "10.0.0.1".into());
        vars.insert("APP_SERVER__PORT".into(), "9090".into());

        let layer = EnvLayer::from_map(vars)
            .with_prefix("APP_")
            .with_separator("__");

        let json = layer.json().unwrap();
        let server = json.get("server").unwrap();
        assert_eq!(
            server.get("host"),
            Some(&serde_json::Value::String("10.0.0.1".into()))
        );
        assert_eq!(server.get("port"), Some(&serde_json::json!(9090)));
    }

    #[test]
    fn empty_stack_extract() {
        #[derive(serde::Deserialize, Debug)]
        struct Config {
            #[serde(default)]
            name: String,
        }

        let config: Config = ConfigStack::new().extract().unwrap();
        assert_eq!(config.name, "");
    }

    #[test]
    fn defaults_layer_from_map() {
        let mut map = HashMap::new();
        map.insert("server.host".into(), serde_json::json!("localhost"));
        map.insert("server.port".into(), serde_json::json!(8080));

        let layer = DefaultsLayer::from_map(map);
        let json = layer.json().unwrap();

        let server = json.get("server").unwrap();
        assert_eq!(
            server.get("host"),
            Some(&serde_json::Value::String("localhost".into()))
        );
        assert_eq!(server.get("port"), Some(&serde_json::json!(8080)));
    }

    #[test]
    fn custom_layer_trait() {
        struct StaticLayer;
        impl Layer for StaticLayer {
            fn name(&self) -> &str {
                "static"
            }
            fn json(&self) -> Result<serde_json::Value> {
                Ok(serde_json::json!({"key": "value"}))
            }
        }

        let config: serde_json::Value = ConfigStack::new()
            .with_layer(StaticLayer)
            .extract()
            .unwrap();

        assert_eq!(
            config.get("key"),
            Some(&serde_json::Value::String("value".into()))
        );
    }

    #[test]
    fn error_display_messages() {
        let err = ConfigError::MissingField {
            field: "host".into(),
        };
        assert!(err.to_string().contains("host"));

        let err = ConfigError::ParseError {
            field: "port".into(),
            message: "invalid".into(),
        };
        assert!(err.to_string().contains("port"));

        let err = ConfigError::ValidationError {
            field: "email".into(),
            message: "bad".into(),
        };
        assert!(err.to_string().contains("email"));

        let err = ConfigError::LayerError {
            layer: "toml".into(),
            message: "parse failed".into(),
        };
        assert!(err.to_string().contains("toml"));

        let err = ConfigError::Custom("something went wrong".into());
        assert!(err.to_string().contains("something went wrong"));
    }

    #[test]
    fn toml_layer_from_file_nonexistent() {
        let result = TomlLayer::from_file("/nonexistent/path.toml");
        assert!(result.is_err());
    }

    #[test]
    fn toml_layer_invalid_syntax() {
        let result = TomlLayer::from_str("this is not [valid toml");
        assert!(result.is_err());
    }

    #[test]
    fn multiple_defaults_deep_merge() {
        let stack = ConfigStack::new()
            .with_default("a.b.c", serde_json::json!(1))
            .with_default("a.b.d", serde_json::json!(2))
            .with_default("a.e", serde_json::json!(3));

        let merged = stack.merge().unwrap();
        let a = merged.get("a").unwrap();
        let b = a.get("b").unwrap();
        assert_eq!(b.get("c"), Some(&serde_json::json!(1)));
        assert_eq!(b.get("d"), Some(&serde_json::json!(2)));
        assert_eq!(a.get("e"), Some(&serde_json::json!(3)));
    }

    #[test]
    fn merge_layer_priority_order() {
        let mut vars = HashMap::new();
        vars.insert("host".into(), "from-env".into());

        let toml_content = r#"
            host = "from-toml"
            port = 8080
        "#;

        let config: serde_json::Value = ConfigStack::new()
            .with_env_map(vars)
            .with_toml_str(toml_content)
            .unwrap()
            .with_default("host", "from-default")
            .with_default("port", serde_json::json!(0))
            .extract()
            .unwrap();

        // env wins over toml wins over default
        assert_eq!(
            config.get("host"),
            Some(&serde_json::Value::String("from-env".into()))
        );
        // toml wins over default
        assert_eq!(config.get("port"), Some(&serde_json::json!(8080)));
    }

    // ---- Additional ConfigStack with env prefix tests ----

    #[test]
    fn config_stack_env_prefix_excludes_non_matching() {
        let mut vars = HashMap::new();
        vars.insert("APP_HOST".into(), "127.0.0.1".into());
        vars.insert("DATABASE_URL".into(), "postgres://localhost".into());

        let layer = EnvLayer::from_map(vars).with_prefix("APP_");
        let json = layer.json().unwrap();

        assert!(json.get("host").is_some());
        assert!(json.get("database_url").is_none());
        assert!(json.get("DATABASE_URL").is_none());
    }

    #[test]
    fn config_stack_env_prefix_exact_match_excluded() {
        let mut vars = HashMap::new();
        vars.insert("APP_".into(), "should be excluded".into());
        vars.insert("APP_HOST".into(), "included".into());

        let layer = EnvLayer::from_map(vars).with_prefix("APP_");
        let json = layer.json().unwrap();

        assert!(json.as_object().unwrap().len() == 1);
        assert_eq!(
            json.get("host"),
            Some(&serde_json::Value::String("included".into()))
        );
    }

    // ---- Additional ConfigStack with TOML file tests ----

    #[test]
    fn config_stack_with_toml_file_strict_missing_returns_error() {
        let result = ConfigStack::new()
            .with_toml_file_strict("/nonexistent/path/config.toml");
        match result {
            Err(err) => {
                let msg = err.to_string();
                assert!(msg.contains("I/O error") || msg.contains("No such file"));
            }
            Ok(_) => panic!("expected error for nonexistent file"),
        }
    }

    #[test]
    fn config_stack_with_toml_file_lenient_missing_is_ok() {
        let result = ConfigStack::new()
            .with_toml_file("/nonexistent/path/config.toml")
            .extract::<serde_json::Value>();
        assert!(result.is_ok());
    }

    #[test]
    fn config_stack_toml_file_actual_content() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("envstack_test_toml");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "name = \"testapp\"\nport = 9090").unwrap();

        let config: serde_json::Value = ConfigStack::new()
            .with_toml_file(&path)
            .extract()
            .unwrap();

        assert_eq!(
            config.get("name"),
            Some(&serde_json::Value::String("testapp".into()))
        );
        assert_eq!(config.get("port"), Some(&serde_json::json!(9090)));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn config_stack_toml_file_with_nested_sections() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("envstack_test_toml2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nested.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "[server]\nhost = \"0.0.0.0\"\nport = 3000\n\n[database]\nurl = \"postgres://localhost/mydb\"\npool_size = 10").unwrap();

        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct ServerConfig {
            host: String,
            port: u16,
        }

        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            server: ServerConfig,
        }

        let config: Config = ConfigStack::new()
            .with_toml_file(&path)
            .extract()
            .unwrap();

        assert_eq!(
            config,
            Config {
                server: ServerConfig {
                    host: "0.0.0.0".into(),
                    port: 3000,
                }
            }
        );

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn config_stack_toml_invalid_syntax_strict() {
        let result = ConfigStack::new()
            .with_toml_str("this is not = [valid toml");
        assert!(result.is_err());
    }

    // ---- Additional ConfigStack with defaults tests ----

    #[test]
    fn config_stack_defaults_deeply_nested() {
        let stack = ConfigStack::new()
            .with_default("a.b.c.d.e", serde_json::json!(42));
        let merged = stack.merge().unwrap();
        let a = merged.get("a").unwrap();
        let b = a.get("b").unwrap();
        let c = b.get("c").unwrap();
        let d = c.get("d").unwrap();
        assert_eq!(d.get("e"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn config_stack_defaults_override() {
        let stack = ConfigStack::new()
            .with_default("key", "first")
            .with_default("key", "second");
        let config: serde_json::Value = stack.extract().unwrap();
        assert_eq!(
            config.get("key"),
            Some(&serde_json::Value::String("first".into()))
        );
    }

    #[test]
    fn config_stack_defaults_various_types() {
        let stack = ConfigStack::new()
            .with_default("string_val", "hello")
            .with_default("int_val", serde_json::json!(42))
            .with_default("float_val", serde_json::json!(3.14))
            .with_default("bool_val", serde_json::json!(true))
            .with_default("null_val", serde_json::Value::Null)
            .with_default("array_val", serde_json::json!([1, 2, 3]));
        let merged = stack.merge().unwrap();
        assert!(merged.get("string_val").unwrap().is_string());
        assert!(merged.get("int_val").unwrap().is_number());
        assert!(merged.get("float_val").unwrap().is_number());
        assert!(merged.get("bool_val").unwrap().is_boolean());
        assert!(merged.get("null_val").unwrap().is_null());
        assert!(merged.get("array_val").unwrap().is_array());
    }

    // ---- Additional extract() with simple struct tests ----

    #[test]
    fn extract_struct_with_optional_fields() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
            #[serde(default)]
            debug: bool,
            #[serde(default)]
            count: u32,
        }

        let config: Config = ConfigStack::new()
            .with_default("name", "myapp")
            .extract()
            .unwrap();

        assert_eq!(config.name, "myapp");
        assert!(!config.debug);
        assert_eq!(config.count, 0);
    }

    #[test]
    fn extract_struct_with_vec() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            tags: Vec<String>,
        }

        let config: Config = ConfigStack::new()
            .with_default("tags", serde_json::json!(["alpha", "beta"]))
            .extract()
            .unwrap();

        assert_eq!(config.tags, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn extract_failure_on_missing_required_field() {
        #[derive(serde::Deserialize, Debug)]
        struct Config {
            required_field: String,
        }

        let result = ConfigStack::new().extract::<Config>();
        assert!(result.is_err());
    }

    #[test]
    fn extract_type_mismatch() {
        #[derive(serde::Deserialize, Debug)]
        struct Config {
            port: u16,
        }

        let result = ConfigStack::new()
            .with_default("port", "not_a_number")
            .extract::<Config>();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("parse error") || msg.contains("<root>"));
    }

    // ---- Additional ConfigError display tests ----

    #[test]
    fn config_error_missing_field_display() {
        let err = ConfigError::MissingField {
            field: "database_url".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("missing required field"));
        assert!(msg.contains("database_url"));
    }

    #[test]
    fn config_error_parse_error_display() {
        let err = ConfigError::ParseError {
            field: "port".into(),
            message: "invalid digit found in string".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("parse error"));
        assert!(msg.contains("port"));
        assert!(msg.contains("invalid digit found in string"));
    }

    #[test]
    fn config_error_validation_error_display() {
        let err = ConfigError::ValidationError {
            field: "email".into(),
            message: "must contain @".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("validation failed"));
        assert!(msg.contains("email"));
        assert!(msg.contains("must contain @"));
    }

    #[test]
    fn config_error_layer_error_display() {
        let err = ConfigError::LayerError {
            layer: "env".into(),
            message: "permission denied".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("layer"));
        assert!(msg.contains("env"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn config_error_io_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = ConfigError::Io(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn config_error_custom_display() {
        let err = ConfigError::Custom("something went wrong".into());
        let msg = err.to_string();
        assert_eq!(msg, "something went wrong");
    }

    // ---- Additional TOML layer tests ----

    #[test]
    fn toml_layer_arrays() {
        let toml_content = r#"
            hosts = ["a", "b", "c"]
            ports = [80, 443, 8080]
        "#;
        let layer = TomlLayer::from_str(toml_content).unwrap();
        let json = layer.json().unwrap();

        let hosts = json.get("hosts").unwrap().as_array().unwrap();
        assert_eq!(hosts.len(), 3);
        assert_eq!(hosts[0], serde_json::Value::String("a".into()));

        let ports = json.get("ports").unwrap().as_array().unwrap();
        assert_eq!(ports.len(), 3);
        assert_eq!(ports[0], serde_json::json!(80));
    }

    #[test]
    fn toml_layer_nested_tables() {
        let toml_content = r#"
            [database]
            host = "localhost"
            port = 5432

            [database.credentials]
            user = "admin"
            password = "secret"
        "#;
        let layer = TomlLayer::from_str(toml_content).unwrap();
        let json = layer.json().unwrap();

        let db = json.get("database").unwrap();
        assert_eq!(
            db.get("host"),
            Some(&serde_json::Value::String("localhost".into()))
        );
        let creds = db.get("credentials").unwrap();
        assert_eq!(
            creds.get("user"),
            Some(&serde_json::Value::String("admin".into()))
        );
    }

    #[test]
    fn toml_layer_name() {
        let layer = TomlLayer::from_str("key = \"value\"").unwrap();
        assert_eq!(layer.name(), "toml");
    }

    #[test]
    fn env_layer_name() {
        let layer = EnvLayer::from_map(HashMap::new());
        assert_eq!(layer.name(), "env");
    }

    #[test]
    fn defaults_layer_name() {
        let layer = DefaultsLayer::new(serde_json::json!({"key": "value"}));
        assert_eq!(layer.name(), "defaults");
    }

    // ---- Deep merge edge cases ----

    #[test]
    fn deep_merge_empty_objects() {
        let base = serde_json::json!({});
        let override_val = serde_json::json!({});
        let merged = deep_merge(base, override_val);
        assert_eq!(merged, serde_json::json!({}));
    }

    #[test]
    fn deep_merge_three_levels() {
        let base = serde_json::json!({
            "a": { "b": { "c": 1, "d": 2 } }
        });
        let override_val = serde_json::json!({
            "a": { "b": { "d": 99, "e": 3 } }
        });
        let merged = deep_merge(base, override_val);
        let b = merged.get("a").unwrap().get("b").unwrap();
        assert_eq!(b.get("c"), Some(&serde_json::json!(1)));
        assert_eq!(b.get("d"), Some(&serde_json::json!(99)));
        assert_eq!(b.get("e"), Some(&serde_json::json!(3)));
    }

    // ---- ConfigStack get() tests ----

    #[test]
    fn config_stack_get_existing_key() {
        let stack = ConfigStack::new()
            .with_default("host", "localhost")
            .with_default("port", serde_json::json!(8080));
        assert_eq!(
            stack.get("host"),
            Some(serde_json::Value::String("localhost".into()))
        );
        assert_eq!(stack.get("port"), Some(serde_json::json!(8080)));
    }

    #[test]
    fn config_stack_get_missing_key() {
        let stack = ConfigStack::new()
            .with_default("host", "localhost");
        assert!(stack.get("missing").is_none());
    }

    #[test]
    fn config_stack_get_nested_key() {
        let stack = ConfigStack::new()
            .with_default("server.host", "localhost")
            .with_default("server.port", serde_json::json!(8080));
        assert_eq!(
            stack.get("server.host"),
            Some(serde_json::Value::String("localhost".into()))
        );
        assert_eq!(stack.get("server.port"), Some(serde_json::json!(8080)));
    }

    // ---- Secret additional tests ----

    #[test]
    fn secret_equality() {
        let s1 = Secret("secret1".to_string());
        let s2 = Secret("secret1".to_string());
        let s3 = Secret("secret2".to_string());
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn secret_clone() {
        let original = Secret("my-secret".to_string());
        let cloned = original.clone();
        assert_eq!(original.expose(), cloned.expose());
    }

    // ---- Empty / edge case stacks ----

    #[test]
    fn empty_stack_merge() {
        let merged = ConfigStack::new().merge().unwrap();
        assert_eq!(merged, serde_json::json!({}));
    }

    #[test]
    fn config_stack_default_trait() {
        let stack = ConfigStack::default();
        let merged = stack.merge().unwrap();
        assert_eq!(merged, serde_json::json!({}));
    }
}
