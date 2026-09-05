# ADR 0051: Share one adapter with bounded local proofs

## Status

Accepted

## Context

ADR 0046 established one private-file JSON adapter for python-bugzilla library comparisons and
allowed null transport only for a single named component-update proof. Issue #669 needs library-only
login/logout operations and a client-certificate configuration proof while mutual-TLS behavior
remains owned by #677. Treating the configuration proof as network evidence would be false, while a
second adapter would duplicate the established secret, path, and serialization boundary.

## Decision

Retain one fixed `python-bugzilla-adapter.py` dispatch table and the existing
`/work/compare` private-file boundary. Network operations construct the pinned client against the
disposable server and must report an observed REST or XML-RPC transport. Local-proof operations are
an explicit fixed registry; they construct only the minimum pinned-library object or recorder needed
to observe a public client surface without issuing a network request, and their output transport is
null.

The registry initially contains the component-update request-shape proof and the client-certificate
session-configuration proof. Each local result states what it observed and may not claim server
acceptance or persisted outcome. Adding another local proof requires a comparison issue that cannot
exercise the surface against the repository's live server fixture, plus focused adapter fixtures
that prove no network call occurs.

Login, cached-auth, and logout are network operations. They read credentials from private request
files, construct a new client per operation, and serialize no password, token, API key, certificate
path, or upstream exception text.

## Consequences

- One adapter continues to own path confinement, credential loading, transport normalization, safe
  errors, and JSON serialization.
- Null transport means a bounded local client-surface observation, not successful server behavior.
- The client-certificate row can document a surface gap without adding a mutual-TLS fixture to
  issue #669.
- The local-proof registry can grow only with explicit comparison ownership and a no-network test.

## Considered & rejected

- **Keep exactly one local-proof operation.** verified: issue #669 requires a client-certificate
  gap marker while issue #677 owns the mutual-TLS fixture, leaving no live server proof in this
  change.
- **Add a second adapter for authentication and TLS.** judgment: this duplicates the private-file,
  error-redaction, and serialization boundary ADR 0046 created to centralize.
- **Treat client construction as transport evidence.** judgment: setting a session certificate
  path does not prove a server requested or accepted it.
- **Add mutual TLS here.** verified: issue #669 and its frozen scope assign that fixture and product
  behavior to #677.
