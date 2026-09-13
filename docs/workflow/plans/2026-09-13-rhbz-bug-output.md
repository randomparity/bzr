# RHBZ bug-output implementation plan

1. Centralize the bounded recognition predicate for `cf_*`, `target_release`,
   and `sub_components`; use it in REST response decoding/output serialization,
   XML-RPC decoding, and field projection.
2. Add sibling Rust tests that preserve empty and multi-value server shapes and
   reject an unrelated non-`cf_` name.
3. Extend the RHBZ comparison phase so `bzr bug view` reads its write results
   in JSON and NDJSON: assert a fresh bug's empty `target_release` array and
   a server-controlled multi-value `sub_components` object before the existing
   populated write-to-read checks.
4. Document the named RHBZ extension fields and stock absence behavior.
5. Run focused tests, lint, the quiet suite, and RHBZ functional coverage.
