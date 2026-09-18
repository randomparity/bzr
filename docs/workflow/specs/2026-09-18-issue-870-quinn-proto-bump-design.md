# quinn-proto 0.11.14 → 0.11.18 (clear GHSA-4w2j-m93h-cj5j)

## Problem

`quinn-proto` 0.11.14 carries two HIGH Dependabot alerts, both
GHSA-4w2j-m93h-cj5j (*Remote memory exhaustion from unbounded out-of-order
stream reassembly*), one per manifest: `Cargo.lock` and `fuzz/Cargo.lock`. It is
lockfile-resident but activated by nothing (`cargo tree -i quinn-proto --target
all` prints nothing), so it compiles into no target graph; the lockfile-resident
version is what Dependabot scans.

## Scope

Bump `quinn-proto` 0.11.14 to 0.11.18 in both lockfiles and keep every guard
green.

- **Workspace** (`Cargo.lock`): `cargo update -p quinn-proto`. Adds 7 transitive
  packages: `chacha20`, `cpufeatures` 0.3.1, `getrandom` 0.4.3, `r-efi` 6.0.0,
  `rand` 0.10.2, `rand_core` 0.10.1, `rand_pcg` 0.10.2.
- **Fuzz** (`fuzz/Cargo.lock`): a surgical bump of the `quinn-proto` subtree only.
  A full `cargo update` cascades ~31 unrelated packages including `rustix` 1.1.5,
  which `fuzz.yml`'s pinned `nightly-2026-05-02` is fixed for; that full sync is
  excluded and tracked by #875. The subtree was verified byte-identical to cargo's
  own resolution for all 11 affected entries.

Exclusions (operator-approved): no fuzz full-sync, no dismiss-with-reasoning, no source/`deny.toml`/docs change.

### Failure model

- **A new package fails a license or ban.** `deny.toml` pins exact versions for
  its license exceptions and bans; `cargo deny check` gates it and is observed
  green, so no `deny.toml` edit is needed.
- **The fuzz subtree is left inconsistent.** Hand-editing a lock risks a
  disambiguation cargo would have made; every affected entry was byte-compared
  against cargo's own `cargo update` output.
- **The pinned fuzz CI build breaks.** The cascade moves `rustix`, the crate
  `fuzz.yml` pins its nightly for; the surgical bump does not move `rustix`, and
  the full sync that would is out of scope (#875).
- **The advisory is not actually cleared.** Confirm both locks show `quinn-proto`
    0.11.18 and `cargo deny check` reports advisories ok.

## Success

Both lockfiles carry `quinn-proto` 0.11.18; `cargo deny check` is green across
advisories, bans, licenses, and sources; the full guardrail suite (fmt, clippy,
structural checks, the unit suite, the functional suite) is green; the 7 added
packages are acknowledged in the commit and PR.

## Validation

- **Workspace lock resolves and builds.** `focused-test`: `cargo build --locked`
  succeeds and `make test-fast` passes 3215/3215; red is a resolve or compile
  failure on the bumped graph.
- **Advisory and policy green.** `focused-test`: `cargo deny check` prints
  `advisories ok, bans ok, licenses ok, sources ok`; red is any non-ok surface.
- **Fuzz subtree consistency.** `task-test-not-applicable`: no executable consumer
  validates `fuzz/Cargo.lock`'s internal consistency, so the contract is the
  byte-match of all 11 affected entries against cargo's output.
- **Behavior unchanged end-to-end.** `focused-test`: `make functional-test` (bz50)
   passes 578/578; red is a regression in the real-server tier.
