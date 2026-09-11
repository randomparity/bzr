# Authenticated attachment streaming (#756)

## Problem and authority

`attachment download` materializes base64 inside a bounded API response and rejects
large attachments. The operator approved existing authenticated REST/XML-RPC streaming
and replacement of the pre-release buffered client API, after CGI authentication was
shown incompatible. Frozen authority is issue #756 WORK:SCOPE token q756-45a97a32.
Exclusions are explicitly empty. [ADR0071](../../adr/0071-stream-attachment-payloads.md)
records the transport and staging decision.

## Scope and architecture

Retain REST/Hybrid/XML-RPC dispatch, authentication, TLS, retries, and existing error
classification. A payload reader consumes `Response::chunk()` and incrementally decodes
only the requested attachment's base64 data into a private temporary file. It retains a
bounded copy of the structured envelope with each payload replaced by a bounded index sentinel. Store decoded candidates as
sequential ranges in one temporary file; after envelope parsing, select the range
associated with the requested record, including flat-array data-before-id ordering.
The streaming path checks the selected embedded ID and complete outer XML framing;
existing parsers supply mapping and API error classification. Nonmatching records are
permitted, but duplicate requested records are rejected before exposing the selected range.
Metadata outside the payload continues to obey the shared 64 MiB bound. Neither
Content-Length nor a server's declared attachment size controls allocation.

The JSON lexical reader handles chunk boundaries, quoted keys, escaped base64 characters,
and padding. The XML lexical reader handles markup boundaries, entity references,
base64 and string data values, and distinguishes comments/CDATA from markup. Decoding
uses the existing base64 dependency with fixed-size working buffers. Malformed or
ambiguous payload selection is an error; another attachment's data is never returned.
The normal structured readers remain bounded and unchanged for other resources.

Replace `download_attachment(id) -> (String, Vec<u8>)` with
`download_attachment(id) -> (String, std::io::Take<std::fs::File>)`; the file is positioned
at the selected range, limited to its length, and automatically removed when closed.
The read limit supplies existing result byte counts. The existing
pinned tempfile dependency becomes a normal runtime dependency (3.27.0, registry stable
verified at design time). No new package or toolchain floor is introduced.

Single-file and stdout commands copy the validated file in bounded chunks. Batch paths
request metadata without `data` and call the same download interface for each target.
Add XML-RPC metadata-only selection rather than fetching and discarding base64. Preserve
existing full-record reads used outside download. Stage before opening destination files
so a network, protocol, or decoding error does not truncate an existing file or emit
unvalidated stdout. Preserve current destination naming, overwrite/symlink semantics,
batch continue-on-error results, and structured output shapes. Disk write failures retain
the existing partial-write semantics; do not add unrelated atomic replacement behavior.

## Success

- Payloads exceeding the old limit download with bounded RAM in supported REST,
  Hybrid, and XML-RPC single/stdout/batch shapes.
- Anonymous public and API-key private downloads retain access behavior.
- Metadata, malformed envelopes, and error bodies remain bounded; bad base64,
  wrong IDs, duplicate requested records, and truncated streams fail before output.
- Success byte counts and bytes match the uploaded content; existing filenames and
  result schemas remain valid. Temporary storage is released on success/error.

## Failure model

- Actors and deployments: local CLI operators and library callers on supported targets;
  upstream Bugzilla 5.0/5.2/5.3 and already-supported REST envelope variants; malicious
  API servers, attachment uploaders, and network intermediaries.
- Invariants/assets: credentials, bounded memory, exact downloaded bytes, existing
  output files, anonymous/private authorization, and per-target batch outcomes.
- Accepted failure classes: insufficient temporary disk space or filesystem write
  errors return actionable I/O errors; stdout copy errors may emit a partial stream,
  as existing stdout writes can. Temporary disk use scales with decoded payload size.
- Covered elsewhere: TLS and redirect policy in tls/, authentication fallback and
  transient retries in client/transport.rs; no change to those policies.

## Threat model

- Boundary inventory: server-controlled response bytes enter a payload-specific decoder;
  uploader-controlled filenames reach existing destination selection; validated payloads
  cross from private staging to caller-selected files/stdout. No new auth boundary.
- Actor model: servers control envelopes and payloads; uploaders control attachment
  names/data; operators control destinations. Existing filesystem trust is retained.
- Controls: bounded envelope, fixed decode buffers, exact payload/ID validation,
  existing safe_basename, private anonymous temporary files, unchanged auth/TLS and
  sanitized diagnostics. Cleanup uses file ownership and occurs on error as well as success.
- Outside this change: existing local output-path symlink/write semantics, server disk
  limits, TLS/auth implementation internals, and process termination/power-loss recovery
  are covered by their existing behavior; this change introduces no persistent state.

## Validation

Focused unit/transport tests cover each byte split across JSON/XML markers, escapes,
padding, empty/malformed/truncated payloads, wrong IDs, metadata bounds, and error bodies.
Command tests cover staging failure before stdout/destination writes and batch metadata
selection. Existing attachment baseline: 262 unit and 10 integration tests pass.
Live functional phases exercise large public/private payloads, API modes and all download
shapes with byte/hash checks; run the complete default functional suite before opening PR.
Run make test and make lint before implementation commit. Rust 1.89.0, current-thread
runtime, and x86_64/aarch64/powerpc64le/s390x targets remain unchanged.
