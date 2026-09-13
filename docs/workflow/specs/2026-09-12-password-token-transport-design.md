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
does not write an API-key auth-method cache, and uses REST even when anonymous
version detection cached Hybrid for Bugzilla 5.0. An explicit `--api xmlrpc` or
`--api hybrid` request is rejected before a request could put the token into
XML-RPC's API-key field. This makes the unsupported transport boundary explicit
without making the default supported-server path unusable.

The prepared-auth representation has a distinct token variant. Its requests
always carry `Bugzilla_token` and never take the API-key alternate-auth retry
on a 401. API-key query credentials retain that retry behavior.

The redaction layer becomes credential-neutral: it tracks the active secret,
redacts both API-key and token query markers in raw diagnostics, and preserves
the existing bounded-preview protection for either secret. User-facing
credential-required errors describe the accepted configured sources without
printing their values.

Configuration display must mask and identify the token as a token rather than
an API key. Existing keyring commands remain API-key-only: migration rejects a
token source, and setting an API-key keyring source removes a token so it never
persists an invalid multi-source server. Token keyring storage is lifecycle
work owned by #681, not a silent reinterpretation of existing keyring commands.

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

Unit tests cover source exclusivity and token resolution, configuration-display
masking, keyring/migration safety, token request placement, API-key
non-regression, REST-only rejection, no API-key fallback after a token 401, and
token redaction including bounded previews. Connection tests prove tokens
bypass API-key detection/cache persistence and force REST despite a detected
Hybrid mode. The auth comparison phase replaces #676's controlled expected gap
with a real configured-token REST identity check and the parity report marks
that row as parity. The phase obtains a disposable token from the existing
real-container `/rest/login` fixture credentials into a private 0600 file,
writes it directly to a private test config, and never prints either file or
token. This is test fixture setup, not a login command or comparison-harness
change. `make functional-compare` exercises that comparison against a real
Bugzilla container; `make functional-test` remains the separate regression arm.

## Alternatives considered

- **Represent tokens as API keys.** Rejected: the REST parameter name and
  XML-RPC semantics differ, so the representation would send a token through
  unsupported paths and make redaction incomplete.
- **Add token flags or login commands now.** Rejected: issue #676 explicitly
  excludes new commands; #681 owns token lifecycle and CLI surface.
- **Permit hybrid fallback.** Rejected: XML-RPC's existing adapter inserts
  `Bugzilla_api_key`, not `Bugzilla_token`, so fallback would change a token's
  meaning.
