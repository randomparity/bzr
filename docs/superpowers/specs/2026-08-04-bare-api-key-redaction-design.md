# Bare API-key error redaction design

Issue: #509  
Decision: [ADR 0016](../../adr/0016-thread-local-error-redaction-context.md)

## Scope charter

- **Interaction:** unattended.
- **Scope identity:** issue #509, annotation token `scope-509-20260805T022000Z`.
- **Outcome:** prevent a configured Bugzilla API key echoed bare or behind a
  percent-encoded marker from reaching final CLI error output.
- **Completion criteria:** the issue body and owner comment require bare-key redaction
  for `Api` and `HttpStatus` on human, JSON, and progress-NDJSON paths; encoded-marker
  redaction; a documented short-key floor; wiremock coverage for both `Api`
  construction sites; a functional phase extension; and preservation of existing marker
  behavior.
- **Provenance:** issue #509 and its 2026-08-05 repository-owner comment; campaign notes
  are empty.
- **Exclusions:** cookies and session tokens; tracing work owned by #511; response-body
  bounding owned by #512.
- **Surface:** authentication redaction and credential-resolution seams, final error
  rendering, the production thread-handoff guard in `Makefile` and its focused shell
  checker/fixtures, directly related tests, functional phase, changelog, and
  issue-specific design artifacts. The orchestrator approved this guard expansion after
  spec review found the existing CONC-3 gate incomplete for this decision.
- **Ambiguities:** none. The issue explicitly delegates the seam and minimum-length
  decisions.

## Approaches considered

1. **Thread-local redaction context (chosen).** Register the already-resolved credential
   and consume it only while formatting errors. This covers pre-client and client errors,
   preserves one formatting seam, avoids a public error-shape change, and follows the
   repository's guarded single-thread runtime invariant.
2. **Secret-bearing errors.** Add a private wrapper or field to `Api` and `HttpStatus`.
   This makes secret propagation explicit but changes every constructor and risks exposing
   the credential through derived debug output.
3. **Output-layer credential reload.** Reload configuration and credentials in `main`
   after dispatch fails. This keeps global state out, but duplicates source selection and
   can perform a second keyring access with a different result.

## Design

`bugzilla_auth` owns a thread-local optional redaction key and exposes three crate-private
operations: clear it, register a resolved key, and redact a message. `dispatch` clears the
slot before building command context. Credential resolution registers a non-empty key as
soon as it succeeds, before auth detection, version probing, or client construction can
return a server error. `dispatch` clears the slot before returning success, and the binary
clears it after converting a failed dispatch into its final formatted output string.

`redact_api_key` remains the single function called by `BzrError` display. It recognizes
literal `Bugzilla_api_key=` plus upper- and lower-case percent-encoded equals markers.
After marker redaction, it replaces every occurrence of the active configured key when
the key is at least eight bytes. Eight bytes is long enough to avoid common words and
short identifiers while covering the project's test credential and normal Bugzilla keys.
The threshold is byte-based because replacement searches UTF-8 bytes and credentials are
opaque strings; no partial scalar value is constructed.

The final error line remains the same for table, JSON, and NDJSON output. With
`--progress ndjson`, the separate progress error event contains only type and exit code;
the following formatted error line is protected by the same seam.

## Error handling and lifecycle

Credential lookup errors cannot leak a resolved secret because registration happens only
after successful resolution. Anonymous commands leave the cleared context empty. A later
dispatch on the same thread begins by clearing stale state. Keyring, inline, and
environment-backed credentials all converge at the same resolver return. Direct library
client construction can leave a registered key on its thread until another registration
or thread teardown; the guarantee here is final CLI output, not safe derived `Debug` for
raw server payloads or isolation between unrelated direct-client displays.

## Threat model

### Boundary inventory

- **Existing boundary widened:** a Bugzilla-controlled response crosses into CLI stderr;
  the server can repeat request credentials in arbitrary text.
- **Existing boundary used:** configured secret material crosses from config, environment,
  or keyring into request authentication and the redaction context.
- **No new external boundary:** the context is memory-only and crate-private.

### Actors and trust

The Bugzilla server or an intermediary proxy controls response text. The local operator
controls configuration and is trusted with the key. Final CLI error output protects
recognized marker values at every length and bare configured keys of at least eight bytes
before they reach terminal transcripts or copied reports. A shorter bare key remains an
explicit false-positive trade-off and may appear unchanged. Tracing and other diagnostic
log paths remain owned by #511 and must not infer safety from this design.

### Controls

- Marker-driven redaction covers literal and encoded query markers at any key length.
- Bare substring redaction uses the exact resolved credential, not a guess, and refuses
  keys shorter than eight bytes to bound false positives.
- Dispatch clearing and CONC-3's current-thread/no-fan-out guard bound the key to one CLI
  invocation. The bounded scanner rejects these in production Rust files: Tokio
  `spawn`, `spawn_local`, `spawn_blocking`, `LocalSet`, `JoinSet`, `join!`, `try_join!`,
  and `select!`; futures `FuturesUnordered`, `buffered`, `buffer_unordered`, and
  `for_each_concurrent`; and `std::thread::spawn` / `thread::spawn`. Adding another
  concurrency primitive to the dependency or source surface requires updating this
  inventory. Sibling `*_tests.rs`, test helpers, and documentation are excluded so
  examples and tests do not create false failures.
- The secret is not added as a separate `BzrError` field, serialized detail, log field,
  or persisted value. Raw server payloads inside `Api` and `HttpStatus` remain unredacted
  in derived `Debug`; only the final CLI `Display` path is a safe output boundary.

### Explicitly out of scope

Cookie and session-token redaction remain outside issue #509. Response tracing and body
size limits remain owned by #511 and #512. A future multi-thread runtime or production
thread-handoff API must replace the thread-local context; the static guard covers the
in-repository entry points above, while dependency-internal threads that do not move this
crate's futures or redaction calls remain outside the invariant.

## Testing

- Unit tests pin literal and case-insensitive encoded markers, multiple occurrences,
  exact bare-key replacement, the eight-byte boundary, short-key refusal, stale-context
  clearing at dispatch entry, Unicode surrounding text, and idempotence.
- Wiremock tests make both `Api` construction sites echo the active key bare; the
  non-JSON `HttpStatus` path receives equivalent coverage.
- Main formatting tests assert the configured bare key is absent and `[REDACTED]` is
  present for table, JSON, and NDJSON, including progress-enabled formatting semantics.
  Separate tests exercise both lifecycle exits: after final error formatting and after
  `dispatch` returns success, an unrelated error containing the former key remains unchanged.
- The `check-no-spawn` guard delegates to one shell checker. Its self-test builds temporary
  fixture trees and proves representative tokens from every forbidden family fail in a
  production `.rs` file, while the same tokens in sibling tests, test helpers, and docs
  pass. `make check-no-spawn` runs both the real scan and this self-test, so CI exercises
  the regex and the file-selection boundary together.
- The existing real-container phase is extended to exercise `--progress ndjson` alongside
  table and JSON, asserting the configured key never reaches stderr. Stock Bugzilla does
  not echo the key bare, so synthetic unit/wiremock tests remain the positive regression
  proof while the container test proves the real user path and no-regression direction.

## Documentation

`CHANGELOG.md` will describe bare and percent-encoded marker coverage and the deliberate
eight-byte minimum. No CLI reference changes because no command or flag changes.
