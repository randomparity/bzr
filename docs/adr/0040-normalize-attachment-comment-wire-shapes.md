# 0040 — Normalize attachment and comment wire shapes at the REST boundary

## Status

Accepted

## Context

Bugzilla installations expose several equivalent JSON shapes for attachment and comment
operations. Attachment creation IDs may be decimal strings or numbers; by-ID attachment reads
may return either a keyed map or the flat array already supported by list reads; and comment
privacy may be a boolean or binary integer. Strict decoding can therefore report failure after
an upload has already committed, or reject otherwise usable reads. Attachment list requests also
omit no fields, so stock servers transfer base64 bodies that the list output discards.

## Decision

Normalize these variants only where each wire value enters the REST client:

- deserialize each attachment-create ID through the shared unsigned number-or-decimal-string
  adapter;
- inspect the by-ID response's `attachments` container, decode an object as the keyed shape or an
  array as the flat shape, and select the requested ID inside that recognized container;
- deserialize `Comment.is_private` through the shared optional bool-or-binary-integer adapter; and
- add `exclude_fields=data` to REST attachment list requests. Bulk download retains its existing
  per-attachment re-fetch when list metadata has no body.

Malformed or unrecognized container values remain deserialize errors. A recognized keyed object
or flat array that does not contain the requested ID returns `NotFound`; it never returns a
different attachment merely because it is first.

## Consequences

The public Rust and CLI output types do not change. REST reads accept more production-compatible
representations, list traffic excludes unused bodies, and bulk downloads make one metadata request
plus one body request per attachment. Hybrid and XML-RPC transport selection remains unchanged.

The functional response-shape proxy becomes responsible for proving the affected REST variants
against live Bugzilla containers and for logging each rewrite or observed `exclude_fields=data`
request.

## Considered & rejected

- **Deserialize the whole response through `serde_json::Value` everywhere.** judgment: this would
  discard typed boundary validation beyond the fields known to vary.
- **Add a second attachment-list method for metadata.** verified: `rg -n
  'get_attachments\\(' src` at `b9d779ef1ad9c196f37e1b8a69b7897276e640fc` shows the existing
  list method serves both display and bulk download, whose `write_one_attachment` already re-fetches
  absent data; another public client path would duplicate policy.
- **Return the first item in a flat by-ID response.** judgment: binding selection to the requested
  identifier prevents a malformed or over-broad response from silently returning the wrong file.
- **Leave the current behavior unchanged.** verified: Bugzilla's documented attachment-create
  example uses string IDs, while `AttachmentCreateResponse` at the recorded base requires
  `Vec<u64>`; a successful server mutation can therefore be followed by a local decode error.
