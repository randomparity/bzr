# Section-only bugzillarc import design

## Authority

Issue #800, frozen by `WORK:SCOPE` token `q800-9c4fa187`, permits safe
section-only and multi-server bugzillarc import. It excludes arbitrary
substring credential routing and username/password/certificate import.

## Problem

`resolved_servers` only accepts `DEFAULT.url`, then chooses one matching
section. A conventional file containing explicit URL sections but no default
therefore imports nothing, and a multi-server file loses all but one server.

## Design

Keep `DEFAULT.url` as the legacy single target: it may receive only an exact
raw-authority section override. Without `DEFAULT.url`, each non-DEFAULT section
whose section name parses as an absolute HTTP(S) URL becomes one imported
server. Its own values are used directly; DEFAULT values do not propagate
credentials into section-only servers. Non-URL section names remain
unresolvable, rather than becoming URL-substring credential destinations.
Invalid explicit URL sections fail with an actionable input error. BTreeMap
ordering makes imports deterministic.

No ownership transition is needed: the existing import command owns parsing,
resolution, persistence, and human/JSON results. Direct tests prove legacy,
section-only, multi-server, invalid, and credential-policy behavior; the real
functional phase proves a local import through the compiled CLI. Documentation
defines the accepted URL-section form and bounded limitation.

## Failure model

- **Actors and deployments:** a local operator or CI invokes the local-only
  command against a chosen bugzillarc file; imported servers are not contacted.
- **Invariants and assets at stake:** API keys only reach their explicitly
  named URL section; password, certificate, and arbitrary substring data are
  never persisted as credentials.
- **Accepted failure classes:** non-URL sections without `DEFAULT.url` stay
  unresolvable because there is no safe target; malformed explicit URLs fail
  before config persistence.
- **Covered elsewhere:** token/password lifecycle is credential policy; client
  certificate import is #677; substring matching is deliberately excluded.

## Success

- A no-DEFAULT file imports the bounded set of explicit HTTP(S) URL sections,
  including more than one server, with their API keys only.
- The existing DEFAULT-url path keeps exact-authority override behavior.
- Unsupported credentials remain reported and unpersisted; unresolvable or
  malformed section-only inputs receive actionable errors.
- CLI reference and live functional coverage document and exercise this set.

## Validation

- `make test-one T=import_bugzillarc` exercises resolver and persistence
  contracts, including a no-DEFAULT multi-server fixture.
- `make lint` and `make test` cover Rust, shell, docs, and regression gates.
- `make functional-test` runs the compiled CLI against a real Bugzilla
  container; the config phase is local-only but proves the shipped binary.
