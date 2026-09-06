# Server-side saved search design

- Issue: #670
- Scope token: `q670-1c1b8eb2`
- Decision: [ADR 0052](../../adr/0052-detect-vendor-extension-support-before-dispatch.md)
- Branch: `feat/saved-search-670`
- Base branch: `main`

An earlier revision of this spec designed a pass-through-and-document approach. The operator
rejected it in favour of detect-and-error; ADR 0052 records both and why.

## Outcome

`bzr bug search --saved-search <NAME> [--sharer <ID>]` runs a saved search stored in the
Bugzilla account. Because that is a Red Hat extension, bzr establishes support before
dispatching and refuses with an actionable error when the server does not have it.

Unrelated to bzr's local saved queries (`bzr query`), which are untouched.

## Verified server contract

Established before design, against the project's own functional images (5.0.6, 5.2, 5.3.3+):

- **Upstream cannot resolve these parameters.** Neither `savedsearch` nor `sharer_id` appears
  anywhere under `Bugzilla/` in any image; `Bug.search` passes its parameter hash into
  `Bugzilla::Search->new`, which ignores unknown keys. No `Bugzilla/WebService/` module
  references the `namedqueries` table.
- **Upstream accepts them silently.** `GET /rest/bug?savedsearch=…&sharer_id=1` returned
  output byte-identical to an unfiltered search on all three images. An XML-RPC `Bug.search`
  carrying the same member — authenticated as the owner of a query seeded to match nothing —
  returned the same. So a pass-through would have produced a wrong answer with exit 0.
- **The capability signal exists and discriminates.** `bugzilla.redhat.com/rest/extensions`
  advertises `"RedHat": {"version": "0.3"}`; all three images return `{"extensions":{}}`. The
  endpoint answers HTTP 200 **unauthenticated**, so the check works on the credentialless path.

The `RedHat` key is a **proxy** for a patched `Bug.search`, not proof of one — see ADR 0052's
consequences, which record the false negative this accepts.

## Capability detection

`BugzillaClient::server_extensions()` already exists (`src/client/resources/server.rs:39`) and
issues `GET /rest/extensions`; nothing new is built for the probe itself.

The result joins the per-server detection state that `DetectedServerSettings` already carries.
That struct documents exactly the semantics this needs for `server_version`: `Some` when the
endpoint responded, `None` on transient failure, and callers persist only when it is `Some`
(`src/client/auth/mod.rs:116`, honoured by `persist_detected_settings` at
`src/commands/runtime/shared/connection/detect.rs:38-44`). `extensions: Option<Vec<String>>`
follows that contract unchanged, and `ServerConfig` gains a matching
`server_extensions: Option<Vec<String>>` beside `auth_method`, `api_mode` and `server_version`.

**Where the gate lives.** `connect_and_configure` returns only a `BugzillaClient`, so the
command layer cannot see what detection found. Rather than widen that signature — it is called
by every command — the check goes in a new command-layer helper beside the other shared runtime
helpers:

```rust
// src/commands/runtime/shared/capability.rs
pub(crate) async fn require_server_capability(
    ctx: &CommandContext,
    client: &BugzillaClient,
    capability: &str,
    operation: &str,
) -> Result<()>
```

It reads the cached list from `ServerConfig` via `Config::resolve_server(ctx.server())`, probes
through the client on a cache miss, persists what it learned, and decides. One helper, reusable
by the sibling issues that ADR 0052 governs, and no ripple through unrelated commands.

**Inline servers have no cache.** A `--server-url` connection resolves to `INLINE_SERVER_NAME`
with no config entry, so there is nothing to read from and nothing to write to. The helper
probes every time in that case and persists nothing. This is the path the credentialless
functional test exercises, and it works because `/rest/extensions` answers unauthenticated.

Three outcomes, and they are deliberately three rather than two:

| Probe | Extension | Behaviour |
|---|---|---|
| responded | `RedHat` present | dispatch the search |
| responded | `RedHat` absent | refuse: unsupported capability, exit 15 |
| failed | unknown | refuse: capability undetermined, exit 15, naming the transport failure |

Collapsing the third into the second would let a transient network fault masquerade as a
statement about the server. ADR 0015 already forbids bzr masking what the server actually did.

## Error contract

