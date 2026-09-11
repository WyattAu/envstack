# Requirements — envstack

Numbered, testable requirements. Every requirement maps to at least one named
test or doc-comment contract; security-relevant items cite threat-model rows.

Scope: Layered configuration (`envstack`) — env vars over TOML files over CLI args with type-safe extraction

## Functional

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-ES-001 | Precedence is CLI > env > file > defaults, applied in documented order | MUST |
| REQ-ES-002 | Type errors report the layer and key that failed | MUST |
| REQ-ES-003 | Missing required keys return a typed error listing the key | MUST |

## Security

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-ES-100 | Secret-valued keys are never logged by the loader | MUST |
| REQ-ES-101 | File parsing bounds input size (TOML parser limits) | SHOULD |

## Observability & API hygiene

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-ES-900 | All fallible public APIs return typed errors; production `unwrap`/`expect` is denied or explicitly justified with an invariant comment | MUST |
| REQ-ES-901 | Public items carry doc comments with runnable examples where practical | SHOULD |
