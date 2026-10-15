# Checkout credential persistence

## Problem and authority
Issue #867 requests explicit checkout credential policy and a sibling workflow audit.
The approved WORK:SCOPE token is q867-b8484c20. Pinned checkout v7.0.1 stores
credentials under RUNNER_TEMP (since v6); later Git commands can still use them.
Source: [pinned README](https://github.com/actions/checkout/blob/3d3c42e5aac5ba805825da76410c181273ba90b1/README.md).

## Design and success
Keep policy in existing checkout inputs. Set persist-credentials: false on each
of ci.yml's 14 checkout steps, preserving its existing Sonar full-history input.
Audit the nine workflow files. Existing false inputs stay intact. Add false to
publish-crates and release checkouts without an authenticated Git consumer.
Keep explicit true with a consumer comment only for release homebrew's tap
checkout (Commit and push) and post-release-bump's checkout (Commit and push branch).
The homebrew template checkout is read-only. Release uploads use GH_TOKEN;
cargo publish uses CARGO_REGISTRY_TOKEN; neither needs checkout Git credentials.
Release merge-bottles configures authentication itself and has no checkout.
Acceptance: those inputs and two documented exceptions, then green PR CI.
No ownership transition or caller migration is necessary.

## Global constraints
Do not change token permissions, secret availability, release publication,
registry authentication, action versions, dependencies, or Rust 1.89.0.
Use existing platform tools; add no persistent checker or abstraction.

## Failure model
1. Actors/deployments: PR contributors and maintainers on existing GitHub-hosted
   Linux, macOS and Windows jobs; trusted tag-triggered release jobs.
2. Invariants/assets: read-only checkout steps retain no reusable Git credential;
   the two existing authenticated Git push paths keep working; other inputs stay intact.
3. Accepted classes: tag-only jobs cannot be proven end-to-end without publishing
   a release; inspect their consumer paths and validate workflow syntax locally.
4. Covered elsewhere: token privileges and secret provisioning belong to the
   repository owner; release and registry authentication belong to release maintainer.

## Threat model
- Boundary inventory: checkout action receives a job token; subsequent build/test
  code can be PR-controlled. No new or widened boundary.
- Actors: untrusted PR code; trusted maintainers and pinned checkout action.
- Controls: persist-credentials: false removes checkout's post-fetch Git credential
  configuration. True is retained only at the two audited trusted push consumers.
- Out of scope: process-level access to other job secrets, token permission changes,
  or redesign of trusted release authentication; approval excludes these surfaces.

## Validation and rollback
A temporary Ruby/YAML structural test inventories every checkout and requires
false except the two exact release consumers; deletion/true violations must fail.
No prose snapshots. Run actionlint on changed workflows and repository guardrails.
Run full default real-Bugzilla functional suite before PR. GitHub CI proves PR
checkout consumers on runner platforms. Revert this isolated workflow change if
a previously unidentified authenticated consumer fails; investigate before retry.

Decision: [ADR0080](../../adr/0080-checkout-credential-persistence.md).
