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
| [0008](0008-bug-history-flattened-change-records.md) | `bug history` JSON emits flattened change records with correlated `comment_id` | Accepted |
| [0009](0009-whoami-connection-metadata.md) | `whoami` JSON carries connection metadata via a flatten wrapper | Accepted |
| [0010](0010-uniform-fields-projection.md) | Uniform `--fields` projection is a generic serde-key filter, not the bug enum | Accepted |
| [0011](0011-progress-ndjson-stream.md) | Structured progress stream on stderr (`--progress ndjson`) | Accepted |
| [0012](0012-compound-create-report-not-rollback.md) | Compound `bug create` reports partial failure; it does not roll back | Accepted |
| [0013](0013-skills-installer-remote-fetch.md) | Agent-skills installer fetches its payload from a GitHub tarball, unverified | Superseded |
| [0014](0014-structured-error-detail-keys.md) | Structured per-variant detail keys on the `--json` error object | Accepted |
| [0015](0015-server-errors-are-never-masked.md) | A server error is never masked by an empty result | Accepted |
| [0016](0016-thread-local-error-redaction-context.md) | Thread-local error-redaction context | Accepted |
| [0017](0017-post-release-skill-version-update-stays-inline.md) | Post-release skill version updates stay inline | Accepted |
| [0018](0018-embed-canonical-skills-in-binary.md) | Embed canonical skills and retain standalone fetch | Accepted |
