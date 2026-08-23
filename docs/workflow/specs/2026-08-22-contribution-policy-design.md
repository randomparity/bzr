# Contribution Policy Design

## Scope

Issue [#544](https://github.com/randomparity/bzr/issues/544) requests a discoverable
contribution policy grounded in the repository's existing development workflow. This
change adds `CONTRIBUTING.md` and the smallest useful link from `README.md`. It does not
change code, CI, Make targets, security policy, governance, or contributor automation.

The document shape is governed by
[ADR 0021](../../adr/0021-contributor-guidance-lives-in-contributing.md).

## Audience and outcome

The audience is a developer preparing a first issue or pull request. After reading the
policy, they can set up the repository, choose the relevant verification commands, prepare
a reviewable branch and commit history, and select the correct reporting channel.

## Policy structure

`CONTRIBUTING.md` will contain:

1. A welcome and pointers for ordinary issues and private vulnerability reports.
2. Prerequisites and `make setup` as the canonical development setup entry point.
3. A branch and change workflow that keeps work off `main`, scopes each pull request, and
   uses conventional, imperative commits.
4. A verification table grounded in current repository commands:
   `make lint`, `make test`, and `make functional-test-all`, with the repository's existing
   minimum fallback of `make functional-test` when all supported versions cannot run.
5. Pull-request expectations: describe the current diff, identify verification performed,
   disclose unavailable functional testing, keep CI green, and avoid squash merging code.
6. Links to `docs/bzr-cli.md`, `docs/adr/`, and `SECURITY.md` as authoritative specialized
   references.

The README will link to the policy in a short `Contributing` section near its testing and
reference material. It will not duplicate the policy.

## Accuracy and failure handling

Every command named by the policy must exist in `Makefile` or Cargo's standard interface.
Every relative link must resolve in the checkout. When container-backed functional tests
cannot run, contributors must say so in the pull-request body and name the tier omitted;
the policy must not describe a skipped tier as passing.

The policy links to `SECURITY.md` for vulnerabilities and explicitly distinguishes that
private path from ordinary public bug reports. It does not restate contact details, keeping
the security policy authoritative.

## Verification

- A focused shell check asserts the required files, headings, command names, and relative
  link targets exist. It first runs against the pre-change tree and must fail because
  `CONTRIBUTING.md` is absent; it then passes after implementation.
- `make lint` validates formatting and Rust lint guardrails.
- `make test` validates the ordinary automated suite.
- Because this is a documentation-only change, run at least `make functional-test` before
  opening the pull request, or record a verified container-runtime blocker in the PR.

## Rollback

Revert the documentation commit. No runtime, schema, dependency, or persisted state is
affected.
