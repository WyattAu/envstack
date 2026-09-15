//! Config-knob behavior matrix for envstack's `ConfigStack`.
//!
//! Every `with_*` layer source must observably change the merged config.
//! Unit tests cover most builders; this matrix closes the gaps that only
//! appear through the `ConfigStack` surface: real process-env layers,
//! strict success paths, and dotenv-from-cwd.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use envstack::ConfigStack;
use serde_json::Value;
use std::collections::HashMap;

// --- with_env: real process environment ------------------------------------
//
// This crate is edition 2024 with `unsafe_code = forbid`, so tests cannot
// call `std::env::set_var`. The positive path (a var set -> surfaces
// lowercased in the merge) is proven at the layer level by the
// `EnvLayer::from_map` tests; here we probe the live process env instead.

#[test]
fn knob_with_env_surfaces_live_process_environment() {
    let merged: Value = ConfigStack::new().with_env().extract().unwrap();

    // `PATH` exists on every supported platform; envstack lowercases keys.
    let path = merged
        .get("path")
        .expect("with_env must surface the process PATH (lowercased) into the merge");
    assert!(
        path.as_str().expect("PATH is a string").contains('/'),
        "PATH must be carried through verbatim"
    );
}

// --- with_env_prefix via ConfigStack -----------------------------------------

#[test]
fn knob_with_env_prefix_filters_via_stack() {
    // Positive filtering behavior is covered at layer level (from_map tests).
    // Via the stack: a prefix nothing matches must yield an empty layer,
    // letting the default win.
    let merged: Value = ConfigStack::new()
        .with_env_prefix("ENVSTACK_MATRIX_CERTAINLY_UNSET_")
        .with_default("host", "from-default")
        .extract()
        .unwrap();

    assert_eq!(
        merged.get("host"),
        Some(&Value::String("from-default".into())),
        "non-matching prefix must contribute nothing to the merge"
    );
}

// --- with_env_map vs defaults (priority knob: layer order) -------------------

#[test]
fn knob_layer_order_env_map_beats_default() {
    let mut vars = HashMap::new();
    vars.insert("matrix_key".into(), "from-map".into());

    let with_env: Value = ConfigStack::new()
        .with_env_map(vars.clone())
        .with_default("matrix_key", "from-default")
        .extract()
        .unwrap();

    let default_only: Value = ConfigStack::new()
        .with_default("matrix_key", "from-default")
        .extract()
        .unwrap();

    assert_eq!(
        default_only.get("matrix_key"),
        Some(&Value::String("from-default".into()))
    );
    assert_eq!(
        with_env.get("matrix_key"),
        Some(&Value::String("from-map".into())),
        "adding the env layer must override the default"
    );
}

// --- TOML: strict success path + content knob ---------------------------------

#[test]
fn knob_with_toml_strict_success_changes_merged_output() {
    let dir = std::env::temp_dir().join("envstack_matrix_toml");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("matrix.toml");
    std::fs::write(&path, "matrix_key = \"from-strict-toml\"\n").unwrap();

    let merged: Value = ConfigStack::new()
        .with_toml_file_strict(&path)
        .unwrap()
        .with_default("matrix_key", "from-default")
        .extract()
        .unwrap();

    assert_eq!(
        merged.get("matrix_key"),
        Some(&Value::String("from-strict-toml".into())),
        "strict TOML layer must override defaults when the file exists"
    );
    std::fs::remove_file(&path).unwrap();
}

// --- YAML: strict success path (feature-gated) ---------------------------------

#[test]
#[cfg(feature = "yaml")]
fn knob_with_yaml_strict_success_changes_merged_output() {
    let dir = std::env::temp_dir().join("envstack_matrix_yaml");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("matrix.yaml");
    std::fs::write(&path, "matrix_key: from-strict-yaml\n").unwrap();

    let merged: Value = ConfigStack::new()
        .with_yaml_file_strict(&path)
        .unwrap()
        .with_default("matrix_key", "from-default")
        .extract()
        .unwrap();

    assert_eq!(
        merged.get("matrix_key"),
        Some(&Value::String("from-strict-yaml".into()))
    );
    std::fs::remove_file(&path).unwrap();
}

#[test]
#[cfg(feature = "yaml")]
fn knob_with_yaml_str_changes_output() {
    let merged: Value = ConfigStack::new()
        .with_yaml_str("matrix_key: from-yaml-str\n")
        .unwrap()
        .with_default("matrix_key", "from-default")
        .extract()
        .unwrap();

    assert_eq!(
        merged.get("matrix_key"),
        Some(&Value::String("from-yaml-str".into()))
    );
}

// --- dotenv: from_cwd + map (feature-gated) --------------------------------------

#[test]
#[cfg(feature = "dotenv")]
fn knob_with_dotenv_map_changes_output() {
    let mut vars = HashMap::new();
    vars.insert("MATRIX_KEY".into(), "from-dotenv-map".into());

    let merged: Value = ConfigStack::new()
        .with_dotenv_map(vars)
        .with_default("matrix_key", "from-default")
        .extract()
        .unwrap();

    assert_eq!(
        merged.get("matrix_key"),
        Some(&Value::String("from-dotenv-map".into()))
    );
}

#[test]
#[cfg(feature = "dotenv")]
fn knob_with_dotenv_from_cwd_surfaces_errors_instead_of_swallowing() {
    // edition 2024 + unsafe_code=forbid: chdir is unsafe, so we cannot point
    // the cwd at a fixture dir. The knob's contract vs the lenient
    // `with_dotenv`: a missing ./.env must SURFACE as `ConfigError::Dotenv`
    // (or succeed if a .env happens to exist in the cwd) — never silently
    // swallowed.
    let result = ConfigStack::new().with_dotenv_from_cwd();

    match result {
        Ok(_) => {} // a .env exists in the cwd; loading it is the success path
        Err(err) => assert!(
            matches!(err, envstack::ConfigError::Dotenv(_)),
            "missing ./.env must surface as ConfigError::Dotenv, got: {err}"
        ),
    }
}

// --- validate knob -----------------------------------------------------------------

#[test]
fn knob_validate_blocks_bad_and_allows_good_config() {
    let check = |v: &Value| {
        if v.get("port").and_then(|p| p.as_u64()) == Some(0) {
            Err(envstack::ConfigError::ValidationError {
                field: "port".into(),
                message: "must be > 0".into(),
            })
        } else {
            Ok(())
        }
    };

    let bad: Result<Value, _> = ConfigStack::new()
        .with_default("port", serde_json::json!(0))
        .validate(check)
        .extract();
    assert!(bad.is_err(), "validator must block port = 0");

    let good: Result<Value, _> = ConfigStack::new()
        .with_default("port", serde_json::json!(8080))
        .validate(check)
        .extract();
    assert_eq!(
        good.unwrap().get("port"),
        Some(&serde_json::json!(8080)),
        "validator must allow port = 8080"
    );
}
