# RHBZ core signal implementation plan (#798)

## Scope and baseline

- Branch: `feat/rhbz-core-signal-798`
- Base: `origin/main` at `8cf32f8f16c82efa518655eff039c5ab46ed91f1`
- Guardrails: `make lint`, `make test`, `make functional-compare-rhbz`
- Design denominator: 250 changed lines (M), from the campaign assessment.

## Steps

1. Add an RHBZ core comparison phase to the isolated runner. Reuse the existing
   resource server setup and assert create, REST/XML-RPC reads, update/readback,
   and named-account identity.
2. Print immutable server and binary provenance before phases; extend the RHBZ
   fixture so this runner contract has a fast deterministic check.
3. Add a dedicated scheduled/manual workflow job that builds the release binary,
   runs the RHBZ target, and stops the RHBZ container under `always()`.
4. Document the local target, its isolated lifecycle, and provenance output.
5. Run focused tests, lint, the regular suite, and the live RHBZ comparison.

## Contract verification

| Contract | Focused test | Task test |
| --- | --- | --- |
| RHBZ core phase is isolated and sourced | `pybz/container-tests.sh` fixture | `make functional-compare-rhbz` |
| CI job is independent and cleans up | workflow fixture inspection | GitHub Actions CI |
| Core CLI journey persists server state | phase assertions | `make functional-compare-rhbz` |
| README procedure is accurate | doc review | command named in README |