No existing `BzrError` variant fits: `InputValidation` blames well-formed input, `Api`/`XmlRpc`
mean the server reported a fault when it reported nothing, and `NotFound`/`Config`/`Auth`
describe unrelated conditions. A new variant is added:

```rust
UnsupportedServerCapability { capability: String, detail: String }
```

with `EXIT_CODE_UNSUPPORTED_CAPABILITY = 15` (codes 2–14 are contiguous and full) and
`error_type()` `"unsupported_server_capability"`. `schemas/error.json` raises its `exit_code`
`maximum` from 14 to 15 and gains a `capability` property. Both are additive, so
`SCHEMA_VERSION` goes `3.0.1` → `3.0.2` — the same bump sibling issue #672 makes, on the same
unreleased cycle; the orchestrator reconciles that one line at serial merge.

Messages follow the repository's operation/input/fix rule, e.g.

```
saved search 'triage': server does not implement the Red Hat saved-search extension
(no 'RedHat' extension advertised at /rest/extensions). Stock Bugzilla does not support
savedsearch; use `bzr bug list` filters or `bzr query` for a local saved query.
```

## CLI contract

| Flag | Type | Constraint |
|---|---|---|
| `--saved-search <NAME>` | `Option<String>` | conflicts with the positional `<QUERY>` and `--from-url` |
| `--sharer <ID>` | `Option<u64>` | requires `--saved-search` |

`--saved-search` conflicts with the positional quicksearch string for a verified reason:
`Bug.search` replaces its whole parameter hash when `quicksearch` is present
(`$match_params = $cgi->Vars`), so the server would discard the saved-search name without a
diagnostic. It conflicts with `--from-url` because that is a complete alternate query source.

`--sharer` is a numeric Bugzilla user id, matching Red Hat's documented payload
(`sharer_id: 112233`) and python-bugzilla's `--savedsearch-sharer-id`. Accepting a login would
need a `User.get` round trip whose result is usable only where the extension exists, and would
silently diverge from the tool this campaign measures parity against.

Paging, projection, sorting and `--count` compose unchanged.

## Wire mapping

`SearchParams` gains `saved_search: Option<String>` and `sharer_id: Option<u64>`.
REST appends `savedsearch` to the existing `append_option_params` string table and `sharer_id`
beside `limit`/`offset`. XML-RPC appends `savedsearch` to its `option_fields` table and
`sharer_id` through the existing `xmlrpc_id` range check. Both omit an absent parameter.

`saved_search` joins `has_filters()` (a consistency invariant — the predicate has no production
caller) and `has_structured_filters()` (behavioural: it keeps the hybrid XML-RPC retry
available for the one vendor extension bzr sends, at the cost of one capped round trip on a
result that was already empty).

## Comparison harness

The gap **stays**, because bzr genuinely cannot do this on any supported image — it now says so
with exit 15 instead of a parse error. The parity row is reworded rather than flipped: bzr
errors where python-bugzilla returns unfiltered results, which is bzr behaving better, not a
gap in capability.

One mechanical consequence: `lifecycle_bzr_probe` admits a gap only at `BZR_EXIT -eq 2` with an
exact diagnostic match (`tests/functional/compare/01-bug-lifecycle.sh:53-56`), so an exit-15
failure would fall through to `test_fail`. The probe gains an optional expected-exit parameter
defaulting to 2, leaving every other block's behaviour identical, and the saved-search block
passes 15 with the new diagnostic. That is the minimum required to keep the marker the
orchestrator asked to keep.

The python-bugzilla side of that comparison remains vacuous — it returns an unfiltered search
that happens to match a two-bug corpus. Recorded, not fixed here; the Red-Hat-shaped fixture is
issue #710.

## Testing

- **Unit.** Wiremock proves both transports carry the parameters when the extension is
  present, and that an absent extension refuses before any `/rest/bug` request is made (the
  mount for it is never satisfied). Clap tests cover the two conflicts, the `requires`
  relation, and the non-numeric `--sharer` rejection. An error test pins exit 15 and the
  `error_type`.
- **Functional** (`tests/functional/phases/08f-bug-saved-search.sh`). Against a real container,
  which lacks the extension, the flag must exit 15 with a message naming the capability —
  that is the primary path here, not an edge case. Also the credentialless variant, `--count`
  composition, and the four parse-time rejections. The success path cannot be exercised: no
  supported image implements the extension, and the phase header says so.
