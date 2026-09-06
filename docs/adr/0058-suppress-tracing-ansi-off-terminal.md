# ADR 0058: Plain-text tracing off a terminal, and a comparison container per run

## Status

Accepted

## Context

ADR 0045 derives each comparison operation's transport from bzr's request-boundary debug events,
matched with an `^`-anchored regex over the captured stderr file (`tests/functional/lib.sh:269`).
That contract assumed the captured bytes are plain text. They were not: `src/main.rs:29-32` built
the `tracing_subscriber::fmt()` subscriber without calling `with_ansi`, and `tracing-subscriber`
0.3.23 defaults ANSI on unless `NO_COLOR` holds a non-empty value (`fmt_layer.rs:743`), with no
terminal detection. `observe_bzr_transport` therefore matched nothing and every comparison
capability failed as "transport observation is missing" unless the caller exported `NO_COLOR=1`,
which nothing in the repository does. The escapes are not a leading prefix only — a record wraps
the target name and its trailing colon separately
(`ESC[2mbzr::client::transportESC[0mESC[2m:ESC[0m`) — so a harness-side fix must strip escapes
from the whole line rather than relax an anchor. Meanwhile `src/main.rs:36-37` already disables
the `colored` override when stdout is not a terminal; the two streams disagreed.

The comparison tier carries a second undocumented precondition: it must run against a container
created for that run. `setup-bugzilla.sh` `cmd_start` reuses a running container by design
("Container is already running"), so `make functional-compare` inherits whatever server state an
earlier `make functional-test` left behind.

## Decision

bzr enables ANSI on its tracing stream only when `--no-color` is absent, `NO_COLOR` is unset or
empty, and stderr is a terminal. `src/main.rs` passes that to `with_ansi`, computed by the pure
helper `tracing_ansi_enabled` and unit-tested in `src/main_tests.rs`. The helper re-applies the
`NO_COLOR` rule itself, because an explicit `with_ansi` call replaces `tracing-subscriber`'s
default outright; a decision built only from `is_terminal()` would silently withdraw the
`NO_COLOR` support the crate was providing. `CLICOLOR` and `CLICOLOR_FORCE` stay confined to the
stdout `colored` path, and `--no-color`'s clap help and `docs/bzr-cli.md` row are corrected to
say so.

ADR 0045's observation contract is unchanged; this record supplies the production-side property
it depended on and did not state, and 0045 carries a banner pointing here.

`make functional-compare` and `tests/functional/run-compare-all.sh` recreate the container
through the existing `setup-bugzilla.sh reset`. This is a defensive precondition, not a
characterised coupling: issue #722 reports the tier failing as `saved-search precondition failed`
after a `functional-test` run, but that probe queries `bug_id=<id1>,<id2>&bug_id_type=anyexact`
over two bugs the same run created (`tests/functional/run-compare.sh:50`,
`tests/functional/compare/01-bug-lifecycle.sh:474-484`), which a grown corpus does not perturb.
A `reset` removes the whole variable for the price of a container rebuild, and the tier's
contract is to compare against a server the run set up.

## Consequences

- `bzr -vv 2> file` now writes plain tracing output — a user-visible change for anyone capturing
  the escapes deliberately. A terminal, or a pty wrapper such as `script`, restores them.
- `--no-color` now covers both streams. `NO_COLOR` keeps working and is no longer load-bearing
  for the comparison tier, which reads production tracing with no interposed transformation, so a
  change to a boundary message still fails the tier loudly as ADR 0045 requires.
- `make functional-compare` destroys any container a developer was holding for
  `make functional-test`, and the recreated container publishes on a new host port. CI starts
  each comparison job cold, so nothing there changes.
- `cmd_reset` gains a removal check. `cmd_stop` discards `rm -f` failure (`2>/dev/null || true`)
  and logs "Container removed." regardless, and podman refuses to remove a container a running
  one depends on through `--network container:<name>` — which is what a leftover python-bugzilla
  sidecar is (`tests/functional/lib.sh:406`) after a compare run whose EXIT trap did not fire. An
  unchecked `reset` would then be exactly `start`, silently. It now fails with a diagnostic
  instead; `stop` keeps its permissive behaviour for its other callers.
