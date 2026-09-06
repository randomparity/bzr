# Design: plain-text tracing off a terminal, and a fresh comparison container

Issue: [#722](https://github.com/randomparity/bzr/issues/722). Problem, decision, and rejected
alternatives: [ADR 0058](../../adr/0058-suppress-tracing-ansi-off-terminal.md), amending
[ADR 0045](../../adr/0045-observe-comparison-transport-from-debug-events.md). Component shapes
and steps: [the plan](../plans/2026-09-06-tracing-ansi-off-terminal.md). This document is the
requirement set and the proof map, and restates neither.

## Requirements

- **R1** — bzr's tracing subscriber writes no ANSI escape to stderr when stderr is not a
  terminal, at any verbosity and under any `RUST_LOG` value. The `colored` stdout path is
  untouched and out of scope here.
- **R2** — `--no-color` is *extended* to cover the tracing stream. Neither `docs/bzr-cli.md:50`
  nor the clap doc comment at `src/cli/mod.rs:247-251` promised this before, and both are
  corrected: each stream's automatic suppression follows its own terminal status, and
  `CLICOLOR` / `CLICOLOR_FORCE` govern stdout colour only.
- **R3** — A non-empty `NO_COLOR` still suppresses tracing ANSI when stderr *is* a terminal. An
  explicit `with_ansi` call replaces `tracing-subscriber`'s own `NO_COLOR` default, so the
  decision must re-apply the rule rather than inherit it.
- **R4** — On a terminal with no `--no-color` and `NO_COLOR` unset or empty, ANSI stays on.
- **R5** — `observe_bzr_transport` returns a transport for a real `bzr -vv` invocation whose
  stderr was redirected to a file, with no environment variable set by the caller.
- **R6** — `make functional-compare` and `make functional-compare-all` run against a container
  created for that run.
- **R7** — The functional assertion bites on both halves independently: an ANSI regression turns
  it red, and a request-boundary event the harness regexes no longer match also turns it red.

## Non-goals

- Changing what the request-boundary debug events say or when they are emitted; ADR 0045 owns
  that.
- Extending `CLICOLOR` / `CLICOLOR_FORCE` to the tracing stream.
- Issue #721's truncated action SHA, which has its own PR.

## Proof map

| Requirement | Proof |
|---|---|
| R1 | `verbose-tracing-is-plain-on-redirected-stderr` (functional phase 17), plus the `stderr_is_terminal = false` unit cases |
| R2 | unit case: flag set, terminal, `NO_COLOR` unset → `false`; both doc surfaces edited in Task 1 step 6 |
| R3 | unit case: no flag, terminal, `NO_COLOR = "1"` → `false` |
| R4 | unit cases: no flag, terminal, `NO_COLOR` unset → `true`; `NO_COLOR = ""` → `true` |
| R5 | the same phase-17 test's `observe_bzr_transport` call |
| R6 | manual: `make functional-test` then `make functional-compare` in one checkout, no `NO_COLOR` set |
| R7 | two controlled faults, plan Task 2 step 4: hard-code `.with_ansi(true)` and expect the `stderr unexpectedly contains` failure; then, separately, break `BZR_REST_BOUNDARY_RE` and `BZR_XMLRPC_BOUNDARY_RE` and expect the `transport is not observable` failure. The `&&` in the test short-circuits, so one fault cannot prove both halves. |

`make lint` and `make test` gate the unit half; `make functional-test` the functional half;
`make functional-compare` the comparison half.

## Threat model

Not security-relevant: the change selects a display attribute for a stream bzr already writes,
adds no entry point, parses no foreign input, touches no credential path, and changes no
dependency. API-key redaction in verbose diagnostics is unaffected and stays asserted by
`verbose-response-body-diagnostics-redact-api-keys`.
