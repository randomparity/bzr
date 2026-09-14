# RHBZ component update plan

1. Add the RHBZ-only CLI and command validation.
2. Add XML-RPC `Component.update` request construction and client dispatch.
3. Supersede ADR 0037 for the capability-gated RHBZ path and document it.
4. Replace the RHBZ parser-gap test with an independent persisted-state proof.
5. Run focused tests, lint, the normal suite, and the RHBZ functional comparison.
