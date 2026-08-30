# ADR 0026: Scope-qualify dependency collector preflight

## Status

Accepted

## Context

The dependency collector currently proves that each configured Bugzilla server is reachable by
running a termless, one-row `bug list` before resource reads. Several production installations
reject termless searches with API code 1000 even though they allow the explicitly requested bug
reads. The proof is also security-relevant: only after it succeeds may a later code 102 response be
classified as an inaccessible bug instead of a command-fatal authentication ambiguity.

## Decision

Keep one preflight per declared server, before resource collection, but derive its search term from
that server's canonical input scopes. Bug-ID and alias scopes become filtered `bug list` probes;
saved-query, custom-search, product, milestone, and version scopes use the same bounded command
shape as their normal enumeration. The search operation is independent from the later single-bug
detail operation: an invisible bug is omitted from search results, while a detail denial can return
code 102. A code 102 response to the search itself remains command-fatal and never proves its own
classification. A restriction is eligible only when no ordinary scope names that server.

Selection is total and input-order independent. All bug IDs across a server's scopes are
aggregated and the lowest is preferred. Otherwise candidates are ordered by scope-kind rank and
canonical normalized JSON. An alias is already unique per server by policy validation. This rule
also governs semantically equivalent policies with repeated same-kind scopes.

Every probe remains read-only, requests at most one ID, and must return a valid structured success
envelope before the server is marked reachable. A structured probe failure, including API code
1000, remains command-fatal with its API classification. Code 102 remains resource-scoped only
after the corresponding scope-qualified probe succeeds.

## Consequences

Production servers that prohibit termless searches can be collected when the declared scope is
valid. Rejected credentials still fail before resource reads. Preflight may execute a bounded
scope command that collection later repeats; retaining the existing authorization boundary is
preferred to making the first resource failure carry both reachability and resource semantics.

## Considered & rejected

- **Treat every code 102 as inaccessible.** verified: `collect.py` uses successful preflight as
  the independent reachability condition that separates a resource denial from a command-wide
  authentication failure; removing it would weaken that boundary.
- **Mark a server reachable after its first successful resource read.** judgment: this avoids a
  duplicate request but cannot classify code 102 when the first requested resource is inaccessible.
- **Retry termless code 1000 with a filtered probe.** judgment: knowingly sending an incompatible
  request adds noise and makes an expected production policy look like a recoverable failure.
