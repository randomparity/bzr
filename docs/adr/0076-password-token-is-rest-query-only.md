# ADR 0076 — Password tokens are REST query-only credentials

## Status

Accepted (2026-09-12)

## Context

Issue #676 adds storage and use of Bugzilla login tokens. Existing bzr
credentials are API keys: REST can detect header or `Bugzilla_api_key` query
transport, while XML-RPC always embeds `Bugzilla_api_key` in its request body.
A login token has a different REST parameter name, `Bugzilla_token`.

## Decision

Store a token as a credential kind distinct from API keys. It is mutually
exclusive with API-key sources and uses `Bugzilla_token` only for REST
requests. A token forces REST for the default detected path; explicit XML-RPC
or hybrid overrides are rejected rather than falling back through the API-key
XML-RPC adapter. A token also bypasses API-key alternate-auth retries.
Redaction tracks either credential kind and both query parameter names.

## Consequences

Token-backed servers work for REST surfaces on all supported container versions
and cannot silently send their token as `Bugzilla_api_key`. API-key detection,
proof, cache state, alternate-auth retry, and XML-RPC behavior remain
unchanged. Users needing XML-RPC token support need a separate, explicitly
designed protocol change.

## Considered & rejected

- **Reuse the API-key credential kind.** verified: `src/xmlrpc/protocol/client.rs`
  writes the literal `Bugzilla_api_key` XML-RPC parameter, which is not the
  token contract.
- **Use the detected Hybrid mode for tokens.** verified: Bugzilla 5.0 detects
  as Hybrid and hybrid can invoke the XML-RPC adapter after a REST transport
  failure; judgment: defaulting token connections to REST preserves supported
  container coverage without changing the credential's wire meaning.
