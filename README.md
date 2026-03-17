# bzr - Bugzilla CLI

A command-line interface for Bugzilla servers, written in Rust. Inspired by the GitHub CLI (`gh`), `bzr` lets you search, view, create, and update bugs, manage comments and attachments, and switch between multiple Bugzilla instances — all from your terminal.

## Features

- **Bug management** — list, search, view, create, update, and view change history
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

```bash
cargo install --path .
```

Requires Rust 1.70+.

## Quick Start

```bash
# Configure a Bugzilla server (add --email for Bugzilla 5.0 or earlier)
bzr config set-server myserver --url https://bugzilla.example.com --api-key YOUR_API_KEY

# List open bugs in a product
bzr bug list --product MyProduct --status NEW

# View a specific bug
bzr bug view 12345

# Search for bugs
bzr bug search "crash on startup"

# View change history (optionally since a date)
bzr bug history 12345 --since 2025-01-01

# Create a bug
bzr bug create --product Fedora --component kernel --summary "Boot failure on 6.x"

# Update a bug with flags
bzr bug update 12345 --status RESOLVED --resolution FIXED --flag "review+(alice@example.com)"

# Add a comment (opens $EDITOR if --body omitted)
bzr comment add 12345 --body "I can reproduce this on Fedora 42"

# Tag a comment
bzr comment tag 98765 --add needs-info

# Upload an attachment with a review flag
bzr attachment upload 12345 patch.diff --flag "review?(alice@example.com)"

# Check who you're authenticated as
bzr whoami

# Check server version and extensions
bzr server info

# List products, view details
bzr product list
bzr product view MyProduct

# Search for users
bzr user search "alice"

# Manage group membership
bzr group add-user --group testers --user alice@example.com
```

## CLI Reference

See [docs/bzr-cli.md](docs/bzr-cli.md) for the full command reference covering all commands and options.

## Claude Code Skills

See [docs/skills.md](docs/skills.md) for writing Claude Code skills that automate Bugzilla workflows with bzr.

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

`bzr` authenticates using Bugzilla API keys. When you run `bzr config set-server`, it auto-detects whether your server supports header-based auth (`X-BUGZILLA-API-KEY`) or query parameter auth (`Bugzilla_api_key`), and caches the result. See [docs/bzr-cli.md](docs/bzr-cli.md#authentication) for details on generating and configuring API keys.

## License

MIT
