# bzr Functional Tests

End-to-end tests that exercise `bzr` CLI commands against real Bugzilla containers. The default target is Bugzilla 5.0, and the repo also includes Bugzilla 5.2 and 5.3 coverage.

## Prerequisites

- `podman` or `docker` (podman preferred; docker used as fallback)
- `jq`
- `curl`
- `cargo` + Rust toolchain
- `python3` and `openssl` — only for the ad-hoc TLS phase; when either is
  missing that phase skips cleanly (see the TLS Fixture section below)

## Quick Start

```bash
# Build the Bugzilla container image (one-time, ~5 min)
make functional-build

# Start the default Bugzilla 5.0 container and run the standard test suite
make functional-test

# Run the same suite across all supported Bugzilla versions
make functional-test-all

# Stop the container when done
make functional-stop
```

## Manual Steps

```bash
# Build the image
tests/functional/setup-bugzilla.sh build

# Start the container (waits for Bugzilla to be ready)
tests/functional/setup-bugzilla.sh start

# Run the tests
tests/functional/run-tests.sh

# Check container status
tests/functional/setup-bugzilla.sh status

# View container logs
tests/functional/setup-bugzilla.sh logs

# Stop and remove the container
tests/functional/setup-bugzilla.sh stop
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BZR_FUNC_PORT` | `(runtime-assigned)` | Host port mapped to container port 80 (overrides the runtime-assigned default when set) |
| `BZR_FUNC_CONTAINER` | `bzr-func-test-<version>-<checkout-id>` | Container name |
| `BZR_FUNC_IMAGE` | `localhost/bzr-func-<version>:latest` | Image name |
| `BZR_FUNC_TIMEOUT` | `90` (`240` for bz52/bz53) | Health check timeout in seconds |
| `BZR_BIN` | `target/release/bzr` | Path to pre-built bzr binary (skips cargo build) |
| `BZR_FUNC_TLS_PORT` | `(resolved backend port) + 1000` | Host port the TLS proxy listens on for the ad-hoc TLS phase |
| `BZR_FUNC_REDHAT_PORT` | `(resolved backend port) + 2000` | Host port for the Red Hat response-shape profile |

## Test Structure

Tests run in dependency order across the phase files sourced by
`tests/functional/run-tests.sh`:

1. Build and isolated config setup
2. Server/auth detection and fixture capability setup
2c. Ad-hoc TLS trust flags over an HTTPS fixture (see below)
3. Products and components
4. Fields, classifications, users, and groups
5. Bug create/read/update/search/paging/relationships/collision/clone workflows
6. Batch updates and convenience verbs
7. My-bug filters, templates, and saved queries
8. Comments and attachments, including private-resource hybrid/XML-RPC paths
9. Global options, argument validation, completion, schema, and sequence tests

The suite creates real Bugzilla data in the running container and reads it back
through the CLI. Count and paging assertions use per-run unique whiteboard
markers so repeated runs against an already-started container stay stable.

## TLS Fixture (ad-hoc `--server-tls-*` flags)

Phase `02c-tls-inline` exercises the stateless TLS trust controls
(`--server-tls-insecure`, `--server-tls-ca-cert`, `--server-tls-pin-sha256`,
`--server-tls-pin-now`) against a real Bugzilla server over HTTPS.

The Bugzilla containers serve HTTP only, so the phase starts a small
TLS-terminating reverse proxy (`tests/functional/tls-proxy.py`, python3 stdlib)
that fronts the running container:

```
bzr --(HTTPS, 127.0.0.1:$BZR_FUNC_TLS_PORT)--> tls-proxy.py --(HTTP, container)--> Bugzilla
```

`openssl` generates a throwaway CA and a leaf certificate (with an
`IP:127.0.0.1` SAN so `--server-tls-ca-cert` passes hostname verification), and
the leaf's `sha256//<base64>` pin is computed for the pin cases. The proxy and
certs are created in a temp dir and torn down at the end of the phase.

The phase is **default-safe**: it runs automatically when `python3`, `openssl`,
and `curl` are present and skips cleanly otherwise, so `run-all-versions.sh`
stays predictable on hosts without TLS tooling. It runs against the single
container for the active `BZR_BZ_VERSION`.

Note: a wrong `--server-tls-pin-sha256` is rejected with exit 5 (a transport
error), not exit 13; the test asserts rejection without hard-coding the code.
Clap-level mutual-exclusion and require-`--server-url` validation lives in
`phases/17b-arg-validation.sh` (tests 125b/125c).

## Config Isolation

Tests set `XDG_CONFIG_HOME` to a temp directory, so they never touch `~/.config/bzr/config.toml`.

## Troubleshooting

**Container build fails with Perl dependency errors:**
The Containerfile installs Perl modules via dnf + cpanm. Network issues or missing repos can cause failures. Retry, or check container logs with `tests/functional/setup-bugzilla.sh logs`.

**Health check times out:**
Bugzilla's `checksetup.pl` can take 30-60s on first run. Increase timeout with `BZR_FUNC_TIMEOUT=120`.

**Port conflict:**
No longer occurs by default — each invocation gets a runtime-assigned
host port. Set `BZR_FUNC_PORT` to pin an exact one if needed (e.g. to
attach a debugger to a known address).

**`BZR_FUNC_PORT` does not match the running container's actual port:**
If a container from a prior run is still up, a follow-up `start` invocation
with `BZR_FUNC_PORT` set to a different port now errors instead of silently
proceeding, since the running container's port cannot change (`status` only
reports `REST API: unknown` and exits 0 in this case). Run
`tests/functional/setup-bugzilla.sh stop` first, then start again with the
desired `BZR_FUNC_PORT`.

**Orphaned containers left behind by a deleted worktree or clone:**
Container names are checkout-scoped, so a container started from a worktree
or clone that no longer exists is not discoverable by any make target or
`setup-bugzilla.sh` invocation. Find them with `podman ps -a --filter
name=bzr-func-test-` (or `docker`) and remove with `podman rm -f <name>`
(or `docker`).

**Tests fail after image rebuild:**
The container starts fresh each time. If tests fail, check `tests/functional/setup-bugzilla.sh logs` for Bugzilla errors.
