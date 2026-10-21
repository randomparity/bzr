# ADR 0084: Keep insecure-warning acknowledgment in connection policy

## Status
Accepted (2026-10-21), for issue #921 under scope q921-5fd4c1a1.

## Context
An intentionally insecure saved server repeats a diagnostic after its operator
has acknowledged the condition. Suppression must not change transport trust.

## Decision
Persist a false-default `tls_insecure_acknowledged` boolean on saved servers.
Require insecure mode when true. Carry it on the resolved connection target;
only the existing disabled-verification warning tests it. Inline targets use false.
The set-server flag participates in its existing full replacement semantics;
omitting it revokes acknowledgment. Agents require explicit user authorization.

## Consequences
Old config keeps warning. Editing the boolean preserves other settings, while
set-server replacements must resupply them. Old binaries safely keep warning.
No output projection or TLS verifier field changes. ADR index row is campaign-owned.

## Considered & rejected
- **Put acknowledgment in TlsConfig.** judgment: warning policy should not enter
  the transport trust object when only connection orchestration consumes it.
- **Allow true while verification is enabled.** judgment: latent acknowledgment
  could silently suppress a later insecure configuration; reject the combination.
- **Do nothing.** judgment: leaves the repeated warning that issue #921 requests
  operators be able to acknowledge.
