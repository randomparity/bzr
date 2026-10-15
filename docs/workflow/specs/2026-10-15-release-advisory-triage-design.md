# Release advisory reachability triage

Issue: #869. Scope: q869-654937ad. Base: main.

## Problem and ownership

RELEASING.md identifies four reachability hazards but deliberately supplies no
resolution procedure. Keep it as the release operator entry point and replace
the omission with a link to docs/release-advisory-triage.md. The new page owns
one manual procedure. ADR0022 keeps advisory assessment with the operator.
No new ADR is needed; optional reservation 0082 remains unused.

## Scope and success

- Separate lockfile membership from activation, dependency kinds, package and
  feature selection, and each of the seven release targets.
- Analyze bzr default features with vendored-keyring on the two bottle Linux
  targets, defaults on the other five, and xtask without defaults on the
  Linux manpage runner separately. Never use workspace-wide feature unification
  or --target all to decide that a released binary contains a dependency.
- Include default-feature crates.io build verification on the Linux publish
  runner. Exclude arbitrary consumer builds from the seven-target conclusion.
- Route fuzz/Cargo.lock through its independent manifest and bzr-fuzz root;
  distinguish its developer execution from release artifacts.
- Every graph probe uses --locked. A producer must succeed before a consumer
  can decide; failed probes remain unknown. Test membership by exit status,
  with a known-present root control and exact crate/version comparison.
- Compare normal,build with normal to distinguish build-only paths; retain
  proc macros as build-host exposure, and compare normal,build,dev only to
  identify development exposure. Trace positive paths before saying runtime.
- Exercise the current quinn-proto version in both lockfiles, every released
  profile, xtask, publish and fuzz. Confirm unchanged lockfile bytes. Absence
  is bounded graph evidence, not an advisory dismissal or universal safety.
- Bound github-actions conclusions by affected pinned action versions, job
  triggers, permissions, token/secret access, reusable/composite actions,
  artifact/cache flow, and publication/signing access. CI-only is not all-clear.
- Record advisory identity/range, commit, manifest, package/features, target
  and build host, Cargo version/config, probe statuses, paths, scope limits,
  conclusion and owner in the release-preparation PR. No alert writes here.

## Probe design

Use forward cargo tree output to avoid treating an inverse-tree warning or
unknown package error as absence. Flat output uses --prefix none --format '{p}'.
An awk predicate compares first field with crate name and second with vVERSION;
exit 0 means present, 1 absent, other statuses unknown. Root validation accepts
any version of the selected root and must return 0. A controlled synthetic
positive/negative input validates the predicate before graph decisions.

Use one Bash subshell with set -eu, mktemp -d and an EXIT trap; never pipe Cargo
into a quiet search. Save each successful graph, classify it independently,
and retain the statuses in operator notes before cleanup. No new committed
script, gate, runtime change or dependency. Shell recipes target Bash 3.2+,
awk and Cargo 1.89+; use installed tools only during verification.

The exact release targets are x86_64-unknown-linux-gnu,
aarch64-unknown-linux-gnu, powerpc64le-unknown-linux-gnu,
s390x-unknown-linux-gnu, aarch64-apple-darwin,
x86_64-pc-windows-msvc and aarch64-pc-windows-msvc.

## Limits and decisions

Cargo tree approximates compilation; package selection and host build graphs
matter. A normal-edge match may be a proc macro, not shipped runtime code.
Local cross-target probes are preliminary when build hosts or toolchain/config
 differ from the release jobs. Resolve discrepancies using matching release
build evidence before declaring an affected release safe. Graph activation
does not prove vulnerable code executes. Registry tools, native libraries,
actions, containers and externally fetched payloads require their own review.
For absent graph nodes use the bounded phrase “not activated in the inspected
profiles”; for present nodes investigate the advisory conditions; unresolved
inputs prevent a yes/no all-clear and stay owned by the release operator.

## Failure model

- Actors and deployments: release operator using a clean release checkout;
  the seven binary jobs, Linux xtask and publish jobs, and separate fuzz usage.
- Invariants and assets: candidate lockfiles unchanged; evidence tied to commit;
  no false runtime or CI all-clear from an omitted graph or failed command.
- Accepted failure classes: external inventory changes after review remain the
  ADR0022 pre-tag refresh race; arbitrary downstream configurations are outside
  the stated profile set, so receive no conclusion.
- Covered elsewhere: alert remediation/dismissal and dependency updates belong
  to release operator; automation/settings to repository owner; changing release
  profiles to release maintainer. No new controls for those surfaces.

## Threat model

- Boundaries: advisory and dependency metadata inform a trusted operator;
  third-party action code executes with job credentials and artifact access.
  No new or widened execution/permission boundary is added by these docs.
- Actors: vulnerable or compromised dependency/action maintainers, untrusted PR
  inputs, and a fallible operator; trust the checked-out release inputs and tools.
- Controls: exact version/manifest identity; separate successful producer from
  status predicate; explicit graph/host bounds; positive control; unchanged locks;
  inspect Actions privilege and release data flow before a bounded conclusion.
- Out of scope: fixing vulnerabilities, changing permissions, or proving arbitrary
  package code safe; named owners above retain those decisions.

## Validation

Execute the published shell block, including positive and negative predicate
controls, on the candidate; inspect each profile result and exact lock versions.
Exercise an invalid Cargo target to prove producer failure cannot become absence.
Review Actions guidance against merged release/publish workflows. Check Markdown
links and git diff --check. Mandatory hooks provide clippy/layout and final unit
verification; no additional full suite for this documentation-only change.
