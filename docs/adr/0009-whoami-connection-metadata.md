# 0009 — `whoami` JSON carries connection metadata via a flatten wrapper

Status: Accepted

## Context

Issue #470 asks `bzr whoami --json` to report, in one call, the resolved server
name and the auth mode (API key vs. anonymous) alongside the identity fields the
Bugzilla server returns.

`WhoamiResponse` (`src/types/user.rs`) is the `Deserialize` target for the
server's `/rest/whoami` body (`id`, `name`, `real_name`, `login`). The two new
values are **connection facts**, not identity facts: the server never sends
them; `bzr` resolves them locally (`server_name` during connection-target
resolution, the credential during client construction). They are already known
to `BugzillaClient`, which holds the base URL, resolved API mode, and credential.

The published `--json` contract is closed (`additionalProperties: false`,
enforced by the schema drift test), so the output type must serialize to exactly
the documented keys.

## Decision

1. **Connection metadata is owned by `BugzillaClient`, not `WhoamiResponse`.**
   The client gains a `server_name: String` field (passed via
   `BugzillaClientConfig`) and an `auth_mode()` accessor derived from credential
   presence (`api_key.is_some()` → `ApiKey`, else `Anonymous`). `WhoamiResponse`
   stays a pure server-body deserialization type — no placeholder
   `#[serde(default)]` fields for data the server never sends.

2. **A `WhoamiOutput` wrapper composes identity + connection metadata using
   `#[serde(flatten)]`**, so the wire shape is one flat object
   (`{id, name, real_name, login, server_name, auth_mode}`) and the identity
   fields keep their names/types/positions (additive per the stability policy).

3. **`auth_mode` reuses the `server capabilities` `auth_modes` vocabulary** —
   `"api_key"` / `"anonymous"` — via a new `AuthMode` enum in
   `src/types/transport.rs`, rather than inventing a parallel set of strings.

## Consequences

- One published schema (`schemas/whoami.json`, closed) describes the flat shape;
  the drift test pins it to `WhoamiOutput`'s serialization.
- `BugzillaClientConfig` grows one field; every construction site (real and test
  helpers) passes a server name — a small, mechanical, compile-checked change.
- `auth_mode` is honest about client state. Because `whoami` requires
  credentials today, it is `"api_key"` on every current success; the
  `"anonymous"` variant exists so the contract stays correct if that gate is
  ever relaxed, and so the same enum can be reused by other identity-adjacent
  output.

## Considered & rejected

- **Add `auth_mode` / `server_name` directly to `WhoamiResponse`.** Rejected:
  forces `#[serde(default)]` placeholders on a type whose whole purpose is to
  deserialize the server body, and conflates server identity with local
  connection state. The flatten wrapper keeps the two concerns separate at zero
  wire-shape cost.

- **Carry the values on `CommandContext` and read them in the command.** Rejected:
  the resolved server name and credential live behind `connect_and_configure`;
  the context only knows the *requested* `--server` flag (often `None` for the
  default server) and never the resolved inline/default name. The client is the
  component that actually resolved them.

- **Model `auth_mode` on `AuthMethod` (`header` / `query_param`).** Rejected:
  `AuthMethod` is the credential *transport*, irrelevant to "am I authenticated".
  #470 asks for "API key vs. anonymous", which the `server capabilities`
  `auth_modes` vocabulary already names.
</content>
