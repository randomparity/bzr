# ADR-0001: clap-derived CLI types are crate-internal

Status: Accepted (2026-06-23)

## Context

The `bzr` library crate re-exports ~45 clap-derived action and argument types
from `src/cli/` as `pub` (`BugAction`, `ListArgs`, `QueryAction`, …). Desloppify
flags each as a future-proofing risk: as public API they are not marked
`#[non_exhaustive]`, so adding a field or enum variant is a breaking change for
external consumers.

`bzr` is published to crates.io, but it is a command-line *application*, not a
library intended to be depended upon. Its real, stable contract is the CLI
itself: argv → behavior → output, plus the config-file format. The Rust types
that model parsed arguments are an implementation detail of that contract.

Surveying actual consumers of `bzr::cli::*`:

- `src/main.rs` uses only `Cli::parse()`, the `Cli` flag fields, and
  `dispatch(&cli, …)`. It never constructs an action type.
- `xtask` uses `<bzr::cli::Cli as CommandFactory>::command()` to generate
  manpages. It needs only `Cli`.
- `tests/integration.rs` is the sole place that constructs action/argument types
  directly — and it already has helpers (`dispatch_cli*`) that drive the public
  `Cli` + `dispatch` seam instead.

The rest of the crate (`error.rs`, `types/`, `config/model.rs`, `client/`)
already marks its genuinely public domain types `pub` + `#[non_exhaustive]`.
Those types model external Bugzilla data and have a real reason to be a stable,
evolvable surface. The CLI parse types do not.

## Decision

Treat the clap-derived CLI action and argument types as **crate-internal**.
Reduce their visibility from `pub` to `pub(crate)`. Keep public only:

- `cli::Cli` (and the flag fields `main.rs` reads),
- `dispatch(&Cli, OutputFormat, &mut Writers)`.

By E0446, `cli::Commands` and the `Cli.command` field also drop to `pub(crate)`,
because a public enum/struct may not expose a crate-private type. The
`src/cli/mod.rs` re-export blocks change from `pub use` to `pub(crate) use`.

Integration tests that constructed action types migrate to the existing
`dispatch_cli*` helpers; binary-crate tests that hand-built a `Cli` migrate to
parse-based construction.

## Consequences

- The desloppify finding is resolved at the source: there is no longer a public
  CLI-type surface to future-proof, so `#[non_exhaustive]` is unnecessary.
- The library's public API shrinks to the minimal honest seam (`Cli` +
  `dispatch`), matching how `bzr` is actually used.
- Integration tests now exercise the full argv → context → dispatch path —
  strictly more coverage than the previous direct `execute(action)` calls.
- Adding or removing CLI flags/variants is no longer a semver-breaking change to
  any external consumer, because none can name those types.
- Cost: a one-time migration of ~56 integration tests and the binary-crate test
  helpers. No production behavior changes.

## Considered & rejected

### Mark all CLI types `pub` + `#[non_exhaustive]`

Keep the types public but declare them non-exhaustive, consistent with the
domain types elsewhere in the crate.

Rejected because:

- It commits to a *stable public library API* that `bzr`, as a CLI application,
  has no intention of supporting or versioning — speculative surface the project
  philosophy explicitly avoids.
- It does not actually let integration tests keep hand-constructing the types:
  `#[non_exhaustive]` blocks external struct-literal construction just as
  `pub(crate)` does, so the test migration cost is identical. Given equal test
  cost, the weaker (still-public) stance buys nothing.
- It leaves a public surface whose only real consumer is the crate's own tests.

### Leave the types `pub` as-is

Rejected: this is the status quo the issue exists to fix; it leaves every CLI
type as an unversioned breaking-change hazard and the desloppify finding open.

### Move the construct-and-execute tests in-crate instead of migrating to dispatch

Keep `pub(crate)` types but relocate the integration tests into the crate so
they can still construct actions directly.

Rejected: `CLAUDE.md` mandates integration tests live in `tests/integration.rs`
and is enforced by `make check-test-layout`; bulk-moving ~3000 lines in-crate
violates that layout and is higher-churn than reusing the existing `dispatch_cli`
helpers, which also yields better coverage.
