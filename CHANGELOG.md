# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/): versions follow [semver](https://semver.org).

## [1.0.0] - 2026-10-26

### Declared stable

First 1.0. `envstack` layers production configuration; this release commits the env, TOML, and CLI-arg extraction surfaces and the type-safe accessors as API-stable.

### Guarantees from here on

- Breaking changes to public items require a minor bump plus a `#[deprecated]`
  window of at least one minor release.
- MSRV bumps are minor releases, not patch releases.
- `#![deny(missing_docs)]` and `#![forbid(unsafe_code)]` are enforced.
- Layer precedence (env, then TOML, then CLI args) will never change.

### Explicitly not guaranteed

- New source types may arrive in minors behind features; existing layer precedence will not change.

## [Unreleased]

## [0.2.1] - 2026-09-12

### Added

- config-knob behavior matrix: tests/config_matrix.rs closes the ConfigStack coverage gaps — with_env surfaces the live process env, with_env_prefix filters via the stack, with_env_map overrides defaults (layer-order knob), strict TOML/YAML success paths override defaults, with_yaml_str, with_dotenv_map, with_dotenv_from_cwd error-surfacing contract, and the validate knob blocks bad / allows good config.


## [0.2.0] - 2026-09-01

### Added
- Layered configuration — environment variables, TOML files, CLI overrides.
- Published to crates.io (2026-09-01).
