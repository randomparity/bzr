# ADR 0050: Run comparison proxies in the sidecar namespace

## Status

Accepted

## Context

The python-bugzilla comparison client shares the active Bugzilla container's network namespace
under ADR 0044. Existing TLS and Red Hat-shape proxies instead listen on host loopback and forward
to Bugzilla's runtime-assigned host port. The comparison client therefore cannot reach those
fixtures without either changing its network topology or starting equivalent proxy processes in
the namespace it already shares with Bugzilla.

Issue #669 must exercise python-bugzilla's `--nosslverify` and `.redhat.com` Bearer paths on Docker
and Podman hosts while preserving the long-lived sidecar home used for cached authentication state.

## Decision

Stage the existing proxy programs and generated TLS material through the comparison runner's
private `/work` mount. Start namespace-local proxy processes through `container exec` in the
existing python-bugzilla sidecar. They listen only on namespace loopback and forward directly to
Bugzilla on `127.0.0.1:80`. The runner proves readiness from inside that sidecar and container
removal remains the final process-lifecycle boundary.

Keep the existing host proxy instances for bzr-side observations. The two clients may traverse
separate instances of the same fixture, but both instances forward to the same Bugzilla container
and emit credential-kind evidence without credential values.

Add the `.redhat.com` loopback alias inside the sidecar rather than changing host DNS. The alias and
namespace proxy exist only for the checkout/version-scoped sidecar lifetime.

## Consequences

- Comparison coverage retains ADR 0044's single long-lived client and cached home state.
- No host-network mode, fixed host route, or newly published container port becomes a prerequisite.
- Proxy scripts, certificates, logs, and PID records cross only the existing private `/work` mount.
- Host and namespace proxy lifecycle need separate helpers and readiness checks.
- Abrupt phase failure may leave a namespace proxy running until the runner removes the sidecar;
  checkout/version-scoped container cleanup still terminates it.

## Considered & rejected

- **Start a second sidecar with host networking.** judgment: this adds another lifecycle and splits
  client state while making comparison behavior depend on Docker/Podman host-network semantics.
- **Publish namespace proxy ports to the host.** verified: ADR 0044 and
  `tests/functional/lib.sh` start the sidecar with `--network container:<bugzilla-container>`;
  published ports belong to the network-owning Bugzilla container and cannot be added afterward.
- **Expose the host proxies beyond loopback.** judgment: this widens fixture reachability and still
  requires runtime-specific host discovery from the sidecar.
- **Replace the existing host fixtures with namespace-only proxies.** judgment: bzr executes on the
  host, so this would exchange one reachability problem for its mirror image.
