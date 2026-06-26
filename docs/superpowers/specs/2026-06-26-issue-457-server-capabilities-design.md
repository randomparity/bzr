# Issue #457: `bzr server capabilities` Design

## Context

Agents that drive `bzr` infer what a Bugzilla instance supports by probing:
try a REST call and fall back to XML-RPC, call `field list status` to learn
transitions, attempt a flag and parse the rejection. There is no single command
that answers "this server does X, doesn't do Y" so an agent can self-configure
its workflow without trial-and-error.

`server info` already reports *identity* (version + extensions). Issue #457 asks
for a sibling `server capabilities` that reports the *behavior surface* an agent
needs to plan mutations, emitted as a structured `--json` / `--output ndjson`
document with a published schema.

The constraint that shapes the whole design: criterion 2 requires the command to
work against `--server-url` with **no saved config and no API key**. Whatever the
document reports must therefore be derivable from what a stock Bugzilla 5.x server
serves anonymously, or explicitly marked absent. The acceptance test confirms this
expectation — it mocks only `/rest/version` and `/rest/field/bug/status`, not the
admin-gated `/rest/parameters` or any flag endpoint.

## Decision

Add `ServerAction::Capabilities`. It connects with the existing
`connect_and_configure()` path (credentialless-capable, per issue #380), fetches
only anonymously-available data, and assembles a `ServerCapabilities` value that
serializes to the documented shape. Fields that a stock anonymous server does not
expose are `null` (best-effort fields) rather than errors.

### Output shape

```json
{
  "version": "5.0.4.t112",
  "api_modes": ["rest", "xmlrpc"],
  "auth_modes": ["api_key"],
  "max_attachment_size": null,
  "status_transitions": [
    {"from": "NEW", "can_change_to": ["ASSIGNED", "RESOLVED"]}
  ],
  "flag_types": null,
  "custom_fields": [
    {"name": "cf_cust_field", "type": "freetext", "values": []}
  ],
  "supports_comments": true,
  "supports_attachments": true,
  "supports_history": true,
  "supports_flag_requests": true
}
```

### Field-by-field data path and falsifiable semantics

- **`version`** — `GET /rest/version`, reused verbatim from `server_version()`.
  The raw Bugzilla version string. Never null on a reachable server.

- **`api_modes`** — derived from the already-detected `ApiMode` (no extra
  request), via the existing version threshold logic:
  - `ApiMode::Rest` → `["rest"]`
  - `ApiMode::Hybrid` → `["rest", "xmlrpc"]`
  - `ApiMode::XmlRpc` → `["xmlrpc"]`
  This reports what transports the server *supports*, not just the one bzr chose.

- **`auth_modes`** — the auth mechanism the **server accepts**, not bzr's local
  credential storage. Bugzilla 5.x REST accepts API-key auth (header or query
  param, modeled by `AuthMethod`); both reduce to the single capability
  `"api_key"`. Value: `["api_key"]` for any server bzr can speak REST/hybrid to
  (version ≥ 5.0). For a pure XML-RPC (`< 5.0`) server, API keys may be absent, so
  the value is `[]`. This is a server-capability statement, independent of whether
  the local config holds a credential — so it is identical under `--server-url`
  with no key.

- **`status_transitions`** — reuses the `field list status` data path:
  `get_field_values("status")` → `/rest/field/bug/bug_status`. Each returned
  `FieldValue` with a non-null `name` and a `can_change_to` list becomes
  `{"from": <name>, "can_change_to": [<transition names>]}`. Values with a null
  name (the "unset" pseudo-entry) and values lacking `can_change_to` are skipped.
  Empty list if the server returns no transitions.

- **`custom_fields`** — `GET /rest/field/bug` (no field name → all fields),
  filtered to `is_custom == true`. Each becomes `{name, type, values}` where
  `type` is the human-readable name of Bugzilla's integer field-type enum
  (1→`freetext`, 2→`single_select`, 3→`multi_select`, 4→`textarea`, 5→`datetime`,
  6→`bug_id`, 7→`bug_urls`, 8→`keywords`, 9→`date`, 10→`integer`, 0/other→
  `unknown`) and `values` is the field's legal-value names (empty for free-form
  types). Empty list when the server has no custom fields.

- **`max_attachment_size`** — best-effort. `GET /rest/parameters` and read
  `parameters.maxattachmentsize` (Bugzilla reports this in **kilobytes**; the raw
  integer is passed through unchanged). The endpoint is admin/permission-gated, so
  anonymously it usually fails or omits the key. **Any failure → `null`**; this
  fetch never fails the command. Value is the raw server integer when present.

- **`flag_types`** — `null` in this version. Bugzilla exposes no global flag-type
  REST endpoint; flag types are per-product and only appear inside product detail
  responses on some releases. There is no anonymous, server-wide data path, so the
  key is published (for forward-compatibility with the schema) but always `null`
  until a per-product path is added. The key is present so agents can branch on
  `flag_types !== null` once it lands.

