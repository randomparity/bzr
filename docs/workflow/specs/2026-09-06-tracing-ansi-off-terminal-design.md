# Design: plain-text tracing off a terminal, and a fresh comparison container

Issue: [#722](https://github.com/randomparity/bzr/issues/722).
Problem statement, decision, and rejected alternatives:
[ADR 0058](../../adr/0058-suppress-tracing-ansi-off-terminal.md), amending
[ADR 0045](../../adr/0045-observe-comparison-transport-from-debug-events.md).
Component shapes and steps:
[the plan](../plans/2026-09-06-tracing-ansi-off-terminal.md).
This document is the requirement set and the proof map, and restates neither.

## Requirements

- **R1** — bzr writes no ANSI escape to stderr when stderr is not a terminal, at any verbosity
  and under any `RUST_LOG` value.
- **R2** — `--no-color` suppresses ANSI on the tracing stream as well as on stdout, which its
  clap help text and `docs/bzr-cli.md` already promise.
- **R3** — A non-empty `NO_COLOR` still suppresses tracing ANSI when stderr *is* a terminal. An
  explicit `with_ansi` call replaces `tracing-subscriber`'s own `NO_COLOR` default, so the
  decision must re-apply the rule rather than inherit it.
- **R4** — On a terminal with no `--no-color` and `NO_COLOR` unset or empty, ANSI stays on; the
  interactive experience is unchanged.
- **R5** — `observe_bzr_transport` returns a transport for a real `bzr -vv` invocation whose
  stderr was redirected to a file, with no environment variable set by the caller.
- **R6** — `make functional-compare` and `make functional-compare-all` run against a container
  created for that run.
- **R7** — The observation bites: if bzr resumes emitting escapes unconditionally, the functional
  tier turns red rather than passing quietly.

## Non-goals

- Changing what the request-boundary debug events say or when they are emitted. ADR 0045 owns
  that and is unchanged.
- Extending `CLICOLOR` / `CLICOLOR_FORCE` to the tracing stream.
- Issue #721's truncated action SHA, which is why nobody saw this. It has its own PR.

## Proof map

| Requirement | Proof |
|---|---|
| R1 | `verbose-tracing-is-plain-on-redirected-stderr` (functional phase 17), plus the `stderr_is_terminal = false` unit cases |
| R2 | unit case: flag set, terminal, `NO_COLOR` unset → `false` |
| R3 | unit case: no flag, terminal, `NO_COLOR = "1"` → `false` |
| R4 | unit cases: no flag, terminal, `NO_COLOR` unset → `true`; `NO_COLOR = ""` → `true` |
| R5 | the same phase-17 test's `observe_bzr_transport` call |
| R6 | manual: `make functional-test` then `make functional-compare` in one checkout, no `NO_COLOR` set |
| R7 | controlled fault: hard-code `.with_ansi(true)`, confirm phase 17 fails, revert |

`make lint` and `make test` gate the unit half; `make functional-test` the functional half;
`make functional-compare` the comparison half.

## Threat model

Not security-relevant. The change moves no trust boundary: it selects a display attribute for a
stream bzr already writes, adds no entry point, parses no foreign input, touches no credential
path, and changes no dependency. The one adjacent property is API-key redaction in verbose
diagnostics, which is unaffected — `17-global-options.sh` already asserts it
(`verbose-response-body-diagnostics-redact-api-keys`) and that test keeps running. Removing
escapes from a captured log is a small improvement: an escape in a pasted CI log is a
terminal-control byte the reader did not consent to.
