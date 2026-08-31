# ADR 0027: Make URL import aware of the active inline server

## Status

Accepted

## Context

Bugzilla Custom Search URL parsing resolves hostnames only against persisted servers. During
`bug search --from-url`, an explicit `--server-url` is carried separately in `CommandContext` and
wins connection selection later. The parser can therefore warn about default-server fallback or
reject a default-less config even when the imported hostname matches the actual inline destination.

## Decision

Pass the active inline server URL into URL parsing as optional invocation context. Keep configured
server lookup authoritative for the saved query's named `server` field, but treat the inline
server as an available request destination for warning and error decisions. Matching normalized
hostnames are silent. A mismatched inline hostname warns accurately that the inline server will be
used. Without an inline server, configured matching and default fallback are unchanged.

Hostname equality deliberately excludes scheme and port because issue #593 defines the contract
in hostname terms and the explicit inline URL, not the imported URL, remains the connection target.

## Consequences

Stateless inline searches no longer require matching persisted configuration. Saved queries still
record only configured server names, and imported URLs still undergo the existing credential
stripping and sanitization. The parser gains one optional input describing invocation routing.

Functional coverage must isolate configuration as well as use a real server; otherwise an earlier
configured-server fixture can mask the inline path.

## Considered & rejected

- **Keep the current behavior.** judgment: it preserves the false default-fallback warning and
  premature no-default failure for matching inline hosts, contradicting issue #593.
- **Suppress the warning in the search caller after parsing.** verified: at commit
  `0faa3df0d27a0365961485f16b3cb2538809e8b0`, `parse_bugzilla_url` emits the warning or returns the
  no-default error before `search::resolve_client_and_params` can apply `CommandContext`, so the
  caller cannot correct either result after the fact; reproduced with
  `git grep -n 'parse_bugzilla_url\|resolve_client_and_params' 0faa3df0 --
  src/commands/bug/search.rs src/commands/runtime/input/url_parser.rs`.
- **Add the inline server temporarily to `Config`.** judgment: mutating the persisted-domain model
  with an ephemeral synthetic entry risks leaking `(inline)` into saved query semantics and makes
  precedence less explicit.
- **Route the request to the imported URL hostname.** verified: at commit
  `0faa3df0d27a0365961485f16b3cb2538809e8b0`, connection target resolution gives the explicit inline
  server priority; reproduced with
  `git show 0faa3df0:src/commands/runtime/shared/connection/target.rs`. Changing routing would
  contradict the issue's observed and expected destination.
- **Compare full origins including scheme and port.** judgment: stricter origin equality does not
  serve the hostname contract and would reintroduce warnings for harmless explicit port or scheme
  differences while the inline URL remains authoritative for transport security.
