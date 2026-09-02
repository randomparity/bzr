# Attachment and comment wire-conformance design

Issue: #627  
ADR: [0040](../../adr/0040-normalize-attachment-comment-wire-shapes.md)

## Outcome

Make attachment upload and reads, plus comment list, tolerate the equivalent Bugzilla REST shapes
identified in issue #627, and avoid transferring attachment bodies for metadata lists without
breaking bulk download.

## Requirements and boundaries

- Attachment creation accepts a non-negative JSON integer or decimal string for every `ids`
  element. Empty arrays keep the existing data-integrity error; negative, fractional, boolean,
  null, object, array, non-decimal, and out-of-range values fail decoding.
- REST `attachment view` and `attachment download <id>` accept either a keyed
  `attachments` object or flat `attachments` array. Selection is by the requested ID. An empty
  envelope or a flat array lacking that ID returns `NotFound`.
- `Comment.is_private` accepts absent/null, booleans, and integers `0`/`1` through the shared
  adapter. Other values remain decoding errors.
- REST attachment list requests send `exclude_fields=data`. XML-RPC list behavior is unchanged.
  `attachment download --bug` keeps writing the correct bytes by exercising the existing
  body-missing re-fetch path.
- A functional proxy run proves string upload IDs through the public command, both by-ID flat
  response paths, integer comment privacy, list field exclusion, bulk body re-fetch, and a
  credentialless public read where the command supports it.
- Every regression test is run against an intentional controlled fault or pre-fix implementation
  and observed red before the implementation is accepted.
- `make lint`, `make test`, and `make functional-test-all` pass.

The public CLI syntax, output schema, API-mode dispatch, privacy policy, and unrelated resources
are excluded. ADR 0040's index row is owned by the campaign orchestrator because the index is not
CI-coupled.

## Design

### Narrow response normalization

`AttachmentCreateResponse.ids` uses a private field deserializer that visits a sequence and calls
`types::deserialization::u64_from_number_or_string` for each element. This reuses ADR 0033's value
contract without exporting a new public type or weakening unrelated numeric fields.

By-ID REST reads parse one raw JSON value, then call `BugzillaClient::try_envelopes` with two
extractors. The keyed extractor looks up the decimal requested ID rather than taking an arbitrary
map value. The flat extractor searches `Attachment.id` for the same ID. Because extractors need the
requested ID, the selector accepts closures or the by-ID helper performs equivalent ordered
attempts while preserving `try_envelopes`' first-error diagnostics. Both full and metadata-only
paths call the same selector; the latter keeps `exclude_fields=data`.

`Comment.is_private` adds the same serde attributes as attachment privacy:
`default` plus `option_bool_from_int_or_bool`.

### Metadata-only list and bulk fallback

`get_attachments_rest` switches from `get_json_value` to a raw-value request carrying
`exclude_fields=data`. Existing envelope parsing is unchanged. XML-RPC-first Hybrid behavior is
unchanged; the optimization applies when REST is selected or used as transport fallback.

`write_one_attachment` remains the only bulk body fallback. A command-level regression provides
metadata with `data` absent, expects a by-ID fetch, and verifies the written bytes. No new download
method or caching layer is introduced.

### Executable production-shape proof

The loopback functional proxy adds an opt-in attachment/comment mode. It rewrites successful REST
attachment-create IDs to strings, keyed by-ID envelopes to flat arrays, and comment privacy booleans
to binary integers. It also records whether list requests carry exactly `exclude_fields=data` and
logs route-specific evidence. Proxy unit tests cover matching, nonmatching, malformed, and absent
fields.

The attachment phase starts that mode around self-contained fixtures, exercises upload with a
private comment, view, single download, list, and bulk download, then asserts the output/files and
proxy evidence. The comment phase performs a REST comment list through the same mode. A named
credentialless server exercises public attachment metadata without forwarding a credential.

## Error handling

- Invalid upload IDs return the existing deserialize error; an empty `ids` list returns the
  existing `DataIntegrity` error.
- Envelope decoding preserves the first relevant decode error when no supported envelope matches.
  Supported envelopes lacking the requested attachment return `NotFound`.
- A list server that ignores `exclude_fields` remains safe: the client may receive `data`, but
  output behavior is unchanged.
- Proxy backend, malformed JSON, and startup failures use the existing bounded 502/startup paths;
  missing route evidence fails the functional case.

## Threat model

### Boundary inventory

- Existing boundary widened: remote Bugzilla JSON enters attachment/comment serde. Only decimal
  string IDs, flat by-ID envelopes, and binary integer privacy values are newly accepted.
- Existing boundary changed: attachment list query construction sends a fixed literal field
  exclusion; no user-controlled query fragment is added.
- Test-only boundary widened: a loopback proxy rewrites successful JSON and observes query names.

### Actors and controls

- A remote Bugzilla administrator controls response values. Shared adapters reject negatives,
  fractions, booleans-as-IDs, non-decimal strings, and out-of-range values; flat selection requires
  exact numeric ID equality; privacy accepts only `0`, `1`, or boolean.
- A local operator controls credentials. The proxy logs route/counter evidence only, strips
  hop-by-hop headers, remains bound to loopback, and never logs query values or request bodies.
- A local filesystem recipient receives downloaded bytes. Existing safe-basename and explicit
  output-directory controls remain unchanged; this change only determines whether bytes are
  obtained inline or by the established re-fetch.

### Out of scope

The design does not authenticate arbitrary proxy clients, alter Bugzilla authorization, accept
signed/fractional identifiers, or harden the test proxy for non-loopback deployment. Those are not
newly reachable in the shipped binary.

## Verification

- focused type, attachment-client, comment-client, and attachment-command tests with controlled
  red/green proof;
- `python3 tests/functional/redhat-shape-proxy.py --self-test`;
- focused functional execution during iteration, followed by `make functional-test-all`;
- `make test-fast`, `make lint`, and `make test`.
