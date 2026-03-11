# bzr - Bugzilla CLI

A command-line interface for Bugzilla servers, written in Rust. Inspired by the GitHub CLI (`gh`), `bzr` lets you search, view, create, and update bugs, manage comments and attachments, and switch between multiple Bugzilla instances — all from your terminal.

## Features

- **Bug management** — list, search, view, create, and update bugs
- **Comments** — list and add comments, with `$EDITOR` integration for composing
- **Attachments** — list, download, and upload file attachments with auto-detected MIME types
- **Multi-server** — configure and switch between multiple Bugzilla instances
- **Output formats** — human-readable tables (with colored status) or JSON for scripting
- **Secure auth** — API key sent via `X-BUGZILLA-API-KEY` header, never in query strings

## Installation

```
cargo install --path .
```

Requires Rust 1.70+.

## Quick Start

```bash
# Configure a Bugzilla server
bzr config set-server myserver --url https://bugzilla.example.com --api-key YOUR_API_KEY

# List open bugs in a product
bzr bug list --product MyProduct --status NEW

# View a specific bug
bzr bug view 12345

# Search for bugs
bzr bug search "crash on startup"

# Add a comment
bzr comment add 12345 --body "I can reproduce this on Fedora 42"

# Download an attachment
bzr attachment download 67890 -o patch.diff

# Upload a file to a bug
bzr attachment upload 12345 screenshot.png --summary "UI glitch screenshot"
```

## Command Reference

### Command Tree

```
bzr [--server <NAME>] [--output table|json]
├── bug
│   ├── list [--product <P>] [--component <C>] [--status <S>] [--assignee <A>] [--limit <N>]
│   ├── view <ID>
│   ├── search <QUERY> [--limit <N>]
│   ├── create --product <P> --component <C> --summary <S> [--version <V>]
│   │          [--description <D>] [--priority <P>] [--severity <S>] [--assignee <A>]
│   └── update <ID> [--status <S>] [--resolution <R>] [--assignee <A>]
│                    [--priority <P>] [--severity <S>] [--summary <S>] [--whiteboard <W>]
├── comment
│   ├── list <BUG_ID>
│   └── add <BUG_ID> [--body <TEXT>]
├── attachment
│   ├── list <BUG_ID>
│   ├── download <ATTACHMENT_ID> [-o <FILE>]
│   └── upload <BUG_ID> <FILE> [--summary <S>] [--content-type <MIME>]
└── config
    ├── set-server <NAME> --url <URL> --api-key <KEY>
    ├── set-default <NAME>
    └── show
```

### Global Options

| Option | Description |
|--------|-------------|
| `--server <NAME>` | Use a specific server from config instead of the default |
| `--output <FORMAT>` | Output format: `table` (default) or `json` |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### `bzr bug` — Bug Operations

#### `bzr bug list`

List bugs matching filter criteria.

```bash
bzr bug list --product Fedora --status ASSIGNED --limit 20
bzr bug list --assignee user@example.com
```

| Option | Description |
|--------|-------------|
| `--product <P>` | Filter by product name |
| `--component <C>` | Filter by component name |
| `--status <S>` | Filter by status (NEW, ASSIGNED, RESOLVED, etc.) |
| `--assignee <A>` | Filter by assignee email |
| `--limit <N>` | Max results (default: 50) |

#### `bzr bug view`

Display detailed information about a single bug.

```bash
bzr bug view 12345
bzr bug view 12345 --output json
```

#### `bzr bug search`

Full-text search using Bugzilla's quicksearch syntax.

```bash
bzr bug search "kernel panic"
bzr bug search "component:NetworkManager priority:high" --limit 10
```

| Option | Description |
|--------|-------------|
| `--limit <N>` | Max results (default: 50) |

#### `bzr bug create`

File a new bug.

```bash
bzr bug create --product Fedora --component kernel \
  --summary "Boot failure on 6.x" \
  --description "System hangs at initramfs" \
  --priority high --severity major
```

