# ADR 0077: Use Bearer API keys for Red Hat Bugzilla REST

## Status

Accepted

## Context

Red Hat Bugzilla's current WebService documentation specifies API-key requests
with `Authorization: Bearer YOURAPIKEY`. bzr currently selects only the
standard `X-BUGZILLA-API-KEY` header or `Bugzilla_api_key` query parameter for
REST, while XML-RPC deliberately sends its key in the protocol body. Treating
an arbitrary server as Red Hat based on a substring would send a credential to
an insufficiently validated destination.

## Decision

For a REST base URL whose parsed host is exactly `bugzilla.redhat.com`, bzr
will use a validated `Authorization: Bearer <api-key>` header for detection,
credential proof, and normal REST requests. The policy is applied at request
construction, so a persisted or user-pinned standard auth method cannot bypass
the host check. All other hosts retain the existing header/query behavior.
XML-RPC remains unchanged and continues to carry the key in its XML-RPC body.

## Consequences

Red Hat REST API-key authentication follows the documented production
transport, while lookalike hosts cannot select Bearer. The key remains covered
by the existing active-key redaction seam. Tests must cover exact-host
selection, suffix-confusion rejection, detection and request application; the
comparison fixture proves the wire-level Bearer header.

## Considered & rejected

- **Use Bearer for every `.redhat.com` suffix.** judgment: the documented
  production endpoint is exact, and a broader credential destination policy
  adds trust surface without satisfying another sourced criterion.
- **Expose an unguarded `bearer` auth-method setting.** judgment: a manually
  selectable transport would bypass the policy that protects credential
  routing.
- **Keep standard API-key transport.** verified: Red Hat Bugzilla's current
  WebService documentation specifies `Authorization: Bearer YOURAPIKEY` for
  REST API keys.
- **Change XML-RPC transport too.** verified: `src/xmlrpc/protocol.rs` owns
  XML-RPC API-key body encoding; issue #678 excludes it from this REST change.
