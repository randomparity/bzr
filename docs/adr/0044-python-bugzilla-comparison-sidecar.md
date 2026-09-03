# ADR 0044: Run python-bugzilla as a network-namespace sidecar

## Status

Accepted

## Context

The functional suite already starts one real Bugzilla container per checkout and version, but a
host process cannot assume python-bugzilla is installed and should not mutate the developer's
Python environment. Issue #666 requires python-bugzilla 3.3.0 comparisons against that exact
server instance for Bugzilla 5.0, 5.2, and 5.3.

The Bugzilla container exposes HTTP on its own port 80 and publishes a runtime-assigned host port.
The comparison client needs stable access to the container without adding another host-port or DNS
contract. It also needs a writable home for python-bugzilla cache/config state and access to the
functional runner's isolated exchange directory.

## Decision

Build a small, pinned python-bugzilla 3.3.0 image and start one long-lived client sidecar for each
active Bugzilla container. Join the Bugzilla container's network namespace with
`--network container:<bugzilla-container>` so python-bugzilla reaches the server as
`http://127.0.0.1`. Bind-mount the runner's private `FUNC_CONFIG_DIR` at `/work`, set the sidecar's
home to a named volume, and keep it alive with a literal command until the comparison runner's EXIT
trap removes it. The image tag includes `bugzilla_checkout_id`, so simultaneous builds from
different worktrees cannot retag the image another comparison run is about to start.

`run_pybz` executes the `bugzilla` CLI inside that sidecar, capturing stdout, stderr, and exit status
in the same `BZR_STDOUT`, `BZR_STDOUT_RAW`, `BZR_STDERR`, and `BZR_EXIT` globals as `run_bzr`.
Comparison phases snapshot the first client's capture before invoking the second, then normalize
both snapshots before comparing capability-level facts. They do not compare presentation bytes.

Comparison-run IDs add the phase-tree namespace to ADR 0029's existing identity:
`compare/<phase>/<slug>`. The ordinary functional runner keeps its established `<phase>/<slug>`
identity. Both static and runtime checks include that namespace, so the same phase and slug in the
two trees remain distinct evidence references.

`expect_gap <issue>` is an explicit result transition for the current test. It records an expected
gap only when the preceding comparison failed. If the comparison passed, it records a failure that
names the stale issue marker, forcing the parity report and test to be updated when the gap closes.

## Consequences

- Developers and CI need only the existing Docker/Podman runtime; no host Python installation is
  required.
- The top-level python-bugzilla package is fixed at 3.3.0. The image fixture detects an incompatible
  base-image or transitive-dependency rebuild; the dependency closure is not immutable.
- Namespace sharing ties sidecar lifetime to the server container. Startup validates the server and
  creates a fresh sidecar; cleanup removes only the checkout/version-scoped sidecar and leaves its
  cache volume for reuse.
- Comparison tests assert semantic parity and can distinguish pass, fail, skip, and known gaps.
- Comparison images, containers, and cache volumes are checkout-scoped; Bugzilla version also
  scopes containers and volumes where their lifecycle differs.
- The sidecar deliberately uses the test server's unauthenticated product-list endpoint for the
  initial smoke comparison; future authenticated comparisons must add credentials deliberately.

## Considered & rejected

- **Install python-bugzilla on the host.** judgment: this mutates developer and CI Python
  environments and makes interpreter/package resolution another supported surface.
- **Run a fresh python-bugzilla container per command.** judgment: repeated startup obscures test
  output, discards useful client state, and costs more than one isolated long-lived sidecar.
- **Reach Bugzilla through its published host port.** verified: ADR 0030 records that the host port
  is runtime-assigned and differs across checkouts; joining the existing container namespace avoids
  exporting host-specific routing into the client container.
- **Treat a known gap as an ordinary skip.** judgment: a skip stays green after parity is restored,
  while the requested gap contract must fail closed when its marker becomes stale.
- **Compare raw CLI output.** judgment: the clients intentionally use different presentation
  formats; normalized capability facts test parity without coupling either CLI's rendering.
