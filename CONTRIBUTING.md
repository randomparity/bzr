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

Development requires Git, GNU Make, Rust 1.89 or newer, and the native packages needed by the
default features. On Linux, the default keyring backend requires `pkg-config` and the libdbus
development headers (`libdbus-1-dev` on Debian and Ubuntu). Functional tests additionally require
Podman or Docker; see the [functional-test guide](tests/functional/README.md) for supported
runtimes.

Clone your fork, enter the checkout, and run:

```bash
make setup
```

This checks the Rust toolchain, installs the required Rust components and coverage tools, installs
the repository hooks, and builds `bzr`. It does not install operating-system packages. If Cargo or
`pkg-config` reports a missing library, install its development headers with your platform's
package manager and run setup again.

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

### Controlled-fault verification

A test that passes both before and after a fix has proved nothing about the fix. When a change
corrects a defect, demonstrate the test goes red against the pre-fix code and green after, and
record both observations in the pull-request body.

1. Write or strengthen the test first.
2. Remove the fix from the working tree — `git stash push` the source paths, or invert the one
   line under test. Do not weaken the test.
3. Run the narrowest command that covers it:
   - a unit test: `make test-one T=<name-substring>`;
   - a production-shape proxy rewrite: `python3 tests/functional/redhat-shape-proxy.py --self-test`;
   - a single functional arm: `make functional-test-bz50`, `make functional-test-bz52`,
     `make functional-test-bz53`, or `make functional-test` for the unpinned default.
4. Observe the failure. Record the exact command and the failing assertion.
5. Restore the fix, confirm the tree really is restored (`git stash list`, `git status`), re-run
   the same command, and observe green.
6. Put both observations in the pull-request body.

**A functional arm needs a fresh binary and a fresh container.** Two things can make the run
report on something other than your fault:

- **A stale binary.** `tests/functional/phases/00-build.sh:16` uses `$BZR_BIN` verbatim when it is
  set and executable, so an exported `BZR_BIN` runs the whole arm against a binary that never
  received your fault. A *failed* build is not the hazard: `run-tests.sh` runs under
  `set -euo pipefail`, so a non-zero `cargo build` aborts the run rather than falling through to
  a stale artifact.
- **A stale container.** `tests/functional/setup-bugzilla.sh` reuses an already-running container
  for this checkout and version, so users, groups, and bugs from earlier runs persist. Residue
  can satisfy the assertion under test in the faulted state, or fail it in the restored state.

So run the functional arm as one gated chain, before and after removing the fault:

```bash
unset BZR_BIN
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset \
  && cargo build --release \
  && make functional-test-bz50
```

Keep the reset, the build, and the arm chained with `&&` rather than pasting them as three
separate lines, so a failed reset or build stops before the arm runs instead of testing the
previous state. (`unset` cannot fail, so it stands on its own line.)

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
