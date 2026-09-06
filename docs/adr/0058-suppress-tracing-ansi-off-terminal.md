# ADR 0058: Suppress tracing ANSI when stderr is not a terminal

## Status

Accepted

## Context

ADR 0045 derives each comparison operation's transport from bzr's own request-boundary debug
events, matched with an `^`-anchored regex over the captured stderr file. That contract assumed
the captured bytes are plain text.

They were not. `src/main.rs` built the `tracing_subscriber::fmt()` subscriber without calling
`with_ansi`, and `tracing-subscriber` 0.3.23 defaults ANSI on unless `NO_COLOR` holds a non-empty
value (`fmt_layer.rs:743`), with no terminal detection at all. Every record began with an escape
even when stderr was a file, so `observe_bzr_transport` matched nothing and every comparison
capability failed as "transport observation is missing" unless the caller happened to export
`NO_COLOR=1`. An undocumented environment variable stood in for the contract, and the scheduled
comparison job sets none.

The escapes are not confined to a leading prefix — a captured record wraps the target name and
its trailing colon separately (`ESC[2mbzr::client::transportESC[0mESC[2m:ESC[0m`), so a
harness-side fix must strip escapes from the whole line rather than relax an anchor.

bzr already treats the other stream this way: `src/main.rs` disables the `colored` override when
stdout is not a terminal. The two streams disagreed, and `--no-color`, documented as "Disable
colored output", reached only stdout.

## Decision

bzr enables ANSI on its tracing stream only when `--no-color` is absent, `NO_COLOR` is unset or
empty, and stderr is a terminal. `src/main.rs` passes that to `with_ansi`, computed by the pure
helper `tracing_ansi_enabled` and unit-tested in `src/main_tests.rs`.

The helper re-applies the `NO_COLOR` rule itself, because an explicit `with_ansi` call replaces
`tracing-subscriber`'s default outright; a decision built only from `is_terminal()` would
silently withdraw the `NO_COLOR` support the crate was providing. `CLICOLOR` and `CLICOLOR_FORCE`
stay confined to the stdout `colored` path.

ADR 0045's observation contract is unchanged and stands as written; this record supplies the
production-side property it depended on and did not state, and 0045 carries a banner pointing
here.

The tier's other undocumented precondition is enforced rather than documented in the same change:
`make functional-compare` and `tests/functional/run-compare-all.sh` recreate the container
through the existing `setup-bugzilla.sh reset` instead of reusing whatever `start` finds running.
The tier asserts exact result sets — the saved-search precondition matches exactly two bug ids —
so a corpus grown by an earlier `make functional-test` fails it on data rather than behaviour.

## Consequences

- `bzr -vv 2> file` now writes plain text. This is a user-visible change for anyone capturing the
  escapes deliberately; a terminal, or a pty wrapper such as `script`, restores them.
- `--no-color` now covers both streams, which its own help text already claimed.
- `NO_COLOR=1` keeps working and is no longer load-bearing for the comparison tier.
- The harness reads production tracing with no interposed transformation, so a change to a
  boundary message still fails the tier loudly, as ADR 0045 requires.
- `make functional-compare` pays a container recreation on every run. That cost buys a
  precondition a runner cannot forget.

## Considered & rejected

- **Strip ANSI in the harness before matching.** verified: a `bzr -vv` capture at `e64cc06a`
  shows escapes inside the record (`ESC[2mbzr::client::transportESC[0mESC[2m:ESC[0m`), so the
  anchored-prefix relaxation the issue proposed does not match and a whole-line filter is
  required. judgment: that filter is a transformation between bzr and the assertion, the coupling
  ADR 0045 chose production tracing to avoid.
- **Export `NO_COLOR=1` from the compare runner.** judgment: it encodes the workaround instead of
  removing it, and leaves every other reader of bzr's stderr — a CI log, a pasted bug report, a
  `tee` — holding escapes it did not ask for.
- **Do nothing and document `NO_COLOR=1` as a precondition.** verified:
  `.github/workflows/functional-tests.yml` runs `make functional-compare-all` with no such
  variable, so the tier stays red for every commit; documentation does not reach a workflow file.
- **Derive the tracing decision from the existing `colored` override.** verified: `src/main.rs`
  sets that override from `std::io::stdout().is_terminal()`, so `bzr -vv 2> file` run from a
  terminal would still write escapes into the file — the exact failure being fixed.
- **Honour `CLICOLOR_FORCE=1` on the tracing stream too.** judgment: a forced-colour variable in
  a runner's environment would restore this defect through a different door.
- **Add a test-only flag that turns bzr's tracing plain.** verified: ADR 0045 already rejected
  expanding production build surface for the harness's benefit, on the ground that existing
  sanitized request-boundary events expose the needed fact.
