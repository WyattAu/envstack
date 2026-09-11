# Threat Model — envstack

Reference: STRIDE. Scope: the crate's public API surface. Trust boundary:
(1) bytes/inputs entering public constructors and parsers, (2) concurrent
callers sharing interior state. envstack is an in-process library — it opens
no sockets and inherits the embedding process's trust domain.

Purpose: Layered configuration (`envstack`) — env vars over TOML files over CLI args with type-safe extraction

## Assets

| ID | Asset | Exposed via |
|----|-------|-------------|
| A1 | configuration integrity (no silent wrong-layer value) | hostile input, concurrent callers |
| A2 | secret confidentiality in logs | hostile input, concurrent callers |

## STRIDE Analysis

| # | Threat | Category | Surface | Mitigation | Residual risk |
|---|--------|----------|---------|------------|---------------|
| T1 | Env var injection overrides file intent | Tampering | `precedence` | precedence is explicit and documented; conflicts are observable via the typed extraction API | documented |
| T2 | Secrets leaked through error messages | Info disclosure | `error rendering` | errors carry keys, never values | documented |
| T3 | Hostile TOML file causes resource exhaustion | DoS | `file layer` | TOML parser bounds + documented input expectations | documented |

## Repudiation

The crate keeps no audit trail; attribution of calls to callers is out of
scope for an in-process library.

## Out of Scope

- Network transport security (the crate never opens sockets).
- Storage-host compromise: an attacker who controls the host can bypass all
  in-process mitigations.
- Denial of service via resource exhaustion of the host process beyond the
  bounds enforced above.

Reviewed: 2026-09-11
