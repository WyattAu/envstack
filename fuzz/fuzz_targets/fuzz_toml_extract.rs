#![no_main]

use libfuzzer_sys::fuzz_target;
use envstack::ConfigStack;
use serde::Deserialize;

/// Minimal config struct for fuzzing TOML extraction.
#[derive(Deserialize, Debug, Default)]
#[allow(dead_code)]
struct FuzzConfig {
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: serde_json::Value,
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Fuzz ConfigStack extraction with arbitrary TOML strings.
        // Must not panic on any input — errors are expected.
        let stack = match ConfigStack::new().with_toml_str(s) {
            Ok(stack) => stack,
            Err(_) => return,
        };

        let _ = stack.extract::<FuzzConfig>();
    }
});
