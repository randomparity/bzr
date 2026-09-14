# RHBZ component update design

Issue #802 restores component update solely for the real-server RHBZ XML-RPC
contract. It supersedes ADR 0037 only for this capability-gated path.

## Contract

`bzr component update --product P --component C` requires at least one of
`--description`, `--default-assignee`, or `--is-active`. After connection it
rejects login tokens, requires the advertised `RedHat` extension, then invokes
`Component.update` with `names:[{product:P,component:C}]` and an `updates`
object.

Stock REST PUT remains removed. ExternalBugs is unchanged.

## Verification

Unit tests pin CLI validation and XML-RPC request fields. The RHBZ comparison
first proves the python-bugzilla positive control, then makes a bzr update and
reads component state independently through the server REST product view.

## Failure model

Malformed local input fails before connection; unsupported servers fail at the
capability gate before XML-RPC mutation; server faults remain XML-RPC errors;
the functional phase fails when the persisted component state differs.
