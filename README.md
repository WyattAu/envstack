# envstack

Layered configuration for Rust — environment variables, TOML files, and CLI args with type-safe extraction and validation.

## Features

- **Layered merging** — environment vars override TOML, which override defaults
- **Type-safe extraction** — deserialize directly into your config struct
- **Multiple sources** — env vars, TOML files, CLI args (with `clap` feature)
- **Validation** — opt-in `validator` integration
- **Zero boilerplate** — no manual `env::var` calls scattered everywhere

## Quick Start

```rust
use envstack::ConfigStack;

#[derive(serde::Deserialize)]
struct AppConfig {
    host: String,
    port: u16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: AppConfig = ConfigStack::new()
        .with_env()
        .with_toml_file("config.toml")
        .with_default("host", "localhost")
        .with_default("port", "8080")
        .extract()?;

    println!("Listening on {}:{}", config.host, config.port);
    Ok(())
}
```

## Comparison with figment / dotenvy

| | figment | envstack | dotenvy only |
|---|---|---|---|
| Env vars | yes | yes | yes |
| TOML files | yes | yes | no |
| CLI args | via adapter | via `clap` feature | no |
| Validation | via adapter | built-in (`validate`) | no |
| Weight | heavier | lighter | lightest |

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
