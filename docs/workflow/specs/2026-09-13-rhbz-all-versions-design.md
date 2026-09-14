# RHBZ bug-version accessor design

## Authority

Issue #796, frozen by `WORK:SCOPE` token `q796-a5a27e54`, requires the
pinned RHBZ test image to serve authenticated REST and XML-RPC bug reads that
request `version`. The operator approved explicit empty exclusions, with the
constraint that this change must not claim unverified multi-value semantics.

## Problem

The pinned RHBZ WebService formatter replaces its normal scalar `version`
output with `all_versions` when that field is requested, but `Bugzilla::Bug`
does not define that accessor. The image already supplies the analogous
single-component compatibility accessors. Thus a core read reaches a missing
method before either client protocol can return a bug.

## Design

Install one compatibility accessor in the pinned image:
`all_versions` returns a one-element array containing the bug's existing
scalar `version`. This is deliberately the same bounded adaptation as the
existing `all_components` accessor: the disposable image's `bugs.version`
column and `Bugzilla::Bug::version` are scalar, and no source in the pinned
fork supplies a second version relation. It neither fabricates nor documents
multi-version support.

The RHBZ smoke phase creates one disposable bug with the existing known
`unspecified` version, then reads `id,version` through both REST and XML-RPC
using bzr. It asserts each result is the one-element JSON array
`["unspecified"]`. This makes an image readiness pass depend on the core
accessor rather than only extension advertisement. The shell fixture models
the two new commands and rejects their absence or a non-array version output.

## Failure model

- A missing accessor causes an API error before a REST or XML-RPC response;
  both protocol reads must fail the smoke phase.
- Returning a scalar changes the RHBZ formatter's response contract and fails
  the JSON assertion.
- Claiming multiple versions without a backed relation would hide an unknown
  server semantic; this design returns only the stored scalar and makes no
  multi-value assertion.
- Stock Bugzilla runners remain outside the RHBZ-only lifecycle of ADR 0073.

## Success and validation

- Both authenticated protocols return the single stored version for a real
  RHBZ bug.
- The self-test covers normal and missing-version-read command paths.
- `make lint`, `make test`, release build, and `make functional-compare-rhbz`
  succeed.
