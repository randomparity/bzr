# Password-token transport design

## Goal

Allow a named server configuration to hold a previously-issued Bugzilla login
token and send it to REST endpoints as `Bugzilla_token`, without changing API
key configuration or adding login/logout commands.

## Scope and constraints

This implements issue #676 only. The source is a persisted `token` value used
by the later login/logout work; token acquisition, invalidation, bugzillarc
import, Bearer transport, client certificates, and comparison-harness changes
remain owned by their respective issues. Existing `api_key`, `api_key_env`,
and `api_key_keyring` configuration remains compatible. The Rust toolchain is
1.89.0; no dependency is added. The existing single-thread Tokio runtime and
test-sibling layout remain unchanged.

## Design

`ServerConfig` gains an optional inline `token` field. It is mutually exclusive
with every API-key source, so a server always resolves to exactly one
credential kind. The credential boundary returns both the secret and its kind,
rather than letting downstream code infer a token from field names. This keeps
configuration validation, runtime resolution, redaction registration, and
transport selection aligned.

API-key credentials retain their detected header/query behavior and remain
available to XML-RPC exactly as before. A token selects fixed REST query
transport with `Bugzilla_token`; it does not run API-key method detection,
does not write an API-key auth-method cache, and rejects an XML-RPC or hybrid
override before a request could put the token into XML-RPC's API-key field.
This makes the unsupported transport boundary explicit instead of silently
mislabeling a token as an API key.

The redaction layer becomes credential-neutral: it tracks the active secret,
redacts both API-key and token query markers in raw diagnostics, and preserves
the existing bounded-preview protection for either secret. User-facing
credential-required errors describe the accepted configured sources without
printing their values.

## Auth detection and proof

API-key detection and `valid_login` proof retain their current behavior. Token
connections skip API-key transport probing because `Bugzilla_token` has one
defined REST placement. `whoami` and other REST operations use that placement;
the existing email requirement for the legacy `valid_login` proof remains an
API-key proof contract and is not repurposed as token detection.

## Threat model

### Boundaries and controls

| Boundary | Actor/input | Control |
|---|---|---|
| Config file to credential resolver | Local operator-controlled token | Existing config permission hardening; exactly-one-source validation; no secret in errors. |
| Resolver to HTTP request | Resolved token | Fixed `Bugzilla_token` query parameter only for REST; XML-RPC/hybrid rejected. |
| HTTP/server diagnostics to terminal or logs | Server/proxy response may echo URL | Marker redaction and active-secret replacement before display or trace output; bounded previews avoid retaining a split secret. |

### Actors

The local operator controls persisted configuration. A Bugzilla server or
intermediary can echo request URLs and bodies into diagnostics; it is therefore
treated as untrusted for secret disclosure. Remote callers do not directly
control this CLI's configuration.

### Out of scope

This change does not obtain, refresh, revoke, or migrate tokens, authenticate
over XML-RPC, implement Bearer auth, or protect a token intentionally written
to an insecure local config file. Those behaviors are either separate owned
work or existing configuration-policy concerns.

## Test strategy

Unit tests cover source exclusivity and token resolution, token request
placement, API-key non-regression, REST-only rejection, and token redaction
including bounded previews. Connection tests prove tokens bypass API-key
detection/cache persistence. The auth comparison phase replaces #676's
controlled expected gap with a real configured-token REST identity check and
the parity report marks that row as parity. The functional run exercises the
compiled binary against a real Bugzilla container.

## Alternatives considered

- **Represent tokens as API keys.** Rejected: the REST parameter name and
  XML-RPC semantics differ, so the representation would send a token through
  unsupported paths and make redaction incomplete.
- **Add token flags or login commands now.** Rejected: issue #676 explicitly
  excludes new commands; #681 owns token lifecycle and CLI surface.
- **Permit hybrid fallback.** Rejected: XML-RPC's existing adapter inserts
  `Bugzilla_api_key`, not `Bugzilla_token`, so fallback would change a token's
  meaning.
