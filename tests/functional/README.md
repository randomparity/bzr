# bzr Functional Tests

End-to-end tests that exercise `bzr` CLI commands against real Bugzilla containers. The default target is Bugzilla 5.0, and the repo also includes Bugzilla 5.2 and 5.3 coverage.

## Prerequisites

- `podman` or `docker` (podman preferred; docker used as fallback)
- `jq`
- `curl`
- `cargo` + Rust toolchain

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
| `BZR_FUNC_PORT` | `8089` | Host port mapped to container port 80 |
| `BZR_FUNC_CONTAINER` | `bzr-func-test` | Container name |
| `BZR_FUNC_IMAGE` | `localhost/bzr-func-test-bz:latest` | Image name |
| `BZR_FUNC_TIMEOUT` | `90` | Health check timeout in seconds |
| `BZR_BIN` | `target/release/bzr` | Path to pre-built bzr binary (skips cargo build) |

## Test Structure

Tests run in dependency order across the phase files sourced by
`tests/functional/run-tests.sh`:

1. Build and isolated config setup
2. Server/auth detection and fixture capability setup
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

TLS functional coverage for ad-hoc `--server-tls-*` flags is intentionally not
part of this suite expansion. It needs an HTTPS fixture in front of Bugzilla and
is tracked separately in GitHub issue #406.

## Config Isolation

Tests set `XDG_CONFIG_HOME` to a temp directory, so they never touch `~/.config/bzr/config.toml`.

## Troubleshooting

**Container build fails with Perl dependency errors:**
The Containerfile installs Perl modules via dnf + cpanm. Network issues or missing repos can cause failures. Retry, or check container logs with `tests/functional/setup-bugzilla.sh logs`.

**Health check times out:**
Bugzilla's `checksetup.pl` can take 30-60s on first run. Increase timeout with `BZR_FUNC_TIMEOUT=120`.

**Port conflict:**
Change the port with `BZR_FUNC_PORT=9089`.

**Tests fail after image rebuild:**
The container starts fresh each time. If tests fail, check `tests/functional/setup-bugzilla.sh logs` for Bugzilla errors.
