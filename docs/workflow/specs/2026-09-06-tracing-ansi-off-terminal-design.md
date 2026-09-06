# Design: plain-text tracing off a terminal, and a fresh comparison container

Issue [#722](https://github.com/randomparity/bzr/issues/722). Problem, decision, rejected
alternatives, and non-goals: [ADR 0058](../../adr/0058-suppress-tracing-ansi-off-terminal.md),
amending [ADR 0045](../../adr/0045-observe-comparison-transport-from-debug-events.md). Steps:
[the plan](../plans/2026-09-06-tracing-ansi-off-terminal.md). This document is the requirement
set and the proof map.

## Requirements

- **R1** — bzr's tracing subscriber writes no ANSI escape to stderr when stderr is not a
  terminal, at any verbosity and under any `RUST_LOG`. The `colored` stdout path is untouched.
- **R2** — `--no-color` is *extended* to the tracing stream. Neither `docs/bzr-cli.md:50` nor the
  clap doc comment at `src/cli/mod.rs:247-251` promised this before; both are corrected to say
  that each stream's automatic suppression follows its own terminal status and that
  `CLICOLOR` / `CLICOLOR_FORCE` govern stdout colour only.
- **R3** — A non-empty `NO_COLOR` still suppresses tracing ANSI on a terminal. An explicit
  `with_ansi` call replaces `tracing-subscriber`'s own `NO_COLOR` default, so the decision must
  re-apply the rule rather than inherit it.
- **R4** — On a terminal with no `--no-color` and `NO_COLOR` unset or empty, ANSI stays on.
- **R5** — `observe_bzr_transport` returns a transport for a real `bzr -vv` invocation whose
  stderr was redirected to a file, with no environment variable set by the caller.
- **R6** — `make functional-compare` and `make functional-compare-all` run against a container
  created for that run.
- **R7** — The functional assertion bites on both halves independently: an ANSI regression turns
  it red, and a request-boundary event the harness regexes no longer match also turns it red.

## Proof map

| Req | Proof |
|---|---|
| R1 | `verbose-tracing-is-plain-on-redirected-stderr` (functional phase 17), plus the `stderr_is_terminal = false` unit rows |
| R2 | unit row `(true, None, true) -> false`; both doc surfaces edited in plan Task 1 step 6 |
| R3 | unit row `(false, Some("1"), true) -> false` |
| R4 | unit rows `(false, None, true) -> true` and `(false, Some(""), true) -> true` |
| R5 | the phase-17 test's `observe_bzr_transport` call |
| R6 | manual: `make functional-test` then `make functional-compare` in one checkout, no `NO_COLOR` set |
| R7 | two separate controlled faults, plan Task 2 step 4. The test's `&&` short-circuits, so one fault cannot prove both halves. |

`make lint` and `make test` gate the unit half, `make functional-test` the functional half, and
`make functional-compare` the comparison half. The change is not security-relevant: it selects a
display attribute for a stream bzr already writes, adds no entry point, parses no foreign input,
touches no credential path, and changes no dependency. API-key redaction in verbose diagnostics
stays asserted by `verbose-response-body-diagnostics-redact-api-keys` in the same phase.
