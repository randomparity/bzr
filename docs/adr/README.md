# Architecture Decision Records

This directory records architecture decisions for `bzr` — choices with viable
alternatives where the rationale is worth preserving. Each ADR captures the
context, the decision, its consequences, and the alternatives considered and
rejected.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-cli-types-are-crate-internal.md) | clap-derived CLI types are crate-internal | Accepted |
| [0002](0002-test-config-isolation-over-env-lock.md) | Per-test config-path isolation replaces the shared env lock | Accepted |
| [0003](0003-admin-mutation-seam-excludes-component-update.md) | Admin create/update seam is linear-only and excludes component update | Accepted |
| [0004](0004-client-and-connection-module-boundaries.md) | Client and connection orchestration module boundaries | Accepted |
| [0005](0005-server-capabilities-contract.md) | `server capabilities` reports the anonymously-derivable surface | Accepted |
| [0006](0006-bug-links-isolated-fetch.md) | `bug links` uses an isolated relationship fetch | Accepted |
| [0007](0007-json-output-schema-version-envelope.md) | `--json` output is wrapped in a versioned envelope | Accepted |
