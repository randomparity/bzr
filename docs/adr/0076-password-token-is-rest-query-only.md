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
requests. Token configurations reject XML-RPC and hybrid modes rather than
falling back through the API-key XML-RPC adapter. Redaction tracks either
credential kind and both query parameter names.

## Consequences

Token-backed servers work for REST surfaces and cannot silently send their
token as `Bugzilla_api_key`. API-key detection, proof, cache state, and
XML-RPC behavior remain unchanged. Users needing XML-RPC token support need a
separate, explicitly designed protocol change.

## Considered & rejected

- **Reuse the API-key credential kind.** verified: `src/xmlrpc/protocol/client.rs`
  writes the literal `Bugzilla_api_key` XML-RPC parameter, which is not the
  token contract.
- **Allow hybrid fallback for tokens.** verified: hybrid can invoke that
  XML-RPC adapter after a REST transport failure; judgment: rejecting the
  unsupported mode is safer than changing the credential's wire meaning.
