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
| [0019](0019-quiet-by-default-unit-test-output.md) | `make test` runs quiet by default; `VERBOSE=1`/`test-verbose` restore full output | Accepted |
| [0020](0020-pages-build-runs-on-pull-requests.md) | Pages builds run on pull requests; deployment remains gated | Accepted |
| [0021](0021-contributor-guidance-lives-in-contributing.md) | Contributor guidance lives in `CONTRIBUTING.md` | Accepted |
| [0022](0022-release-notes-carry-security-assessment.md) | Release notes carry an explicit security assessment | Accepted |
| [0023](0023-skill-owned-weekly-status-snapshots.md) | Skill-owned weekly status snapshots | Accepted |
| [0024](0024-bounded-bug-adjacency-contract.md) | Bound multi-bug adjacency at the CLI request boundary | Accepted |
| [0025](0025-normalize-multi-valued-bug-fields.md) | Normalize multi-valued bug fields to arrays | Accepted |
| [0026](0026-scope-qualified-dependency-preflight.md) | Scope-qualify dependency collector preflight | Accepted |
| [0027](0027-inline-server-aware-url-import.md) | Make URL import aware of the active inline server | Accepted |
| [0028](0028-signed-metadata-sort-keys.md) | Model Bugzilla metadata sort keys as signed integers | Accepted |
| [0029](0029-semantic-functional-test-ids.md) | Identify functional tests by phase and semantic slug | Accepted |
| [0030](0030-dynamic-functional-test-container-ports.md) | Functional-test containers use runtime-assigned ports and checkout-scoped names | Accepted |
| [0031](0031-compose-dependency-presentation-artifacts.md) | Compose dependency presentation artifacts through active capabilities | Accepted |
| [0032](0032-operation-scoped-search-transport.md) | Resolve search transport once per operation | Accepted |
| [0033](0033-share-lenient-deserialization-adapters.md) | Share lenient deserialization adapters by wire contract | Accepted |
| [0043](0043-stabilize-test-tracing-callsite-interest.md) | Keep a sentinel tracing dispatch alive in tests | Accepted |
| [0044](0044-python-bugzilla-comparison-sidecar.md) | Run python-bugzilla as a network-namespace sidecar | Accepted |
| [0045](0045-observe-comparison-transport-from-debug-events.md) | Observe comparison transport from request-boundary debug events | Accepted |
| [0046](0046-share-python-bugzilla-comparison-adapter.md) | Share one fixed python-bugzilla comparison adapter | Superseded |
| [0047](0047-centralize-terminal-width-table-wrapping.md) | Centralize terminal-width table wrapping | Accepted |
| [0048](0048-comment-tags-are-always-present-arrays.md) | Comment tags are always-present arrays | Accepted |
| [0049](0049-comment-list-client-side-multi-id-loop.md) | Fetch multi-ID comment sets with a client-side loop | Accepted |
| [0050](0050-run-comparison-proxies-in-sidecar-namespace.md) | Run comparison proxies in the sidecar namespace | Accepted |
| [0051](0051-share-adapter-with-bounded-local-proofs.md) | Share one adapter with bounded local proofs | Accepted |
