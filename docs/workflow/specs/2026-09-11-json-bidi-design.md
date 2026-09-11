# JSON bidi escaping — issue #757

## Scope and authority

The operator approved standard JSON Unicode escapes at success and error seams,
preserved decoded values and shapes, ADR/tests/docs, and no schema-version bump.
Frozen authority: issue #757 WORK:SCOPE token `q757-ca7b9137`.
Non-bidi `Cf` remains owned by #758's retained policy. No ambiguities remain.
Decision: [ADR 0072](../../adr/0072-standard-json-bidi-escaping.md).

## Design and success

Scan serialized output at the shared pretty JSON and NDJSON writers and the
binary's structured error writer. Replace the twelve existing bidi controls with
standard four-hex Unicode escapes. Reuse one helper through the existing output
module boundary, following the binary's terminal-escape helper import pattern.
Only valid serialized JSON enters this helper; serde continues to own JSON syntax.
String keys, nested values, literal backslashes and other Unicode decode unchanged.
Pretty output retains its envelope; NDJSON retains compact bare records and zero
records for an empty array. Errors retain their keys and exit codes. Serialization
and writer error handling retain their existing behavior. SCHEMA_VERSION stays
unchanged. Update the CLI reference and the tests that previously pinned raw bidi.

## Failure model

- Actors and deployments: local CLI users, automation and terminal/editor readers;
  server strings may be malicious on supported target architectures.
- Invariants and assets: valid JSON, decoded value identity, record boundaries and
  raw-output display integrity for the twelve selected bidi characters.
- Accepted failure classes: a downstream JSON parser restores bidi characters;
  rendering its decoded strings safely belongs to that downstream consumer.
- Covered elsewhere: non-bidi Cf policy (#758), table escaping (ADR 0065/0070),
  existing output-I/O and serializer failure behavior. Raw attachment bytes and
  outbound API serialization are distinct data paths, not JSON CLI envelopes.

## Threat model

- Boundary inventory: existing server-controlled strings to emitted JSON text;
  no new input, authentication or network boundary.
- Actor model: malicious or compromised Bugzilla controls response strings; bzr
  trusts serde to serialize valid JSON and itself to select the bidi escape set.
- Control per boundary: post-serialization escaping removes active bidi from raw
  success/error JSON while standard decoding preserves data.
- Out of scope: downstream re-rendering, non-bidi Cf, transport authentication.

## Validation

Focused tests exercise every selected character in keys/nested strings; normal
Unicode, backslashes, neighboring non-bidi characters, JSON envelopes, NDJSON
arrays/scalars/empty arrays, and structured errors. Prove red before implementation.
Extend phase 08h against real Bugzilla using BZR_STDOUT_RAW for encoding assertions
and decoded values for stored-data identity, including a credentialless read.
Run make test, make lint, and the default full make functional-test with one frozen
binary; document its source commit and hash. Documentation is reviewed as prose.
