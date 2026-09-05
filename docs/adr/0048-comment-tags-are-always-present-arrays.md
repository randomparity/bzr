# ADR 0048: Comment tags are always-present arrays

## Status

Accepted

## Context

`bzr comment tag` can mutate server-side comment tags, but the `Comment` domain type,
field projection list, table writer, and published schema cannot expose them. Issue #700
requires the read side of that round trip. Bugzilla may omit fields from reduced or older
payloads, while bzr's JSON contract needs one stable shape.

The existing comment endpoint is already used for both REST and XML-RPC reads. Authenticated
live probes on Bugzilla 5.0.6, 5.2, and 5.3.3+ confirmed that both transports include a
`tags` array in `Bug.comments` after a tag is written.

## Decision

Add `tags: Vec<String>` to `Comment` with serde's default for an omitted server field. Map
the XML-RPC `tags` member as an array of strings, treating absence as an empty array and a
present non-array or non-string element as a protocol error. REST uses the shared serde type
and therefore gets the same absent-to-empty compatibility behavior.

JSON and NDJSON always serialize `tags`, including `[]`; `COMMENT_FIELDS` includes `tags`,
so projection can select or exclude it. Table output adds `Tags: <comma-separated values>`
only for non-empty tag sets, avoiding an empty label on every untagged comment.

Publish `tags` as a required array of strings in `schemas/comment.json`. This is an additive
payload change under ADR 0007, so advance `SCHEMA_VERSION` from `3.0.0` to `3.0.1` and update
its pinned consumers and examples.

## Consequences

Comment list consumers can observe the state written by comment-tag operations without an
extra request. Missing server fields remain compatible and become an empty collection in
output. Malformed XML-RPC tag values fail at the protocol boundary instead of being silently
dropped. Existing consumers that reject unknown JSON keys can use the schema version to
detect the additive contract revision.

## Considered & rejected

- **Fetch tags separately for every comment.** verified: authenticated REST and XML-RPC
  `Bug.comments` probes on Bugzilla 5.0.6, 5.2, and 5.3.3+ each returned the written
  `quest700-probe` tag; extra requests would add cost without supplying missing data.
- **Model tags as `Option<Vec<String>>`.** judgment: this would expose transport omission as
  a second public state even though the requested contract is an array and absence is safely
  represented by `[]`.
- **Silently discard malformed XML-RPC tag members.** judgment: accepting a present value of
  the wrong shape would hide server/protocol drift and make REST and XML-RPC behavior differ.
- **Print an empty `Tags:` line for every comment.** judgment: it adds visual noise without
  improving the tagged-comment round trip that issue #700 requires.
- **Keep schema version `3.0.0`.** verified: accepted ADR 0007 defines patch increments for
  additive payload changes; adding an always-emitted `tags` key is additive.
