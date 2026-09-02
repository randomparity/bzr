<div align="center">
  <img src="docs/assets/bzr-logo.png" alt="bzr logo" width="128">
</div>

# bzr - Bugzilla CLI

[![CI](https://github.com/randomparity/bzr/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/randomparity/bzr/actions/workflows/ci.yml)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=randomparity_bzr&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=randomparity_bzr)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/randomparity/bzr/badge)](https://scorecard.dev/viewer/?uri=github.com/randomparity/bzr)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/randomparity/bzr/blob/main/LICENSE)
[![MSRV: 1.89](https://img.shields.io/badge/MSRV-1.89-blue.svg)](https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/)
[![crates.io](https://img.shields.io/crates/v/bzr.svg)](https://crates.io/crates/bzr)

A command-line interface for Bugzilla servers, written in Rust. Inspired by the
GitHub CLI (`gh`), `bzr` lets you search, view, create, and update bugs, manage
comments and attachments, switch between multiple Bugzilla instances, and use
REST, XML-RPC, or hybrid API transport as each server requires — all from your
terminal.

<div align="center">
  <img src="docs/assets/bzr-demo.gif" alt="Animated terminal session: listing bugs in a colored table, viewing a bug, adding a comment, updating its status, and piping JSON output to jq">
</div>

## Quick start

```bash
# Install (Homebrew shown; see Installation for .deb/.rpm, one-line installer, cargo)
brew tap randomparity/tap && brew install bzr

# Point bzr at a server — public servers work read-only with no credentials
bzr config set-server myserver --url https://bugzilla.example.com

# Search, view, comment
bzr bug list --product MyProduct --status NEW
bzr bug view 12345
bzr comment add 12345 --body "Reproduced on Fedora 42"

# JSON output for scripting
bzr --output json bug view 12345 | jq -r '.data.assigned_to'
```

See [Getting started](#getting-started) for authentication and a fuller
walkthrough.

## Features

- **Bugs** — list, search, view, create, clone, update, and batch-update bugs; view change history; see your own bugs at a glance (`bzr bug my`)
- **Comments & attachments** — list and add comments (with `$EDITOR` integration), tag comments, and download, upload, and update attachments with auto-detected MIME types
- **Flags** — set, request, and clear flags on bugs and attachments
- **Templates & saved queries** — save reusable bug field templates and search queries, then run them by name
- **Admin & metadata** — create and update products, components, users, and groups; view classifications; look up valid values for bug fields
- **Multi-server** — configure several Bugzilla instances, switch per invocation, and check any server's version and extensions (`whoami`, `server info`)
- **Works with old and new servers** — REST, XML-RPC, or hybrid API transport, auto-detected per server
- **Scriptable output** — human-readable tables with colored status, JSON in a versioned envelope, or NDJSON, plus embedded JSON Schemas for every output object
- **Secure by default** — header-based API-key auth with fallback for older servers, OS-keychain credential storage, and TLS CA / fingerprint pinning

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

**Windows PowerShell (x86_64 / ARM64):**

```powershell
irm https://raw.githubusercontent.com/randomparity/bzr/main/install.ps1 | iex
```

Run this command in PowerShell, not Command Prompt. The installer asks before
adding `bzr` to your user PATH and current PowerShell session.

The installer detects your platform, verifies the SHA-256 checksum
against the published `SHA256SUMS`, and drops the binary in
`~/.local/bin` (Unix) or `%LOCALAPPDATA%\Programs\bzr` (Windows).
The Unix installer never modifies PATH. The Windows installer modifies PATH
only with confirmation; neither installer modifies system files.

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

Requires Rust 1.89+. Same manpage caveat as `cargo install bzr` — see
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
[`docs/troubleshooting.md`](https://github.com/randomparity/bzr/blob/main/docs/troubleshooting.md)
for diagnosing keychain errors.

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

### Shell completion

`bzr completion <SHELL>` prints a completion script to stdout for `bash`,
`zsh`, `fish`, `powershell`, or `elvish`. The script is generated from bzr's
live command tree, so it always matches the installed binary. Install with one
line per shell:

```bash
# bash (the directory must exist and bash-completion must be active)
bzr completion bash > ~/.local/share/bash-completion/completions/bzr

# zsh (~/.zfunc must be on $fpath before compinit; restart the shell after)
bzr completion zsh > ~/.zfunc/_bzr

# fish
bzr completion fish > ~/.config/fish/completions/bzr.fish

# powershell (append to your profile)
bzr completion powershell >> $PROFILE
```

### See also

- [`RELEASING.md`](https://github.com/randomparity/bzr/blob/main/RELEASING.md) — what each release artifact is, how it gets built, and how to verify SHA256 sums and SLSA attestations.
- [`homebrew/README.md`](https://github.com/randomparity/bzr/blob/main/homebrew/README.md) — Homebrew tap layout and bootstrap.

## Getting started

If you are new to `bzr`, this is the fastest path from install to a working
Bugzilla session. Install first — see [Installation](#installation) for the
full menu and tradeoffs — then:

### 1. Configure your first server

```bash
# Public read-only exploration can omit credentials
bzr config set-server public-bz --url https://bugzilla.example.org
bzr --server public-bz bug list --product Firefox --limit 10
bzr --server-url https://bugzilla.example.org bug view 12345
bzr --server-url https://bugzilla.internal --server-tls-ca-cert /etc/pki/internal-ca.pem server info

# Preferred: read the API key from an environment variable
export BZR_API_KEY=YOUR_API_KEY
bzr config set-server myserver --url https://bugzilla.example.com --api-key-env BZR_API_KEY

# For the Bugzilla 5.0/5.2 email-backed whoami fallback
bzr config set-server myserver --url https://bugzilla.example.com --api-key-env BZR_API_KEY --email "user@example.com"

# Stateless form of the same fallback (5.3+/BMO-derived servers use native whoami)
bzr --server-url https://bugzilla.example.com --server-api-key-env BZR_API_KEY --server-email "user@example.com" whoami

# Legacy/insecure: stores the API key in config.toml and may leak via shell history
bzr config set-server myserver --url https://bugzilla.example.com --api-key YOUR_API_KEY
```

To store the API key in your OS keychain instead of an env var, see
[Credential storage](#credential-storage).

### 2. Verify connectivity and authentication

```bash
bzr server info
bzr whoami  # requires credentials
```

### 3. Run your first queries

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

### 4. Save time with local workflows

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

### Everyday commands

```bash
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

## JSON output

All list and view commands support `--output json` for scripting and piping to
tools like `jq`. Output is wrapped in a versioned envelope —
`{"schema_version": "2.1.0", "data": <payload>}` — so read fields under `.data`
(`--output ndjson` records stay bare). See
[docs/bzr-cli.md](https://github.com/randomparity/bzr/blob/main/docs/bzr-cli.md#json-output)
for the stability policy.

```bash
# Get bug IDs matching a search
bzr --output json bug search "memory leak" | jq '.data[].id'

# Extract assignee from a bug
bzr --output json bug view 12345 | jq -r '.data.assigned_to'

# List attachment filenames
bzr --output json attachment list 12345 | jq -r '.data[].file_name'

# Get product component names
bzr --output json product view Fedora | jq -r '.data.components[].name'

# List allowed status transitions from NEW
bzr --output json field list status | jq '.data[] | select(.name == "NEW") | .can_change_to'
```

## Configuration & authentication

Configuration is stored in `~/.config/bzr/config.toml` with support for
multiple named servers. Point bzr at a different file with the global
`--config <PATH>` flag or the `BZR_CONFIG` environment variable (the flag
wins) — handy for CI, throwaway agent runs, and per-profile configs. See
[docs/bzr-cli.md](https://github.com/randomparity/bzr/blob/main/docs/bzr-cli.md#configuration-file-format)
for the full file format.

`bzr` authenticates using Bugzilla API keys when a command needs an identity
or write access. Public Bugzilla servers can omit credentials for read-only
commands; writes and identity-derived reads such as `whoami` and `bug my`
fail fast until a credential source is configured. Prefer `--api-key-env` so
the secret stays out of `config.toml`, shell history, and most process
listings. `bzr` warns when the config directory or file permissions are too
broad on Unix systems. It also auto-detects whether your server supports
header-based auth (`X-BUGZILLA-API-KEY`) or query parameter auth
(`Bugzilla_api_key`), and caches the result. See
[docs/bzr-cli.md](https://github.com/randomparity/bzr/blob/main/docs/bzr-cli.md#authentication)
for details on generating and configuring API keys.

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
[`docs/troubleshooting.md`](https://github.com/randomparity/bzr/blob/main/docs/troubleshooting.md)
for diagnosing keychain errors (locked keyring, missing Secret Service
daemon, builds compiled without the feature, etc.).

## TLS certificate pinning

By default, `bzr` validates server TLS using the operating system's CA
trust store. For self-hosted Bugzilla servers — especially those exposed
on the open internet — you may want stronger guarantees that the
connection is reaching the same server you initially trusted, even if a
CA in the trust store is later compromised.

`bzr` supports two pinning models on `bzr config set-server`:

- `--tls-ca-cert <path>`: pin a custom CA certificate (PEM file). The
  server must present a chain that verifies against this CA.
- `--tls-pin-sha256 <pin>`: pin the SHA-256 fingerprint of the server's
  leaf certificate in `sha256//<base64>` format. The server must present
  a leaf certificate whose DER bytes hash to this fingerprint.

### Trust on first use

If you don't already know the pin, use `--tls-pin-now`. `bzr` connects
once, captures the leaf certificate fingerprint, prints it, and prompts
before storing it. When issuer DER can be extracted, `bzr` also stores a
DER-backed issuer guard so future issuer changes can be reported as
`IssuerChanged`; the human-readable `tls_pin_issuer` is display-only.

```sh
bzr config set-server my-bz --url https://bugzilla.example.com --tls-pin-now
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

### Ad-hoc TLS for stateless runs

The same trust shapes are available without writing config by using
prefixed global flags with `--server-url`:

```sh
bzr --server-url https://bugzilla.internal --server-tls-ca-cert /etc/pki/internal-ca.pem server info
bzr --server-url https://bugzilla.internal --server-tls-pin-sha256 sha256//BASE64PIN bug view 123
bzr --server-url https://bugzilla.internal --server-tls-pin-now server info
```

`--server-tls-insecure`, `--server-tls-ca-cert`,
`--server-tls-pin-sha256`, and `--server-tls-pin-now` are mutually
exclusive, apply only to the current process, and are never persisted.
`--server-tls-pin-now` trusts the first certificate presented for this
invocation only; use an explicit CA or fingerprint when CI needs
reproducible trust.

### Storage

Pins live in `~/.config/bzr/config.toml`, per-server, alongside other
server config. They are not stored in the OS keyring (which is
reserved for credentials). The full reference for these flags is in
[`docs/bzr-cli.md`](https://github.com/randomparity/bzr/blob/main/docs/bzr-cli.md).

## Agent integration

`bzr` ships a set of installable agent skills sourced from
[`content/skills/`](https://github.com/randomparity/bzr/tree/main/content/skills),
with standalone installers under
[`agent-skills/`](https://github.com/randomparity/bzr/tree/main/agent-skills)
that teach AI coding agents to use the CLI correctly — the `--json` contract,
the authentication model, the read-before-write rule, and the real command
surface. They live in this repo so they track the CLI as it changes (CI runs a
command-surface drift check against the built binary).

When `bzr` is installed, use its offline, release-matched payload and choose the scope
explicitly:

```bash
bzr skills install --agent all --global
bzr skills install --agent all --project .
```

`standard`/`bob`/`codex` install to `~/.agents/skills`; `claude` installs to
`~/.claude/skills`; `all` does both. Project scope uses the same relative layouts
beneath the selected repository. The command requires exactly one of `--global` and
`--project <PATH>` and never needs Bugzilla configuration or network access.

For machines without `bzr`, the standalone installers under `agent-skills/` fetch the
payload from `main` by default; set `BZR_SKILL_REF` to pin a tag or commit. That payload
may differ from the release-matched copy embedded in an installed binary. See
[`agent-skills/README.md`](https://github.com/randomparity/bzr/blob/main/agent-skills/README.md)
for standalone installation and development details.

The skills shell out to the real `bzr` binary and are agent-agnostic: global
flags are consistent, machine-readable output is built in, and saved templates
and queries let agents reuse local workflows without custom wrappers.

The `bzr-weekly-status` skill keeps versioned local snapshots and compares each report with the
newest compatible prior run. This asciinema-based demonstration creates a baseline, changes live
Bugzilla state, then runs the shipped selector, comparator, and atomic publisher:

![Snapshot-based weekly status demonstration](docs/assets/bzr-weekly-status-demo.gif)

Regenerate it against a fresh functional container with
`tools/record-demo.sh --weekly-status`.

## Documentation

- [CLI reference](https://github.com/randomparity/bzr/blob/main/docs/bzr-cli.md) — every command, flag, and output format.
- [Changelog](https://github.com/randomparity/bzr/blob/main/CHANGELOG.md) — what's new in each release.
- [Troubleshooting](https://github.com/randomparity/bzr/blob/main/docs/troubleshooting.md) — keychain, TLS, and connectivity diagnostics.

## Contributing

See the [contribution policy](https://github.com/randomparity/bzr/blob/main/CONTRIBUTING.md)
for development setup, how to run the test suites, required verification,
pull-request expectations, and issue-reporting guidance.

## Why the name `bzr`?

`bzr` was historically the command for [GNU Bazaar](https://en.wikipedia.org/wiki/GNU_Bazaar), a version-control system. Bazaar's last release was in 2016, Canonical [announced its retirement in 2025](https://blog.launchpad.net/general/phasing-out-bazaar-code-hosting), and its maintained successor **Breezy renamed its command to `brz`** — so the `bzr` name is effectively being vacated by the VCS world. This project keeps `bzr` (it reads as "Bugzilla" the way `gh` reads as "GitHub"). If you also have GNU Bazaar/Breezy installed and want to keep using `bzr` for it, alias this tool instead, e.g. `alias bz=bzr`. See [the decision record](https://github.com/randomparity/bzr/blob/main/docs/decisions/0001-bzr-command-name.md) for the full rationale.

## License

MIT
