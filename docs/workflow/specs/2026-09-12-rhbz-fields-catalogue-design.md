# RHBZ custom-field and sub-component catalogue (#775)

## Authority

The frozen `WORK:SCOPE` on #775 (token `q775-c0417559`) requires real-RHBZ
evidence for sub-components, target release, fixed-in, and three whiteboards.
It excludes bzr feature implementation. [ADR 0075](../../adr/0075-catalogue-rhbz-custom-fields-with-real-server-controls.md)
records the chosen boundary.

## Design

`run-rhbz-compare.sh` will source `09-rhbz-fields.sh` after phase 07 and before
phase 08. This preserves the seeded component's active state for its bzr probes;
phase 08's deliberate component deactivation remains the final mutation.
The phase owns four tests and run-token-scoped bugs in the seeded product and
component. Its SQL fixture inserts one `releases` row for the seeded product
and one `rh_sub_components` row owned by the seeded component, then reads their
IDs and names back before use. The entrypoint grants `admin@test.bzr` the
`devel`, `redhat`, and `qa` groups; phase 09 reads all three memberships plus
its existing `editbugs` control before any whiteboard mutation. It verifies
that RHBZ advertises each required field; unavailable metadata, fixture, or
privilege is a positive-control failure, never a gap.

The adapter will expose narrow validated RHBZ operations using python-bugzilla
3.3.0's `sub_component`, `target_release`, `fixed_in`, `devel_whiteboard`,
`internal_whiteboard`, and `qa_whiteboard` arguments. Each operation is proven
by a REST readback of the named live field. The bzr arm uses the existing
`--field` entry point only when it accepts the same field; otherwise the exact
parser or API diagnostic is captured. A test's parity classification follows
the observed bzr result, not a preselected outcome.

| Stable ID | Python-bugzilla control | Server readback |
| --- | --- | --- |
| `sub-components` | update with run-token `sub_component` | `sub_components` |
| `target-release` | create with run-token `target_release` | `target_release` |
| `fixed-in` | update with run-token `fixed_in` | `cf_fixed_in` |
| `whiteboards` | update all three whiteboard arguments | all three `cf_*_whiteboard` fields |

## Failure, isolation, and security

The runner's disposable RHBZ container owns cleanup. Names are generated per
run and request captures remain in its private exchange directory. This change
adds no CLI, credential, dependency, or production trust boundary; the adapter
accepts only test-generated bounded JSON and must reject malformed requests.
No proxy or local recorder can substitute for a successful live-server control.

## Validation

Focused shell/adapter fixtures must cover runner order, all IDs, rejected
malformed requests, missing controls, and each report row; run them with
`bash tests/functional/pybz/container-tests.sh`. `make lint` and `make test`
cover repository guardrails. Run `make release` before
`make functional-compare-rhbz`; the latter requires `target/release/bzr` but
does not build it. It is the decisive proof: it must execute all four live
controls and clean up the RHBZ container.
