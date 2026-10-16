# Full-functional binary prerequisite (#900)

## Problem and scope
Phase 00 accepts an executable BZR_BIN, whereas phase 18d requires the checkout's
release path after canonicalizing its directory. An incompatible debug override
therefore fails only after hundreds of cases. The operator-approved #900 charter
owns early validation, agreeing documentation, focused fixtures and a normal live
functional run. Phase 00 remains the selection owner; phase 18d remains unchanged.

## Contract and design
After existing binary selection and executability validation, canonicalize the
selected directory with `cd` and `pwd -P`, append its basename, and compare with
`$REPO_ROOT/target/release/bzr`, exactly as phase 18d does. Reject a mismatch with
exit 1 before the Bugzilla curl check, naming the selected input, required path,
and the remedy: unset BZR_BIN for a release build or select that release path.
Keep existing default build and non-executable-override fallback behavior.
Supported executable overrides include the explicit release path, relative release
spelling from the invoking directory, and a directory symlink to release with
basename bzr. A differently located file symlink is not resolved by the existing
oracle and is rejected. This check promises path agreement, not artifact freshness.

## Success
- C1: an executable debug or alternate path fails before cargo/network fixtures,
  with selected input and actionable release-path remedy.
- C2: default selection/build, explicit release, relative release and directory
  symlink forms reach the network check with the expected selected binary.
- C3: boundary fixtures run without real network/build work; the normal live
  functional suite passes with phase 18d unchanged.
- C4: README documents the release-path constraint and supported spellings.
These criteria trace to WORK:SCOPE q900-7970e36a on issue #900.

## Constraints and alternatives
Support macOS system Bash 3.2 and current Bash without adding dependencies.
Retain Rust 1.89.0 and current-thread runtime policy; no product Rust changes.
A shared path helper would add indirection for two short comparisons; validating
only executability retains the defect. Weakening phase 18d is explicitly excluded.
No architecture decision or migration is introduced.

## Failure model
- Actors and deployments: local operators and CI invoking the full functional
  suite from a checkout on macOS/Linux, with a selected executable or default build.
- Invariants and assets: fail before costly Bugzilla work for incompatible selected
  paths; preserve downstream installed-helper/release-path proof.
- Accepted failure classes: stale binaries and ignored build-pipeline failure are
  operator-owned exclusions; concurrent replacement of an artifact is outside the
  existing path-only contract. Absent/non-executable overrides retain default build.
- Covered elsewhere: phase 18d proves installed helper locations and release path;
  setup-bugzilla owns container provisioning; RHBZ runner prerequisites are excluded.

## Threat model
- Boundary inventory: existing operator-controlled BZR_BIN enters phase 00;
  no new or widened trust boundary. The change compares a canonical directory path.
- Actor model: local operator or CI controls the checkout and environment;
  trust the selected executable as before. Remote Bugzilla cannot supply this path.
- Control per boundary: quote the selected path in shell operations; retain the
  executable check and compare with the phase-18d release-path invariant. Local
  failure diagnostics identify the supplied path and release-path remedy.
- Explicitly out of scope: binary freshness/build status are operator exclusions;
  concurrent artifact replacement remains outside the existing path-only proof.

## Validation
A focused shell fixture sources phase 00 in isolated temporary repositories with
stub cargo/curl. It covers default build, explicit/relative/directory-symlink
release overrides, incompatible debug and file-symlink overrides, and missing
binary fallback. Assertions inspect exit status, selected value, diagnostics and
which boundary stubs ran. Run under /bin/bash and current Bash. Run shell lint,
repository guards, full make test before committing, and a normal live functional
run against a release build matching HEAD; mandatory push hook owns its full run.
