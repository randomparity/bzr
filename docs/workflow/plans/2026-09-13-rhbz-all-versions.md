# RHBZ bug-version accessor implementation plan

## Goal

Repair #796's pinned RHBZ image so a core bug read requesting `version` works
over REST and XML-RPC, without inventing multi-version semantics.

## Tasks

1. In `tests/functional/versions/rhbz/Containerfile`, add the bounded
   `all_versions` compatibility accessor beside the existing component
   accessors. Its sole element is the existing scalar `version`.
2. In `tests/functional/compare/rhbz/07-rhbz-smoke.sh`, reuse the disposable
   smoke bug and add REST/XML-RPC `id,version` reads asserting
   `["unspecified"]`.
3. In `tests/functional/pybz/container-tests.sh`, extend the RHBZ smoke
   fixture to require and model those protocol reads.

## Verification

- The shell fixture fails if either protocol read is absent or produces a
  scalar/incorrect version shape.
- The dedicated live RHBZ runner proves the real pinned image returns the
  expected one-element array through both protocols.
- Repository lint and test guardrails remain green.