| Option | Required | Description |
|--------|----------|-------------|
| `--product <P>` | Yes | Product name |
| `--component <C>` | Yes | Component name |
| `--summary <S>` | Yes | One-line summary |
| `--version <V>` | No | Version (default: "unspecified") |
| `--description <D>` | No | Full description |
| `--priority <P>` | No | Priority level |
| `--severity <S>` | No | Severity level |
| `--assignee <A>` | No | Assignee email |

#### `bzr bug update`

Modify fields on an existing bug.

```bash
bzr bug update 12345 --status ASSIGNED --assignee dev@example.com
bzr bug update 12345 --status RESOLVED --resolution FIXED
```

| Option | Description |
|--------|-------------|
| `--status <S>` | New status |
| `--resolution <R>` | Resolution (FIXED, WONTFIX, DUPLICATE, etc.) |
| `--assignee <A>` | Reassign to email |
| `--priority <P>` | Set priority |
| `--severity <S>` | Set severity |
| `--summary <S>` | Update summary text |
| `--whiteboard <W>` | Set whiteboard text |

### `bzr comment` — Comment Operations

#### `bzr comment list`

List all comments on a bug.

```bash
bzr comment list 12345
bzr comment list 12345 --output json
```

#### `bzr comment add`

Add a comment to a bug. If `--body` is omitted, opens `$EDITOR` (falls back to `vi`).

```bash
bzr comment add 12345 --body "Confirmed on Fedora 42"
bzr comment add 12345   # opens editor
```

| Option | Description |
|--------|-------------|
| `--body <TEXT>` | Comment text (opens `$EDITOR` if omitted) |

### `bzr attachment` — Attachment Operations

#### `bzr attachment list`

List all attachments on a bug.

```bash
bzr attachment list 12345
```

#### `bzr attachment download`

Download an attachment by its ID. Saves to the original filename by default.

```bash
bzr attachment download 67890
bzr attachment download 67890 -o /tmp/patch.diff
```

| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output file path (default: original filename) |

#### `bzr attachment upload`

Upload a file as an attachment to a bug. MIME type is auto-detected from the file extension if not specified.

```bash
bzr attachment upload 12345 screenshot.png
bzr attachment upload 12345 data.csv --summary "Performance data" --content-type text/csv
```

| Option | Description |
|--------|-------------|
| `--summary <S>` | Description of the attachment (default: filename) |
| `--content-type <MIME>` | MIME type (auto-detected if omitted) |

### `bzr config` — Configuration Management

Configuration is stored in `~/.config/bzr/config.toml`. Multiple servers can be configured and switched between using aliases.

#### `bzr config set-server`

Add or update a named server configuration.

```bash
bzr config set-server redhat --url https://bugzilla.redhat.com --api-key abc123
bzr config set-server mozilla --url https://bugzilla.mozilla.org --api-key xyz789
```

The first server added is automatically set as the default.

#### `bzr config set-default`

Change which server is used when `--server` is not specified.

```bash
bzr config set-default mozilla
```

#### `bzr config show`

Display the current configuration (API keys are masked).

```bash
bzr config show
```

## JSON Output

All list and view commands support `--output json` for scripting and piping to tools like `jq`:

```bash
# Get bug IDs matching a search
bzr --output json bug search "memory leak" | jq '.[].id'

# Extract assignee from a bug
bzr --output json bug view 12345 | jq -r '.assigned_to'

# List attachment filenames
bzr --output json attachment list 12345 | jq -r '.[].file_name'
```

## Configuration File Format

`~/.config/bzr/config.toml`:

```toml
default_server = "redhat"

[servers.redhat]
url = "https://bugzilla.redhat.com"
api_key = "your-api-key-here"

[servers.mozilla]
url = "https://bugzilla.mozilla.org"
api_key = "another-api-key"
```

## Authentication

`bzr` authenticates using Bugzilla API keys sent via the `X-BUGZILLA-API-KEY` HTTP header. To generate an API key:

1. Log in to your Bugzilla instance
2. Go to **Preferences > API Keys**
3. Generate a new key
4. Add it with `bzr config set-server`

## License

MIT
