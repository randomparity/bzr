# bzr - Bugzilla CLI

[![CI](https://github.com/randomparity/bzr/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/randomparity/bzr/actions/workflows/ci.yml)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=randomparity_bzr&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=randomparity_bzr)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/randomparity/bzr/badge)](https://scorecard.dev/viewer/?uri=github.com/randomparity/bzr)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![MSRV: 1.89](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://blog.rust-lang.org/2025/06/26/Rust-1.89.0/)
[![crates.io](https://img.shields.io/crates/v/bzr.svg)](https://crates.io/crates/bzr)

A command-line interface for Bugzilla servers, written in Rust. Inspired by the GitHub CLI (`gh`), `bzr` lets you search, view, create, and update bugs, manage comments and attachments, and switch between multiple Bugzilla instances — all from your terminal.

## Features

- **Bug management** — list, search, view, create, clone, update, and batch-update bugs; view change history
- **Bug workflow** — view your bugs (`bzr bug my`), save reusable field templates, and run saved queries
- **Comments** — list and add comments, with `$EDITOR` integration for composing
- **Comment tags** — add, remove, and search comment tags
- **Attachments** — list, download, upload, and update file attachments with auto-detected MIME types
- **Flags** — set, request, and clear flags on bugs and attachments
- **Products** — list, view, create, and update products
- **Components** — create and update product components
- **Classifications** — view classification details
- **Fields** — look up valid values for bug fields (status, priority, severity, etc.)
- **Users** — search, create, and update users
- **Groups** — list members, add/remove users, view, create, and update groups
- **Server diagnostics** — check server version and extensions (`whoami`, `server info`)
- **Admin operations** — create and update products, components, users, and groups
- **Multi-server** — configure and switch between multiple Bugzilla instances
- **Output formats** — human-readable tables (with colored status) or JSON for scripting
- **Secure auth** — API key sent via `X-BUGZILLA-API-KEY` header by default; falls back to query parameter auth for older servers

## Installation

### Choosing an install method

If you have a package manager that fits, use it. The shorter the path,
the more you get for free (manpages, uninstall, dependency tracking):

- **macOS or Linux with Homebrew** — use the [Homebrew tap](#homebrew-macos-linux).
- **Debian / Ubuntu** — install the [`.deb` package](#linux-packages-deb--rpm) attached to the latest release.
- **Fedora / RHEL / CentOS Stream / Rocky** — install the [`.rpm` package](#linux-packages-deb--rpm).
- **Windows, or any Linux distro without `apt`/`dnf`** — use the [one-line installer](#pre-built-binaries) or download a pre-built tarball or zip.
- **Have Rust installed and want to build it yourself** — `cargo install bzr --locked` ([from crates.io](#from-cratesio)) or `cargo install --path . --locked` ([from source](#from-source)).

The first three install manpages and license/doc files automatically.
The other paths need a [manual manpage install](#manual-pages) if you
want `man bzr` to work.

### Homebrew (macOS, Linux)

```bash
brew tap randomparity/tap
brew install bzr
```

Tap repository: <https://github.com/randomparity/homebrew-tap>.

Pre-built binaries are published for macOS arm64 (Apple Silicon) and
Linux x86_64 / aarch64. Intel Mac builds from source automatically
(brew pulls in a build-time `rust` dep for that path; no extra
configuration needed).

The tap is auto-bumped on each stable release. Pre-release tags
(`vX.Y.Z-rcN`) do not update the formula — use the [tarball](#pre-built-binaries)
or `cargo install` if you want to test a release candidate.

Uninstall with `brew uninstall bzr` and `brew untap randomparity/tap`.

### Linux packages (`.deb` / `.rpm`)

Each release attaches Linux packages alongside the tarballs:

- `.deb` for `amd64`, `arm64`, `ppc64el` (Debian arch names)
- `.rpm` for `x86_64`, `aarch64`, `ppc64le`, `s390x` (RPM arch names)

There is no apt or dnf repository today — download the package for your
architecture from [GitHub Releases](https://github.com/randomparity/bzr/releases/latest)
and install it locally.

Debian / Ubuntu:

```bash
sudo apt install ./bzr_X.Y.Z-1_amd64.deb
sudo apt remove bzr            # uninstall
```

Fedora / RHEL / CentOS Stream / Rocky:

```bash
sudo dnf install ./bzr-X.Y.Z-1.x86_64.rpm
sudo dnf remove bzr            # uninstall
```

Files installed:

- `/usr/bin/bzr`
- `/usr/share/man/man1/bzr.1`, `/usr/share/man/man1/bzr-*.1`
- `/usr/share/doc/bzr/README.md`, `/usr/share/doc/bzr/CHANGELOG.md`
- `/usr/share/doc/bzr/copyright` (`.deb`) or `/usr/share/licenses/bzr/LICENSE` (`.rpm`)

Both packages declare a runtime dependency on the system D-Bus library
(`libdbus-1-3` on Debian, `dbus-libs` on RPM) for the OS keychain
backend; `apt`/`dnf` resolves it automatically.

### Pre-built binaries

For a one-line install on Linux, macOS arm64, or Windows:

**Linux / macOS (Apple Silicon):**

```bash
curl -fsSL https://raw.githubusercontent.com/randomparity/bzr/main/install.sh | sh
```

**Windows (x86_64 / ARM64):**

```powershell
irm https://raw.githubusercontent.com/randomparity/bzr/main/install.ps1 | iex
```

The installer detects your platform, verifies the SHA-256 checksum
against the published `SHA256SUMS`, and drops the binary in
`~/.local/bin` (Unix) or `%LOCALAPPDATA%\Programs\bzr` (Windows).
It never modifies PATH or system files.

Env var overrides:

- `BZR_VERSION=vX.Y.Z` — pin to a specific release tag (default:
  latest stable).
- `BZR_INSTALL_DIR=/some/path` — change the install directory.

To pin both the script and the binary to a specific release (e.g.
for reproducible builds):

```bash
curl -fsSL https://github.com/randomparity/bzr/releases/download/vX.Y.Z/install.sh | sh
```

Manpages are not installed by the script; see [Manual pages](#manual-pages).

#### Manual download

Or download the tarball or zip directly from
[GitHub Releases](https://github.com/randomparity/bzr/releases/latest).

Available builds: Linux (x86_64, aarch64, ppc64le, s390x), macOS arm64
(Apple Silicon), Windows (x86_64, aarch64).

```bash
tar xzf bzr-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
cd bzr-vX.Y.Z-x86_64-unknown-linux-gnu
sudo install -Dm755 bzr /usr/local/bin/bzr
```

Each archive bundles the binary, `LICENSE`, `README.md`, and a
`man/man1/` directory of manpages — see [Manual pages](#manual-pages)
to install those.

Each release also publishes a `SHA256SUMS` file. Verify a download
before installing:

```bash
sha256sum --check --ignore-missing SHA256SUMS
```

### From crates.io

```bash
cargo install bzr --locked
```

The `--locked` flag tells cargo to use the exact dependency versions
published in `Cargo.lock`, which are tested against the MSRV. Without it,
cargo re-resolves to newer transitive dependencies that may exceed the MSRV
and fail to build.

`cargo install bzr` does **not** install manpages. See
[Manual pages](#manual-pages) for how to add them.

### From source

```bash
cargo install --path . --locked
```

Requires Rust 1.88+. Same manpage caveat as `cargo install bzr` — see
[Manual pages](#manual-pages).

### OS keychain support (`keyring` feature)

`bzr` can store per-server API keys in the OS keychain (macOS Keychain,
GNOME Keyring / KWallet via Secret Service on Linux, Windows Credential
Manager). This is provided by the `keyring` Cargo feature, which is
**enabled by default** — the install commands above give you keychain
support automatically. See [Credential storage](#credential-storage)
below for how to use it.

On headless Linux systems without a running Secret Service daemon
(servers, containers, CI runners), you can opt out of the feature to
avoid pulling in `libdbus-1` at build time:

```bash
cargo install bzr --locked --no-default-features
```

A build without the feature still supports plaintext and environment
variable credentials; only the `config set-keyring` /
`migrate-to-keyring` subcommands become unavailable. See
[`docs/troubleshooting.md`](docs/troubleshooting.md) for diagnosing
keychain errors.

### Manual pages

The `.deb`, `.rpm`, and Homebrew install methods install manpages
automatically. The other install paths do not — `cargo install bzr`,
`cargo install --path .`, and the pre-built tarballs leave manpages
on disk (the tarballs ship them under `man/man1/`) but do not put them
on your `MANPATH`.

To install them by hand from a release tarball:

```bash
sudo install -Dm644 man/man1/bzr.1 /usr/local/share/man/man1/bzr.1
sudo install -Dm644 man/man1/bzr-*.1 /usr/local/share/man/man1/
sudo mandb        # or `sudo makewhatis` on BSD
```

To regenerate them from a source checkout:

```bash
make man          # writes to man/man1/
```

### See also

- [`RELEASING.md`](RELEASING.md) — what each release artifact is, how it gets built, and how to verify SHA256 sums and SLSA attestations.
- [`homebrew/README.md`](homebrew/README.md) — Homebrew tap layout and bootstrap.

## Onboarding

If you are new to `bzr`, this is the fastest path from install to a working Bugzilla session.

### 1. Install `bzr`

Pick the one that fits your platform (see [Installation](#installation)
for the full menu and tradeoffs):

```bash
# macOS or Linux with Homebrew
brew tap randomparity/tap && brew install bzr

# Debian / Ubuntu
sudo apt install ./bzr_X.Y.Z-1_amd64.deb

# Fedora / RHEL
sudo dnf install ./bzr-X.Y.Z-1.x86_64.rpm

# Anywhere with Rust installed (no manpages — see Installation > Manual pages)
cargo install bzr --locked

# Local source checkout
cargo install --path . --locked
```

### 2. Configure your first server

```bash
# Preferred: read the API key from an environment variable
export BZR_API_KEY=YOUR_API_KEY
bzr config set-server myserver --url https://bugzilla.example.com --api-key-env BZR_API_KEY

# For Bugzilla 5.0 or earlier (XMLRPC)
bzr config set-server myserver --url https://bugzilla.example.com --api-key-env BZR_API_KEY --email "user@example.com"

# Legacy/insecure: stores the API key in config.toml and may leak via shell history
bzr config set-server myserver --url https://bugzilla.example.com --api-key YOUR_API_KEY
```

To store the API key in your OS keychain instead of an env var, see
[Credential storage](#credential-storage).

### 3. Verify authentication

```bash
bzr whoami
bzr server info
```

### 4. Run your first queries

```bash
# List the user's open bugs
bzr bug my --status \!CLOSED

# List open bugs in a product
bzr bug list --product MyProduct --status NEW

# View a specific bug
bzr bug view 12345

# Search across bugs
bzr bug search "crash on startup"
```

### 5. Save time with local workflows

```bash
# Save a reusable bug template
bzr template save fedora-kernel --product Fedora --component kernel

# Create a bug from the template
bzr bug create --template fedora-kernel --summary "Boot failure on 6.x" --description "System fails to boot after upgrade"

# Save a reusable query
bzr query save my-open-bugs --assignee you@example.com --status NEW --status ASSIGNED

# Run the saved query later
bzr query run my-open-bugs
```

## Quick Start

```bash
# Common day-to-day commands
bzr bug history 12345 --since 2025-01-01
bzr bug update 12345 --status RESOLVED --resolution FIXED --flag "review+(alice@example.com)"
bzr comment add 12345 --body "I can reproduce this on Fedora 42"
bzr comment tag 98765 --add needs-info
bzr attachment upload 12345 patch.diff --flag "review?(alice@example.com)"
bzr product list
bzr product view MyProduct
bzr user search "alice"
bzr group add-user --group testers --user alice@example.com
```

## Testing

For regular validation:

```bash
cargo test
make functional-test-all
```

To reproduce the CI coverage summary locally:

```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
make coverage
```

## CLI Reference

See [docs/bzr-cli.md](docs/bzr-cli.md) for the full command reference covering all commands and options.

## Agent Integration

### Claude Code

`bzr` works well with Claude Code skills because the CLI has stable subcommands, global `--json` output, and clear exit codes. See [docs/skills.md](docs/skills.md) for reusable skill definitions such as bug triage, investigation, patch review, and saved-query workflows.

Typical setup:

```text
~/.claude/skills/
  bzr-investigate/SKILL.md
  bzr-bug-summary/SKILL.md
  bzr-review/SKILL.md
```

Once installed, invoke them directly from Claude Code, for example `/bzr-investigate 12345`.

### IBM Bob

IBM Bob uses its own `SKILL.md` conventions under `.bob/skills/` or `~/.bob/skills/`. See [docs/bob-skills.md](docs/bob-skills.md) for Bob-specific examples and guidance tuned for `bzr`.

The same workflow design carries over cleanly:

- Prefer `bzr --json ...` so Bob receives structured data it can parse.
- Keep write operations explicit, for example `bzr bug update`, `bzr comment add --body`, and `bzr attachment upload`.
- Encode repeatable workflows such as "summarize bug", "review patch attachments", or "run saved query and report results" as Bob prompt templates.

The important compatibility point is that `bzr` is agent-friendly by default: global flags are consistent, machine-readable output is built in, and saved templates and queries let agents reuse local workflows without custom wrappers.

## JSON Output

All list and view commands support `--output json` for scripting and piping to tools like `jq`:

```bash
# Get bug IDs matching a search
bzr --output json bug search "memory leak" | jq '.[].id'

# Extract assignee from a bug
bzr --output json bug view 12345 | jq -r '.assigned_to'

# List attachment filenames
bzr --output json attachment list 12345 | jq -r '.[].file_name'

# Get product component names
bzr --output json product view Fedora | jq -r '.components[].name'

# List allowed status transitions from NEW
bzr --output json field list status | jq '.[] | select(.name == "NEW") | .can_change_to'
```

## Configuration & Authentication

Configuration is stored in `~/.config/bzr/config.toml` with support for multiple named servers. See [docs/bzr-cli.md](docs/bzr-cli.md#configuration-file-format) for the full file format.

## Authentication

`bzr` authenticates using Bugzilla API keys. Prefer `--api-key-env` so the secret stays out of `config.toml`, shell history, and most process listings. `bzr` warns when the config directory or file permissions are too broad on Unix systems. It also auto-detects whether your server supports header-based auth (`X-BUGZILLA-API-KEY`) or query parameter auth (`Bugzilla_api_key`), and caches the result. See [docs/bzr-cli.md](docs/bzr-cli.md#authentication) for details on generating and configuring API keys.

## Credential storage

`bzr` supports three ways to supply a Bugzilla API key, in increasing order of safety:

1. **Plaintext in `config.toml`** (`--api-key`) — simplest, but the key lives on disk in your config file.
2. **Environment variable** (`--api-key-env BZR_API_KEY`) — keeps the secret out of the config file; resolved at runtime.
3. **OS keychain** (via `bzr config set-keyring`) — stores the key in the system secret store (macOS Keychain, GNOME Keyring / KWallet via Secret Service on Linux, Windows Credential Manager). Requires the `keyring` Cargo feature, which is on by default.

Commands for managing keychain-backed credentials:

```bash
# Store an API key in the OS keychain for a server (prompts for the key)
bzr config set-keyring myserver

# Remove a keychain entry
bzr config unset-keyring myserver

# Move an existing plaintext / env-backed credential into the keychain
bzr config migrate-to-keyring myserver --yes
```

`bzr config show` labels each server's credential source so you can see
at a glance which mechanism is in use. See
[`docs/troubleshooting.md`](docs/troubleshooting.md) for diagnosing
keychain errors (locked keyring, missing Secret Service daemon, builds
compiled without the feature, etc.).

## TLS certificate pinning

By default, `bzr` validates server TLS using the operating system's CA
trust store. For self-hosted Bugzilla servers — especially those exposed
on the open internet — you may want stronger guarantees that the
connection is reaching the same server you initially trusted, even if a
CA in the trust store is later compromised.

`bzr` supports two pinning models on `bzr config set-server`:

- `--tls-ca-cert <path>`: pin a custom CA certificate (PEM file). The
  server must present a chain that verifies against this CA.
- `--tls-pin-sha256 <hex>`: pin the SHA-256 fingerprint of the server's
  leaf certificate Subject Public Key Info (SPKI). The server must
  present a leaf certificate whose SPKI matches this fingerprint.

### Trust on first use

If you don't already know the pin, use `--tls-pin-now`. `bzr` connects
once, captures the leaf certificate's SPKI fingerprint, prints it, and
prompts before storing it:

```sh
bzr config set-server my-bz https://bugzilla.example.com --tls-pin-now
```

Subsequent connections to `my-bz` verify the pin. If the server
presents a different leaf certificate (rotation, reissue, MITM) bzr
exits with `PinMismatch` and a hint suggesting `--tls-pin-now` to
re-pin or `--tls-pin-clear` to remove the pin.

### Clearing a pin

```sh
bzr config set-server my-bz --tls-pin-clear
```

Removes both `tls_ca_cert` and `tls_pin_sha256` for the server.

### Storage

Pins live in `~/.config/bzr/config.toml`, per-server, alongside other
server config. They are not stored in the OS keyring (which is
reserved for credentials). The full reference for these flags is in
[`docs/bzr-cli.md`](docs/bzr-cli.md).

## License

MIT

![Desloppify Score: 92.5](scorecard.png)
