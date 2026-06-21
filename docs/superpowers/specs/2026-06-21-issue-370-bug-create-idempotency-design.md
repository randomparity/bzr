# Issue #370: Bug Create Idempotency Decision

## Context

Issue #370 asks whether `bzr bug create` can safely support an idempotency key,
or whether the limitation should be documented for agents.

`bzr` already avoids automatic replay of ambiguous writes. `--retry` can retry
429 responses and connect failures for any method, because those failures happen
before Bugzilla processes the write. It does not replay creates, updates, or
comments after 5xx responses or read timeouts where Bugzilla may already have
applied the write.

The remaining risk is manual or agent-driven retry after an ambiguous
`bug create` transport failure.

## API Research

Official Bugzilla 5.2+ REST documentation describes bug creation as
`POST /rest/bug` and returns only the newly filed bug's ID:

- https://bugzilla.readthedocs.io/en/latest/api/core/v1/bug.html#create-bug

The documented create parameters include bug fields such as product, component,
summary, description, alias, URL, whiteboard, target milestone, groups, and
flags. They do not include an idempotency key, idempotency header, client token,
or replay token.

The same API page warns that the WebService interface may accept fields beyond
those listed, but undocumented behavior may change in the future. That makes an
undocumented field unsuitable as a duplicate-prevention contract.

The general Bugzilla REST documentation covers JSON request/response handling,
query-string override behavior, authentication arguments, and common return-field
parameters, but does not define a cross-cutting idempotency mechanism:

- https://bz.apache.org/bugzilla/docs/en/html/api/core/v1/general.html

Searching the official docs for "idempot" found no documented support.

## Decision

Do not add `bzr bug create --idempotency-key` today.

`bzr` cannot safely implement a generic create idempotency key against the
Bugzilla REST APIs it currently targets because there is no documented
server-backed key that guarantees "create once, return the original bug on
replay" semantics.

Client-side deduplication by summary, URL, alias, or whiteboard would not be a
safe replacement:

- summaries are not unique and searches are race-prone;
- URL and whiteboard are optional, mutable fields;
- aliases are unique when supported and supplied, but a second create would
  produce a conflict rather than the original create response, and not every
  workflow can rely on aliases being available or accepted;
- custom fields differ by installation and cannot provide a portable CLI
  contract.

## User-Facing Behavior

Keep the current retry behavior unchanged:

- safe reads can retry 5xx/read-timeout failures;
- writes are not automatically replayed after ambiguous 5xx/read-timeout
  failures;
- 429/connect failures remain retryable for any method.

Document that agents should not blindly rerun `bug create` after an ambiguous
failure. The safe workflow is to search for likely matches using the intended
summary/product/component and any intentionally supplied distinctive marker, then
inspect candidates before retrying. URL and whiteboard matches are search aids,
not uniqueness proof.

`--dry-run` can help audit the payload before the first write, but it is not a
reservation and does not make a later retry idempotent.

## Files

- `docs/bzr-cli.md`: document idempotency and safe retry behavior in the
  `bug create` section.
- `CHANGELOG.md`: record the documentation decision.

## Testing

- `rg -n "idempotency|ambiguous create|blindly rerun" docs/bzr-cli.md CHANGELOG.md`
- `git diff --check`

## Out of Scope

- Adding a client-only deduplication cache.
- Adding an unsupported or Bugzilla-installation-specific idempotency parameter.
- Changing global retry behavior.
- Opening implementation subissues, because the safe API contract does not exist
  in the supported Bugzilla REST documentation.
