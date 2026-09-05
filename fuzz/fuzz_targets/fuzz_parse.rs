#![no_main]

use envstack::layers::{TomlLayer, YamlLayer};
use envstack::ConfigStack;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Bound input so parse attempts stay fast.
    let s = String::from_utf8_lossy(&data[..data.len().min(64 * 1024)]);

    // Malformed TOML/YAML must return Err, never panic.
    let _ = TomlLayer::from_str(&s);
    let _ = YamlLayer::from_str(&s);

    // Through the stack builder: parse, then typed extraction of the merged
    // layers — every step is Result and must not panic on adversarial input.
    if let Ok(stack) = ConfigStack::new().with_toml_str(&s) {
        let _ = stack.extract::<serde_json::Value>();
    }
    if let Ok(stack) = ConfigStack::new().with_yaml_str(&s) {
        let _ = stack.extract::<serde_json::Value>();
    }

    // TOML layer whose parsed JSON is then treated as defaults must not panic.
    if let Ok(layer) = TomlLayer::from_str(&s) {
        let stack = ConfigStack::new().with_layer(layer);
        let _ = stack.merge();
    }
    if let Ok(layer) = YamlLayer::from_str(&s) {
        let stack = ConfigStack::new().with_layer(layer);
        let _ = stack.merge();
    }
});
