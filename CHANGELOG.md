# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/) — versions follow [semver](https://semver.org).

## [Unreleased]

## [0.2.1] - 2026-09-12

### Added

- config-knob behavior matrix: tests/config_matrix.rs closes the ConfigStack coverage gaps — with_env surfaces the live process env, with_env_prefix filters via the stack, with_env_map overrides defaults (layer-order knob), strict TOML/YAML success paths override defaults, with_yaml_str, with_dotenv_map, with_dotenv_from_cwd error-surfacing contract, and the validate knob blocks bad / allows good config.


## [0.2.0] - 2026-09-01

### Added
- Layered configuration — environment variables, TOML files, CLI overrides.
- Published to crates.io (2026-09-01).
