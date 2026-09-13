# RHBZ bug-output implementation plan

1. Centralize the bounded recognition predicate for `cf_*`, `target_release`,
   and `sub_components`; use it in REST decoding/encoding, XML-RPC decoding,
   and field projection.
2. Add sibling Rust tests that preserve empty and multi-value server shapes and
   reject an unrelated non-`cf_` name.
3. Extend the RHBZ comparison phase so `bzr bug view` reads its write results
   in JSON and NDJSON, including empty and populated values.
4. Document the named RHBZ extension fields and stock absence behavior.
5. Run focused tests, lint, the quiet suite, and RHBZ functional coverage.
