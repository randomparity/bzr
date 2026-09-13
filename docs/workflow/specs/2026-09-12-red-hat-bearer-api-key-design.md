# Red Hat REST Bearer API-key transport design

## Goal

Make bzr send documented Bearer API-key credentials to the exact Red Hat
Bugzilla REST host without widening credential routing or changing XML-RPC.

## Scope and constraints

The frozen charter is issue #678, token `q678-843c7fda`. Only
`bugzilla.redhat.com` is a Bearer destination; an explicit parsed-host equality
test treats a port as transport routing rather than host identity, rejects
suffix lookalikes and URL-text substrings as selectors, and accepts normalized
host casing. Non-Red-Hat REST retains its detected or
pinned standard method. XML-RPC retains body credentials. No dependencies,
commands, config schema, token/password login, client certificates, or
bugzillarc work are introduced. Rust remains 1.89.0 and the current-thread
Tokio invariant remains unchanged.

## Architecture

`bugzilla_auth` becomes the single policy owner: it parses the REST base URL,
recognizes the exact host, validates a HeaderValue containing `Bearer ` plus
the API key, and applies it to a RequestBuilder. Client construction and
pre-client probing use that helper, so detection, strict credential proof, and
ordinary requests make the same choice. The existing `AuthMethod` remains the
standard-server fallback selection rather than becoming an unguarded Bearer
setting. XML-RPC never calls the helper.

## Behavior and errors

Invalid API-key header characters fail with the existing actionable config
error before a request is emitted. An invalid base URL is not a Bearer host and
uses the existing standard-method path; URL validation remains owned by the
connection layer. On the exact host, Bearer overrides only REST application,
including auth probes. The active-key redactor still removes the raw key from
headers, URLs, errors, and previews.

## Threat model

The local operator supplies a server URL and API key; a configured server and
network peer receive requests. The added boundary is URL host to credential
scheme selection. The control is parsed exact-host comparison before adding the
Authorization header; a non-match receives existing standard auth instead.
The server controls response text; existing safe URL and active-key redaction
are the control against secret disclosure in diagnostics. TLS trust, client
certificates, user-selected non-Red-Hat destinations, and XML-RPC payload
authentication are explicitly out of scope.

## Acceptance evidence

- Unit tests prove exact host selection, false cases, Bearer syntax, and
  redaction with a Bearer key.
- Client/auth-probe tests prove Red Hat REST sends Bearer and other REST hosts
  preserve header/query behavior.
- The Red-Hat-shaped proxy records exactly one Bearer request; its comparison
  phase changes from `expect_gap 678` to a passing parity result.
- The CLI reference and parity matrix describe the automatic, exact-host REST
  behavior. ADR 0077 records the decision.
