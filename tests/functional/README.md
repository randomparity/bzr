# bzr Functional Tests

End-to-end tests that exercise every `bzr` CLI command against a real Bugzilla 5.0 instance running in a podman container.

## Prerequisites

- `podman` (rootless configured)
- `jq`
- `curl`
- `cargo` + Rust toolchain

## Quick Start

```bash
# Build the Bugzilla container image (one-time, ~5 min)
make functional-build

# Start the container and run all tests
make functional-test

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

Tests run in dependency order across 12 phases:

1. **Config** — set-server, show, set-default (no network)
2. **Server & Auth** — server info, whoami
3. **Products** — create, list, view, update
4. **Components** — create, update
5. **Fields & Classifications** — field list, classification view
6. **Users** — create, search, update
7. **Groups** — create, view, update, add/remove/list users
8. **Bugs** — create, view, list, search, update, history, negative test
9. **Comments** — add, list, tag, search-tags
10. **Attachments** — upload, list, download, update
11. **Global Options** — --output table, --quiet, --server
12. **Cleanup** — summary and exit code

Total: ~62 tests.

## Config Isolation

Tests set `XDG_CONFIG_HOME` to a temp directory, so they never touch `~/.config/bzr/config.toml`.

## Troubleshooting

**Container build fails with Perl dependency errors:**
The Containerfile installs Perl modules via dnf + cpanm. Network issues or missing repos can cause failures. Retry, or check `podman logs bzr-func-test`.

**Health check times out:**
Bugzilla's `checksetup.pl` can take 30-60s on first run. Increase timeout with `BZR_FUNC_TIMEOUT=120`.

**Port conflict:**
Change the port with `BZR_FUNC_PORT=9089`.

**Tests fail after image rebuild:**
The container starts fresh each time. If tests fail, check `tests/functional/setup-bugzilla.sh logs` for Bugzilla errors.
