# Design: plain-text tracing off a terminal, and a fresh comparison container

Issue [#722](https://github.com/randomparity/bzr/issues/722). Problem, decision, rejected
alternatives, and non-goals: [ADR 0058](../../adr/0058-suppress-tracing-ansi-off-terminal.md),
amending [ADR 0045](../../adr/0045-observe-comparison-transport-from-debug-events.md). Steps:
[the plan](../plans/2026-09-06-tracing-ansi-off-terminal.md). This document is the requirement
set and the proof map.

## Requirements

- **R1** — bzr's tracing subscriber writes no ANSI escape to stderr when stderr is not a
  terminal, at any verbosity and under any `RUST_LOG`. The `colored` stdout path is untouched.
- **R2** — `--no-color` is *extended* to the tracing stream; neither doc surface promised this
  before. Four colour-documentation rows are corrected with it — `docs/bzr-cli.md:50`, `:72`
  (`CLICOLOR`), `:74` (`CLICOLOR_FORCE`), and the clap doc comment at `src/cli/mod.rs:247-251`.
  `CLICOLOR_FORCE=1` does not in fact force stdout colour when stdout is redirected:
  `src/main.rs:36-37` sets `colored`'s manual override, and `colored` 3.1.1's
  `ShouldColorize::should_colorize` reads that override before `clicolor_force`.
- **R3, R4** — the tracing stream's ANSI decision is exactly the plan's truth table: on only when
  no `--no-color`, `NO_COLOR` unset or empty, and stderr a terminal. An explicit `with_ansi` call
  replaces `tracing-subscriber`'s own `NO_COLOR` default, so it must re-apply the rule.
- **R5** — `observe_bzr_transport` returns a transport for a real `bzr -vv` invocation whose
  stderr was redirected to a file, with no environment variable set by the caller.
- **R6** — `make functional-compare` and `make functional-compare-all` run against a container
  created for that run, or fail loudly. `setup-bugzilla.sh` `cmd_stop` discards `rm -f` failure,
  so `cmd_reset` must verify the container is gone before starting a new one.
- **R7** — The functional assertion bites on both halves independently: an ANSI regression turns
  it red, and a request-boundary event the harness regexes no longer match also turns it red.

## Proof map

| Req | Proof |
|---|---|
| R1 | `verbose-tracing-is-plain-on-redirected-stderr` (functional phase 17), plus the `stderr_is_terminal = false` unit rows |
| R2 | unit row `(true, None, true) -> false`; the four doc rows edited in plan Task 1 step 6 |
| R3, R4 | the remaining unit rows of the plan's truth table |
| R5 | the phase-17 test's `observe_bzr_transport` call |
| R6 | manual: `make functional-test` then `make functional-compare` in one checkout, no `NO_COLOR` set; the removal check is plan Task 3 step 1 |
| R7 | two separate controlled faults, plan Task 2 step 4. The test's `&&` short-circuits, so one fault cannot prove both halves. |

`make lint` and `make test` gate the unit half, `make functional-test` the functional half, and
`make functional-compare` the comparison half. The change is not security-relevant: it selects a
display attribute for a stream bzr already writes, adds no entry point, parses no foreign input,
touches no credential path, and changes no dependency. API-key redaction in verbose diagnostics
stays asserted by `verbose-response-body-diagnostics-redact-api-keys` in the same phase.
