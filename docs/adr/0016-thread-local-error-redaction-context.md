# ADR 0016: Thread-local error-redaction context

## Status

Accepted

## Context

Bugzilla can echo an API key in an error message without the usual
`Bugzilla_api_key=` marker. Marker-only redaction therefore cannot protect the final
human, JSON, and progress-NDJSON error paths. Redacting a bare value requires the
configured secret at formatting time, after command dispatch has returned an error.

The CLI runs on Tokio's current-thread runtime, and `make check-no-spawn` rejects task
fan-out in production code. Credential resolution and final error formatting therefore
happen on the same OS thread for one invocation.

## Decision

Keep the active API key in a crate-private thread-local redaction context. Clear the
context at the start of every dispatch and register a resolved credential before any
authenticated request. The final `BzrError` display seam applies marker redaction and
then masks every occurrence of that active key when it is at least eight bytes long.
The binary clears the context after it has materialized the final formatted error, and
`dispatch` clears it before returning success.

Percent-encoded `Bugzilla_api_key%3D` markers remain marker-driven and are redacted at
any value length. The existing `make check-no-spawn` guard is part of this decision: a
move to task fan-out or a multi-thread runtime requires replacing the context mechanism.

## Consequences

- Every existing `BzrError::Api` and `BzrError::HttpStatus` construction site stays
  covered by one display seam, including failures before `BugzillaClient` construction.
- The design adds no separate secret-bearing field to the public error enum.
- Raw server payloads stored in `Api` and `HttpStatus`, including their derived `Debug`
  representation, remain unredacted. They are outside this decision's final CLI-output
  boundary and must not be treated as safe diagnostics.
- Tests running on separate OS threads cannot overwrite one another's active key.
- Sequential CLI invocations cannot inherit a prior key: dispatch clears before work and
  before returning success, while the binary clears after formatting an error. A library caller that
  constructs clients directly retains the thread-local context until the next registration
  or thread teardown; that caller must not treat unrelated `BzrError` display on the same
  thread as an independently scoped operation.
- Bare keys shorter than eight bytes are deliberately left unchanged to avoid shredding
  unrelated prose; marked values are still redacted.

## Considered & rejected

- **Store the key in each error variant.** This expands a public error contract, makes
  secret-bearing `Debug` behavior easy to regress, and requires every constructor to
  remember the field.
- **Resolve credentials again in `main` after an error.** This duplicates configuration
  selection, can prompt or fail at a keyring a second time, and still misses the exact
  credential used if state changes between reads.
- **Use process-global synchronized state.** It is broader than the runtime invariant
  requires and lets parallel test threads overwrite one another's secret.
- **Mask every short substring.** Three-character credentials can occur throughout
  ordinary diagnostics, producing destructive false positives.
