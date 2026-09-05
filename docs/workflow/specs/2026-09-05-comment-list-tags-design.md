# Comment list tag output design

- Issue: #700
- Scope token: `q700-3d353b4e`
- Decision: [ADR 0048](../../adr/0048-comment-tags-are-always-present-arrays.md)
- Branch: `feat/comment-list-tags-700`
- Base branch: `main`

## Outcome and scope

`bzr comment list <id>...` will expose comment tags in table, JSON, and NDJSON output. JSON
projection will accept `tags`, the published comment schema will describe the field, and the
help/reference text will accurately name it. This completes the existing tag mutation/read
round trip without adding an endpoint or changing mutation/search behavior.

The permitted implementation surface is the comment type and XML-RPC mapper, the comment
writer and projection declaration, the comment schema plus schema-version consumers, CLI
help/reference text, and focused unit/functional tests. No dependency, config, auth,
pagination, or per-comment fetch behavior changes.

## Verified server contract

Before design, this run created and tagged a comment on each supported functional image.
Native REST and XML-RPC `Bug.comments` responses both returned the tag on Bugzilla 5.0.6,
5.2, and 5.3.3+. The existing one-request comment-list data flow is therefore sufficient.

## Data model and transport behavior

`Comment` gains `pub tags: Vec<String>` with `#[serde(default)]`. REST response envelopes
already deserialize through `Comment`, so an omitted field becomes `[]` and a present JSON
array is preserved. The XML-RPC mapper uses the existing `get_str_array` helper. Absence or
a non-array becomes `[]`, and non-string array members are ignored, matching existing
optional-array compatibility behavior.

`COMMENT_FIELDS` gains `tags`. Because JSON-family writers serialize `Comment` through the
shared projection path, full output always contains the field. On a single-ID call,
`--fields tags` retains only that key. On a multi-ID call, accepted ADR 0049's attribution
contract continues to force `bug_id` into the result and emit its existing stderr notice, so
the projected records contain exactly `bug_id` and `tags`.

## Presentation contract

For a tagged comment, table output prints one indented metadata line before the blank line
and body:

```text
  Tags: needs-info, follow-up
```

Input order is preserved and tags are joined with `, `. Untagged comments omit the metadata
line. Private marking and comment body formatting do not change.

The `comment list` long help again says the printed fields include tags. The CLI reference
adds tags to the command description/example so documentation matches the executable.

## Published JSON contract

`schemas/comment.json` gains a required `tags` property with `type: array` and string items.
The maximally populated schema-conformance sample includes a non-empty tag list. Adding an
always-emitted key is additive under ADR 0007, so `SCHEMA_VERSION` becomes `3.0.1`. Every
current pinned version fixture, installed skill reference, functional assertion, and example
must advance in the same change; no other schema shape changes.

## Errors and compatibility

- Missing REST or XML-RPC `tags` remains accepted and emits `[]`.
- A REST `tags` value of the wrong JSON shape fails normal serde deserialization.
- XML-RPC tag decoding follows the existing lenient optional-string-array mapper.
- No tag sorting, normalization, or deduplication is introduced; server order and values are
  preserved.

## Verification

Focused type tests prove present and absent REST-shaped values, wrong-shaped REST rejection,
field-list drift, and always-present serialization. XML-RPC mapper tests prove non-empty tags,
absence, non-array-to-empty behavior, and filtering of non-string array members. Writer tests
prove the non-empty table line, omission for empty tags, and full/projected `tags` in both JSON
and NDJSON through the existing command/projection paths.

Functional coverage tags a created comment, reads it through both `--api rest` and
`--api xmlrpc`, asserts each tag array, projects `--fields tags`, and checks the restored help
claim. A table case asserts the exact tag line immediately before the blank line and tagged
comment body. NDJSON cases assert the full and projected record shapes. Two-bug JSON and NDJSON
projection cases prove `bug_id` remains beside `tags` with the existing attribution warning. A
new `--server public comment list` case first requires at least one visible comment, then proves
every credentialless result has an array-valued `tags` key without claiming that anonymous
visibility matches authenticated visibility. The full supported-version functional suite runs
these arms on all three Bugzilla images.

## Guardrails and architecture context

Host architecture is arm64 with BSD userland. Declared targets are x86_64, aarch64,
powerpc64le, and s390x; the host architecture is included. Required local guardrails are
`make lint`, `make test`, and `make functional-test-all`; focused iteration uses
`make test-one T=<name-substring>`. CI separately gates formatting, clippy, tests, test
layout/functional IDs, cross-target checks, schemas, and other repository workflows. The ADR
index is not coupled to an individually hard-gated repository check, but this solo run adds
its own row by convention.
