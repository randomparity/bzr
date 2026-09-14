# Dedicated RHBZ core signal (#798)

## Problem

The scheduled functional workflow exercises stock Bugzilla versions only. The
RHBZ catalogue therefore has no independent recurring signal for its core CLI
path, despite needing a pinned, real RHBZ image.

## Scope

Extend the existing RHBZ-only comparison runner, rather than any stock matrix,
with a core phase. It will create a bug, verify REST and XML-RPC view/search,
update the summary, read the persisted result, and authenticate a named admin
for `whoami`. The runner will print the checked-out RHBZ source revision and
the binary source revision and checksum. A distinct scheduled/manual workflow
job builds the release binary once, invokes the RHBZ target, and always stops
its container. README guidance names the command and provenance output.

### Failure model

- A broken RHBZ API or protocol shape fails its core phase instead of becoming a
  stock-version result.
- A failed create/update/readback or named identity command fails the job.
- Unavailable provenance or cleanup fails the RHBZ target; cleanup still runs.
- Anonymous/auth audit, stock arrays, and multi-version behavior stay owned by
  #663, ADR 0073/#665, and #796 respectively.

## Success

The RHBZ arm is independently visible in scheduled and manual runs, records
the server and binary revisions, runs the catalogue plus the bounded core
journey, and leaves no RHBZ container behind. Stock workflow jobs and their
version arrays do not change.

## Validation

- focused-test: shell fixture proves the runner sources the core phase and
  reports both revision classes.
- focused-test: workflow checks prove the dedicated job builds and invokes the
  RHBZ target with unconditional cleanup.
- focused-test: `make lint` and `make test` cover syntax and fixtures.
- live-test: `make functional-compare-rhbz` proves the real RHBZ path.

## Authorized fixture compatibility

The pinned RHBZ source uses PostgreSQL-only `SET STATEMENT_TIMEOUT`, while the
disposable RHBZ fixture uses MariaDB. The operator authorized a narrow
Containerfile substitution to MariaDB `max_statement_time`, including reset,
solely to make the bounded live search proof executable. The fixture retains a
60-second RHBZ query limit. No stock image, version array, or application code
is changed.

Fresh rootless CI image builds also need a Containerfile-local `tar
--no-same-owner` wrapper before `cpanm`: CPAN archive owner IDs are invalid for
the runner and otherwise make the image build fail before the RHBZ phase. The
operator separately authorized that wrapper; it applies only while preparing
this disposable RHBZ image.

Fedora 39's Cpanel::JSON::XS is 4.37, below the RHBZ dependency's 4.38
minimum. The operator authorized a replacement limited to the fixture: install
the pinned 4.42 archive from the CPAN MetaCPAN mirror only after its CPAN
published SHA-256 is verified. This stops the later unpinned CPAN resolution
from fetching that module while retaining ordinary dependency installation.
