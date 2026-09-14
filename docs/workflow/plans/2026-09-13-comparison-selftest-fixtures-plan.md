## Plan

1. Align the parity-report fixture's auth rows with the committed report and
   retain exact-row checks for drift detection.
2. Update the auth phase fixture's baseline and fault expectations for the
   landed login/token behavior.
3. Initialize the RHBZ smoke fixture exchange directory, then update the RHBZ
   field fixture to provide all phase controls, match `/rest/field/bug`, and
   prove only the supported sub-component error is a gap.
4. Run the bare self-test, then the repository lint and test guardrails.

## Contracts

| Contract | Validation |
| --- | --- |
| Self-test does not drift from parity report | Exact report-row fixture check |
| RHBZ fixture models current metadata/control boundary | Self-test and controlled failures |
| Unsupported RHBZ failures remain failures | RHBZ controlled-error assertion |