- **`supports_comments` / `supports_attachments` / `supports_history` /
  `supports_flag_requests`** — capability booleans derived from the detected
  transport. Any Bugzilla bzr can connect to over REST or hybrid (version ≥ 5.0)
  supports all four through the REST API, so they are `true` when `api_modes`
  contains `"rest"`. For a pure XML-RPC (`< 5.0`) server these REST-surfaced
  capabilities are reported `false` (bzr cannot drive them over the chosen REST
  surface). They are derived, not probed, and documented as such.

The contract choices above (auth_modes semantics, nullable hard fields, the
failure-class split) are recorded in
[ADR-0005](../../adr/0005-server-capabilities-contract.md).

### Graceful degradation contract

The command performs three classes of fetch:

1. **Required** — `version` + the `ApiMode` already resolved by
   `connect_and_configure`. If `version` fails, the whole command fails with the
   underlying transport error (the server is unreachable; nothing is knowable).
2. **Best-effort, per-field** — `status_transitions`, `custom_fields`. A failure
   of one of these is surfaced as an error, because criterion 1 requires the
   documented shape for a stock server and criterion 6's test exercises exactly
   these paths. They are not silently nulled. (`NotFound` for the status field on
   an exotic server degrades to an empty `status_transitions` list rather than a
   hard error, since absence of transitions is a real, representable state.)
3. **Optional** — `max_attachment_size`. Always degrades to `null` on any error.

This split is deliberate: "degrade gracefully" (criterion 3) means *fields the
server doesn't expose are null/omitted*, not *swallow every error*. A server that
cannot answer `/rest/field/bug` is not a stock 5.x server and the agent should see
that failure, not a misleadingly-empty document.

### Layering

- `src/types/capabilities.rs` — the `ServerCapabilities` data model and the
  per-field sub-structs (`StatusTransitionSummary`, `CustomFieldSummary`,
  `FlagTypeSummary` reserved for the future flag path), plus the field-type-enum
  → name mapping. Pure data + serialization; no I/O.
- `src/client/resources/server.rs` — a new `server_capabilities()` method that
  orchestrates the fetches and returns `ServerCapabilities`. It composes existing
  `server_version()` and `get_field_values()` plus two new thin fetchers
  (`all_bug_fields()` for custom fields, `attachment_size_limit()` best-effort).
- `src/commands/server.rs` — dispatch the new action.
- `src/output/resources/server.rs` — `write_server_capabilities()` rendering
  `--json` / `--output ndjson` verbatim and a readable table for the default
  format.

No new generic abstraction: each fetcher is a concrete method, mirroring the
existing `server_info()` composition.

## Schema

`schemas/server-capabilities.json` (JSON Schema, draft the other schemas use)
added to the `SCHEMAS` registry in `src/commands/schema.rs`. `max_attachment_size`
and `flag_types` are nullable (`["integer","null"]` / `["array","null"]`).
`schema_tests.rs` gains a maximally-populated `ServerCapabilities` sample asserted
against the schema by the existing `assert_conforms` drift check, so a contract
change fails CI until the schema is updated.

## Testing

Failing-first tests, at the boundary the project prescribes (wiremock for the
client method, direct serialization for the writer/schema):

- **Client (wiremock):** mock `/rest/version` (→ `5.0.4`) and
  `/rest/field/bug/bug_status` (→ NEW/ASSIGNED/RESOLVED with `can_change_to`);
  assert the assembled `ServerCapabilities` has the expected `version`,
  `api_modes`, `auth_modes`, and `status_transitions`. (Criterion 6.)
- **Custom fields:** mock `/rest/field/bug` returning one `cf_*` field with a
  type integer and one built-in field; assert only the custom field appears with
  the mapped type name.
- **max_attachment_size degradation:** mock `/rest/parameters` → 401; assert the
  field is `null` and the command still succeeds.
- **status field absent:** mock `/rest/field/bug/bug_status` → empty `fields`;
  assert `status_transitions` is `[]`, not an error.
- **Field-type mapping:** unit-test each integer → name (and the unknown branch).
- **api_modes / supports_* derivation:** unit-test each `ApiMode` → vectors and
  booleans, including the XML-RPC `auth_modes == []` / `supports_* == false` edge.
- **Writer:** `--json` round-trips the documented shape; table render is stable.
- **Schema drift:** `assert_conforms("server-capabilities", &sample)`.

## Documentation & skills

- `docs/bzr-cli.md` — new `### bzr server capabilities` section with the shape,
  the anonymous/`--server-url` example, and the null-degradation note.
- `agent-skills/.../commands.yml` — `server: info capabilities`.
- `agent-skills/skills/bzr-setup/SKILL.md` — teach `server capabilities` as the
  "what can I do here?" probe an agent runs before planning mutations.
- `CHANGELOG.md` — entry under the unreleased section.
- `make skills-test` drift check passes.

## Out of scope

- Per-product `flag_types` population (the key ships as `null`; a later issue adds
  the data path).
- Authenticated enrichment of `max_attachment_size` (no credential-gated fetch is
  added; best-effort anonymous only).
- XML-RPC-transport capability probing beyond the version-derived booleans.
- Caching the capabilities document; it is computed per invocation like
  `server info`.