- The container-freshness ground is an observation, not a mechanism, and the cost above is
  accepted on that basis deliberately: charter criterion 3 permits the cheaper documented branch,
  and this record takes the dearer one because an entry point that silently inherits arbitrary
  server state is the same class of latent defect as the one being fixed. If the tier is later
  seen to pass reliably against a dirty container, this half is safe to withdraw on its own.
- The two streams disagree on an empty `NO_COLOR`. `colored` 3.1.1 disables stdout colour for any
  present value unless `CLICOLOR_FORCE=1` outranks it (`src/control.rs:102`: "`CLICOLOR_FORCE`
  takes highest priority, followed by `NO_COLOR`"), while the tracing stream follows the crate
  default and no-color.org in treating an empty value as unset and honours no forcing variable. `NO_COLOR=` on a terminal therefore leaves stdout plain and stderr coloured. The
  tracing side keeps the conventional reading rather than matching `colored`, and the reference
  documents the divergence per stream instead of claiming one rule for both.

## Considered & rejected

### Suppressing the escapes

- **Strip ANSI in the harness before matching.** verified: a `bzr -vv` capture at `e64cc06a`
  shows escapes inside the record, not only leading, so the anchored-prefix relaxation issue #722
  proposed does not match and a whole-line filter is required. judgment: that filter is the
  transformation between bzr and the assertion that ADR 0045 chose production tracing to avoid.
- **Export `NO_COLOR=1` from the compare runner.** judgment: it encodes the workaround instead of
  removing it, and leaves every other reader of bzr's stderr holding escapes it did not ask for.
- **Do nothing and document `NO_COLOR=1` as a precondition.** verified:
  `.github/workflows/functional-tests.yml:82` runs `make functional-compare-all` with no such
  variable, and `NO_COLOR` appears nowhere under `.github/`, `Makefile`, or `tests/functional/`;
  documentation does not reach a workflow file.
- **Derive it from the existing `colored` override.** verified: `src/main.rs:36-37` sets that
  override from `std::io::stdout().is_terminal()`, so `bzr -vv 2> file` from a terminal would
  still write escapes into the file — the exact failure being fixed.

### Making the container fresh

- **Do nothing.** verified: the symptom issue #722 attributed to a dirty container is otherwise
  explained — the saved-search probe queries two ids the same run created — so no reported failure
  is known to require this half. judgment: the tier's contract is nonetheless to compare against a
  server the run set up, and leaving an entry point that silently inherits arbitrary state is a
  latent version of exactly the class of defect this issue is.
- **Document the precondition instead of enforcing it.** verified: charter criterion 3 permits it,
  and it is the cheaper branch — no container rebuild, no destroyed developer container.
  judgment: a `##` help line is read by `make help`, not by the runner typing
  `make functional-compare`, so it is the branch that was already effectively taken and already
  forgotten.
- **Enforce it behind a flag or a separate `functional-compare-fresh` target.** judgment: the
  default would stay the unsafe one, which is how this was missed; a developer iterating on one
  comparison failure can still call `run-compare.sh` directly.
- **Detect a dirty container and fail with a diagnostic.** judgment: with the coupling
  uncharacterised there is no property to detect, so the check would be a proxy that goes stale
  unnoticed.
- **Give the comparison tier its own container name.** verified: `bugzilla_container_name` is
  already checkout-scoped (ADR 0030), so a tier suffix would let compare create and destroy a
  container of its own and never touch the one `functional-test` is using — a stronger guarantee
  than `reset`. judgment: it costs a second Bugzilla instance per host and a second image
  lifecycle to reason about, which is more than an uncharacterised precondition is worth. It is
  the right answer if the destructive consequence above proves painful.
