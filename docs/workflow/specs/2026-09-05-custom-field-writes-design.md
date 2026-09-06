# Server-validated arbitrary field writes design

Issues: #283 (design), #671 (parity gap). Decision record: ADR 0053.

Out of scope: `bug clone` and saved bug templates (issue #712).

## Outcome and scope

`bzr bug create` and `bzr bug update` accept repeatable `--field KEY=VALUE` and
`--field-json <PATH|->`. Every supplied key is checked against the server's bug-field
catalogue before dispatch. An undeclared key fails locally at exit 7; a catalogue probe
that fails refuses the write with a distinguishable diagnostic. `--from-json` document
shapes stay strict and no published schema changes, so `SCHEMA_VERSION` is not bumped.

## CLI surface

Added to `CreateFieldArgs` (`src/cli/bug/mod.rs`, used by `bug create`) and to `UpdateArgs`
(`src/cli/bug/update.rs`):

- `--field <KEY=VALUE>` — repeatable. Splits on the first `=`. The value is a JSON string.
  `--field key=` sets the empty string, which clears the field on Bugzilla.
- `--field-json <PATH>` — a JSON object; `-` reads stdin. Values may be any JSON type.

`--field` is a new long name on both verbs; no existing subcommand defines it, so there is no
clap global-name collision.

Neither flag is added to `CloneCreateFieldArgs`.

## Merge and rejection rules

Merged into one `BTreeMap<String, serde_json::Value>` (deterministic key order). Rejected with
`InputValidation` (exit 7):

| Condition | Message names |
|---|---|
| `--field` value with no `=` | the offending argument |
| empty or whitespace-only key | the offending argument |
| same key twice across `--field`/`--field-json` | the duplicated key |
| `--field-json` payload is not a JSON object | the source path |
| `--field-json` path unreadable or not UTF-8 | the source path |
| key already present in the serialized typed payload | the key and its dedicated flag |

The typed-collision check serializes the built `CreateBugParams`/`UpdateBugParams` (before
extras are attached) with `serde_json::to_value` and intersects its object keys with the extra
keys. This tracks `skip_serializing_if`, so `--field whiteboard=x` is accepted when
`--whiteboard` was not given and rejected when it was, and it needs no hand-maintained
reserved-key list that could drift from the structs.

## Catalogue validation

New client method in `src/client/resources/field.rs`:

```rust
pub(crate) async fn bug_field_names(&self) -> Result<Vec<String>>
```

Calls `field/bug?include_fields=name`, returns sorted deduplicated names. It reuses the
existing `FieldBugResponse`/`FieldDefinition` types — every field other than `name` already
carries `#[serde(default)]`, so a names-only response deserializes unchanged. `all_bug_fields()`
stays `pub(super)`; a names-only fetch is what this path needs and it keeps the response small
on installations with hundreds of fields.

New runtime module `src/commands/runtime/shared/field_catalogue.rs`:

```rust
pub(crate) async fn validate_bug_fields(
    client: &BugzillaClient,
    ctx: &CommandContext,
    keys: &BTreeSet<String>,
) -> Result<()>;

pub(crate) async fn connect_and_validate_bug_fields(
    ctx: &CommandContext,
    keys: &BTreeSet<String>,
) -> Result<BugzillaClient>;
```

`validate_bug_fields` first drops every key that is a REST bug field bzr models —
`BUG_FIELDS` in `src/types/bug/fields.rs`, matched on `BugField::canonical()`. Bugzilla's
catalogue reports internal column names for many built-ins (`status_whiteboard`,
`short_desc`, `rep_platform`, `bug_file_loc`, `blocked`) while the write API takes the REST
names, so a catalogue-only check rejects `--field whiteboard=...`. If nothing is left, it
returns without a network call. Otherwise:

1. Load `ServerConfig.bug_field_names` for `client.server_name()` from the config at
   `ctx.config_path_override()`. If every remaining key is present, return.
2. Probe `client.bug_field_names()`. On failure, return the probe error annotated (below).
3. Persist the probed names under the config lock via `Config::update_locked_at`, following
   `persist_detected_settings` (`src/commands/runtime/shared/connection/detect.rs:38`): only
   after a successful probe, and a no-op when the server is not in config (inline `--server-url`
   or a concurrently removed entry). Persistence never fails the caller — a write error is
   logged and stepped over — and a catalogue above `MAX_CACHED_FIELD_NAMES` (4096) is not
   cached at all.
4. Any key still absent → `BzrError::input_field` naming the field, with
   "run `bzr field list` to see the fields this server declares".

Probe-failure annotation preserves the error's class and exit code, following
`annotate_search_fallback` (`src/client/resources/bug.rs:75`):

```
<original message> (could not validate --field keys: the server's bug field
catalogue was not retrieved, so no changes were sent)
```

with arms for `Api`, `HttpStatus`, `Auth`, `Deserialize`, and `other => other`. A raw
transport error (`Http`) propagates unchanged at exit 5; it already names the connection
failure, and the write is still refused.

## Config state

`ServerConfig` (`src/config/model.rs`) gains:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub bug_field_names: Option<Vec<String>>,
```

alongside `auth_method` / `api_mode` / `server_version`. Absent by default; written only by a
successful probe.

## Payload plumbing

`CreateBugParams` gains `pub extra_fields: BTreeMap<String, Value>`; `CreateBugParamsWire`
gains `#[serde(flatten)] extra: &'a BTreeMap<String, Value>` as its last field.

`UpdateBugParams` gains `#[serde(flatten)] pub extra_fields: BTreeMap<String, Value>`. An empty
map contributes no keys, so `is_empty()` (which round-trips through `serde_json::to_value`)
continues to be the single source of truth for "no changes requested" and now counts extras as
changes for free.

`JsonCreateBug` and `BugUpdateDraft` gain `#[serde(skip)] extra_fields: BTreeMap<String, Value>`
so the CLI overlay can carry extras through the `--from-json` path while
`deny_unknown_fields` keeps an `extra_fields` key out of the document itself.

## Dispatch sites

`connect_and_validate_bug_fields` replaces `connect_and_configure` at the three create sites
(`create.rs::create_and_report`, `create_json.rs::create_batch_from_json`,
`compound.rs::create_with_sub_steps`); for a batch the key set is the union across elements.
`apply_checked_connected` (`update/execute.rs:111`) calls `validate_bug_fields` before
`apply_connected`, which covers the flag path, `--from-json`, and the convenience verbs
through the one function they all reach.

`--dry-run` returns before any of these, so it neither connects nor validates.

## Verification

Unit (sibling `*_tests.rs`): parse and rejection table above; wire serialization of extras on
both payload types; `UpdateBugParams::is_empty()` false with only extras; cache hit avoids the
probe; cache miss probes and persists; undeclared key after a fresh probe is exit 7; probe
failure is refused, annotated, and exit-code-preserving; no request is issued on either
refusal (wiremock `expect(0)` on the mutation endpoint).

Functional (`tests/functional/phases/08c-bugs-create-fields.sh`, extended): create with
`--field`, update with `--field`, `--field-json`, an undeclared key at exit 7 with no bug
change, and the credentialless path.

Comparison: remove `lifecycle_expect_gap 671` and the two `lifecycle_bzr_gap` diagnostics at
`tests/functional/compare/01-bug-lifecycle.sh:485,494,504`; flip the `Generic arbitrary fields`
row to parity in `docs/dev/python-bugzilla-parity.md:14` and in its byte-identical copy in
`tests/functional/pybz/container-tests.sh`; update that file's gap/pass counts, its
`arbitrary-fields-create` stub arm, its `run_partial_stale_gap_control` for 671, its
eligibility control, and its stale-gaps issue loop.

Docs: `docs/bzr-cli.md` command tree and flag documentation for both verbs; the
`JsonCreateBug` doc comment at `src/commands/bug/create_json.rs:22`, which currently cites
#283 as the reason `deny_unknown_fields` exists.

## Guardrails and architecture context

Host `arm64`; no target architecture declared by project instructions. Gates: `make lint`,
`make test`, `make functional-test`, `make functional-compare-all`.
