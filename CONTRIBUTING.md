# Contributing to bzr

Thank you for helping improve `bzr`. Keep each change focused, follow the repository's existing
patterns, and verify the behavior you changed before opening a pull request.

## Reporting issues

Search the [issue tracker](https://github.com/randomparity/bzr/issues) before opening a new issue.
For ordinary bugs, include the `bzr` version, operating system, Bugzilla version, command or input,
expected result, and actual result. Remove API keys, session data, and other secrets from logs.

Do not report vulnerabilities publicly. Follow the private process in the
[security policy](SECURITY.md).

## Development setup

Development requires Git, Rust 1.89 or newer, and the native packages needed by the default
features. Functional tests additionally require Podman or Docker; see the
[functional-test guide](tests/functional/README.md) for supported runtimes.

Clone your fork, enter the checkout, and run:

```bash
make setup
```

This checks the Rust toolchain, installs the required Rust components and coverage tools, installs
the repository hooks, and builds `bzr`. If setup reports a missing system library, install the
development package named by the error and run it again.

## Making changes

- Create a branch from the current `main`; do not develop directly on `main`.
- Keep one independently reviewable concern in each pull request.
- Follow the existing module boundaries and tests described in `CLAUDE.md`.
- Add tests for changed behavior, including error and edge paths. User-facing CLI changes also
  require coverage in `tests/functional/phases/` against a real Bugzilla container.
- Use Conventional Commits in imperative mood with a subject no longer than 72 characters.
- Do not amend or rebase commits after pushing them. Keep logical commits intact for `git bisect`.

## Verification

Before committing, run the repository guardrails relevant to your change:

```bash
make lint
make test
```

Functional tests are required before opening a pull request. The complete run covers every
supported Bugzilla version:

```bash
make functional-test-all
```

When running every supported version is not possible, run at least the default version:

```bash
make functional-test
```

If Podman and Docker are genuinely unavailable, state that in the pull-request body and name the
functional tier you could not run. Do not describe an omitted check as passing.

Documentation-only changes should also confirm that every added relative link resolves and every
documented command still exists in the repository.

## Pull requests

- Describe what the current diff does and why.
- Link the issue the pull request resolves, using `Closes #<number>` when appropriate.
- List the exact verification commands run and their results.
- Call out functional-test limitations, platform limitations, and known follow-up work.
- Keep CI green and address review findings with separate commits.
- Preserve commit history when merging code pull requests; use rebase or merge rather than squash.

## Project references

- [CLI reference](docs/bzr-cli.md) documents the command surface and output contracts.
- [Architecture decision records](docs/adr/) explain accepted design constraints.
- [Security policy](SECURITY.md) owns vulnerability reporting and supported-version policy.
- [Functional-test guide](tests/functional/README.md) explains the live Bugzilla harness.
- `CLAUDE.md` records repository architecture, conventions, and guardrail details.
