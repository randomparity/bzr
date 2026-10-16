# Repair the standalone fuzz lockfile (#901)

## Problem and authority

The fuzz workspace is intentionally excluded from the root workspace and has its own
lockfile. Root commit `5485cebd` advanced exact requirements without advancing that
lockfile. Cargo 1.89.0 now rejects locked fuzz metadata before building a target.
This design implements the revised frozen scope `q901-f37d4468` on issue #901, including
the operator's 2026-10-16 "Approve narrow extension" decision after the license checkpoint.

## Scope and architecture

Preserve the standalone workspace and its existing ownership: root `Cargo.toml` owns the
exact requirements, and `fuzz/Cargo.lock` owns their fuzz resolution. Refresh only entries
required to satisfy the current manifest; retain compatible existing transitive versions.
There is no caller migration, ownership move, new dependency requirement or runtime change.
Use Cargo's targeted resolution rather than a blanket dependency update or hand-written
checksums. Inspect the resulting package/version/source/checksum/dependency delta.

Global constraints: Rust MSRV 1.89.0; fuzz nightly-2026-05-02; root deny.toml unchanged;
implementation changes only fuzz/Cargo.lock, fuzz/Cargo.toml package license = MIT, and
fuzz/deny.exceptions.toml exact libfuzzer-sys@0.4.13 NCSA exception. Fuzz verification may
use --allow unmatched-skip only for the justified unused root keyring/dbus skip diagnostic.
Root Cargo.toml, Cargo.lock, toolchain pins, workflows and fuzz targets remain unchanged.

The smaller alternative, leaving the lock unchanged, fails the observed locked-resolution
contract. Broad regeneration would needlessly refresh compatible transitive packages.
Automated drift prevention, root dependency refresh, toolchain changes and further policy
exceptions are excluded and owned by the operator. No architecture or migration is introduced.

The first fuzz policy run failed: the unpublished fuzz package has no license declaration;
libfuzzer-sys 0.4.13 requires NCSA alongside MIT or Apache-2.0. Declare the fuzz package MIT
to match the repository and add a manifest-local deny.exceptions.toml with only that exact
crate-version NCSA exception. This reuses cargo-deny's existing exception discovery and keeps
the root allowlist unchanged. A shared root exception instead warns as unused on root checks.
The root windows-sys 0.59 skip is unused because fuzz disables bzr's keyring default feature;
use the approved diagnostic override when checking fuzz, with that explicit justification.
Duplicate bans, advisory, source and license checks remain enforced.

## Success

1. C1: Cargo metadata resolves the fuzz graph with --locked on the committed lockfile.
2. C2: The existing fuzz_xmlrpc_parse, fuzz_url_parser, fuzz_flag_parser and fuzz_tls_der
   targets build and each run for 30 seconds using nightly-2026-05-02.
3. C3: Report the complete package delta and declared licenses; evaluate advisories, bans,
   sources and licenses against root dependency policy plus the exact approved fuzz-local
   exception. Root checks have no diagnostic override; fuzz checks use only the approved one.
4. C4: Preserve excluded inputs, verify root MSRV locked metadata, repository guardrails,
   full tests and the default real Bugzilla functional suite before opening the PR.

## Failure model

- Actors and deployments: local maintainers on macOS arm64; existing Linux fuzz CI;
  Cargo fetching crates from the existing crates.io registry.
- Invariants and assets: reproducible resolution, compiler compatibility, registry
  integrity, unchanged root graph/policy, and the exact fuzz-only exception boundary.
- Accepted failure classes: fuzz smoke runs give bounded sampling rather than exhaustive
  parser correctness; later upstream releases cannot guarantee future registry availability.
- Covered elsewhere: root dependency refresh, toolchain floors, policy exceptions and
  automated drift prevention are operator-owned exclusions of scope q901-f37d4468. Further
  exceptions or compiler floor changes stop this repair for an operator decision.

## Threat model

- Boundary inventory: Cargo consumes registry metadata and crate archives; refreshed
  transitive package versions widen existing dependency execution during fuzz builds.
- Actor model: registry package publishers control dependency source; maintainers trust
  the existing crates.io source and approved root pins. No new network authority is added.
- Controls: Cargo preserves registry checksums and --locked prevents implicit resolution;
  root cargo-deny policy plus the exact fuzz-local NCSA exception checks advisories, sources,
  duplicates and licenses. The exception is version-bound so a changed libfuzzer-sys version
  requires review. The approved unmatched-skip override only suppresses the unused-entry
  diagnostic and changes no graph or ban rule. Review
  changed versions and declared licenses before accepting the lockfile.
- Out of scope: upstream malicious releases that pass existing controls; redesigning
  dependency approval beyond the approved exception. Further policy failures are checkpointed.

## Validation

Use the existing locked metadata failure as the red observation. Re-run it after the
bounded refresh, then build/run the four pinned-nightly fuzz targets and confirm the lock
hash is unchanged. Root metadata at MSRV, dependency policy, make lint, full make test
and the release binary's default functional suite supply integration evidence. Record
first-run failures and exact deltas in private verification state and the PR.
