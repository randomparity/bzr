# ADR 0033: Share lenient deserialization adapters by wire contract

## Status

Accepted

## Context

Four independent deserializers currently coerce Bugzilla response shapes: signed API error codes,
positive product IDs, optional integer-or-boolean attachment flags, and positive integer-or-object
relationship IDs. Epic #616 requires future ID and boolean tolerance to reuse a shared adapter set,
but the four current parsers do not all accept the same domain. ADR 0024 also deliberately keeps
adjacency response mappings strict, and ADR 0028 already owns signed sort-key adaptation.

## Decision

Place crate-internal serde field adapters in `src/types/deserialization.rs`, beside the existing
shared `sort_key` adapter. Share only exact wire contracts:

- non-negative `u64` from a JSON integer or decimal string; and
- `Option<bool>` from null, a JSON boolean, or exact integer `0`/`1`.

The unsigned decoder accepts caller-supplied expectation and invalid-value messages; its ordinary
serde field adapter supplies generic unsigned wording. This keeps decoding logic shared without
changing established consumer diagnostics.

Product access IDs use the configured shared unsigned decoder with their existing messages and
retain their private post-decode nonzero check. Attachment flags use the shared optional-bool
adapter unchanged. Signed API error codes and integer-or-object relationship IDs remain
specialized, with comments recording why their domains cannot use these helpers. ADR 0024 strict
mappings and ADR 0028 sort keys remain unchanged.

## Consequences

Future consumers with either exact wire contract can reuse one tested implementation. Domain
validation that is narrower than the wire primitive, such as a positive product ID, remains with
the consumer and is visible at the call site. Configured diagnostic text preserves existing
command-visible parse failures while the implementation moves. Consolidation does not widen
existing inputs, add a dependency, or expose a new public API. Two specialized parsers remain
because merging them would change accepted shapes rather than remove duplication.

## Considered & rejected

- **Use one generic visitor framework for signed, unsigned, boolean, and object unions.** judgment:
  the domains share syntax but not semantics, and a generic policy layer would be larger and harder
  to audit than two field adapters plus two specialized parsers.
- **Make the shared unsigned adapter reject zero.** verified: `src/client/resources/product.rs` at
  commit `b83620741d3e28300e9804d338bfb473cd7e86fa` owns product-specific positivity, while issue #620
  requests a reusable unsigned string-or-number adapter for later ID consumers. Keeping nonzero at
  the consumer preserves both contracts.
- **Teach relationship IDs to accept strings and reuse the unsigned adapter.** verified:
  `src/types/bug/links_tests.rs` at commit `b83620741d3e28300e9804d338bfb473cd7e86fa` explicitly
  rejects `{"bug_id":"11"}` and the parser also owns an integer-or-object union; accepting strings
  would violate the behavior-preserving charter.
- **Move signed API error codes onto the unsigned adapter.** verified: `default_error_code()` in
  `src/client/response.rs` at commit `b83620741d3e28300e9804d338bfb473cd7e86fa` returns `-1`, and
  its visitor accepts signed `i64`; an unsigned adapter cannot preserve that domain.
- **Leave all implementations local.** judgment: this fails #620 and would make dependent epic
  entries add further copies of already repeated coercion logic.
