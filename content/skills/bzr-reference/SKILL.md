---
name: bzr-reference
description: Use when working with Bugzilla via the bzr CLI — viewing, searching, creating, or updating bugs, comments, attachments, products, or saved queries. Covers the command surface, the --json output contract, authentication, and the read-before-write rule.
---

# Using bzr (Bugzilla CLI)

`bzr` is a command-line client for Bugzilla. Use it to view, search, file, and
update bugs and their comments, attachments, flags, and saved queries against a
configured Bugzilla server. It shells out to a real server — there is no local
database.

## Output: prefer `--json`

When you will parse output, pass `--json` (bzr's canonical shorthand;
`--output json` is the long form). Output also auto-flips to JSON when stdout is
piped, but pass `--json` explicitly so intent is clear. Pipe to `jq`:

```
bzr bug view 12345 --json | jq -r '.data.summary'
bzr bug search "crash on boot" --json | jq -r '.data[].id'
```

See `reference/json-recipes.md` for extraction patterns.

## Server access

A named server is configured with `bzr config set-server`. Omit credentials for
public read-only Bugzilla exploration; add an API key source only when you need
writes or identity-derived commands such as `whoami` and `bug my`:

```
bzr config set-server public-bz --url https://bugzilla.example.com
bzr --server public-bz server info --json

export BZR_API_KEY=...
bzr config set-server my-bz --url https://bugzilla.example.com --api-key-env BZR_API_KEY
bzr whoami --json
```

For one-off runs, skip config entirely with `--server-url <url>`. It also works
without `--server-api-key-env` for public read-only commands:

```
bzr --server-url https://bugzilla.example.com bug view 12345 --json
bzr --server-url https://bugzilla.example.com --server-api-key-env BZR_API_KEY whoami --json
```

Inline server TLS trust flags are `--server-tls-insecure`,
`--server-tls-ca-cert <path>`, `--server-tls-pin-sha256 <pin>`, and
`--server-tls-pin-now`. Named servers use the matching `config set-server`
`--tls-*` flags. If `whoami` fails, see the `bzr-setup` skill.

## Command groups

The full surface (with one example each) is in `reference/commands.md`. The
groups: `bug`, `comment`, `attachment`, `config`, `product`, `field`, `user`,
`group`, `whoami`, `server`, `classification`, `component`, `template`, `query`,
plus two local (no-network) top-level commands: `completion` (shell completion
scripts) and `schema` (published JSON Schemas for `--json` output and
`--from-json` input).

Surface facts agents get wrong:

- **Components** have their own read verbs as of 0.5.0:
  `bzr component list --product <product>` and
  `bzr component view <product> <component>`. `bzr product view <product>`
  still lists components inline alongside versions and milestones, which is
  often the convenient one-shot view.
- **Flags** are changed through `bzr bug update --flag ...` — there is **no**
  standalone `bzr flags` command.
- **Relationships** have a dedicated verb as of 0.7.0: `bzr bug links <id>`
  emits one record per related bug across all six Bugzilla relationship types,
  with `--recursive --depth <N>` for a bounded, cycle-safe walk. Do not
  reconstruct a graph by repeatedly parsing `bug view`.
- **Server behavior** is `bzr server capabilities` (0.7.0) — auth modes,
  status transitions, custom fields, attachment-size limit. `bzr server info`
  reports version and extensions only. Read capabilities before planning a
  mutation against an unfamiliar server.

## Global flags worth knowing

These work on any command (place them before the subcommand):

- `--json` / `--output ndjson` — JSON, or newline-delimited JSON (one record per
  line) for streaming into `jq -c` or agents.
- `--dry-run` — preview supported bug mutations and product/component/user/group
  create/update without writing; prints the would-be payload.
- `-y` / `--yes` — skip the confirmation prompt for a batch mutation touching
  more than 10 bugs (interactive terminals only).
- `--timeout <secs>` / `--retry <n>` — tune per-request timeout and transient
  retry (reads only, with backoff).
- `--progress ndjson` — stream NDJSON progress events on stderr for long
  operations: `page`/`done` for `bug list`/`search --paginate` and `query run
  --paginate`, `batch`/`done` for `bug create`/`update --from-json` array form,
  and a terminal `error` on failure. stdout is unaffected, so combine it with
  `--json`/`--output ndjson` and parse stderr line-by-line (skip lines without an
  `event` key) to show "page N / item N of M" while a batch runs.
- `--config <path>` — use an alternate `config.toml` (sandboxes the run).
- `--server-url <url>` (+ `--server-api-key-env <env>`, optional
  `--server-email <email>`) — a fully stateless inline server, no config file
  needed; ideal for CI and agents. See the `bzr-setup` skill.
- `--server-tls-insecure` / `--server-tls-ca-cert <path>` /
  `--server-tls-pin-sha256 <pin>` / `--server-tls-pin-now` — TLS trust controls
  for that one inline server invocation.

## Structured input

`bug create`, `bug update`, and admin create/update commands for products,
components, users, and groups accept `--from-json <path|->`. Explicit CLI flags
override matching JSON fields, and unknown JSON keys exit 7 instead of being
ignored. Bugs support object and array payloads; admin resources accept one
object payload. Use `bzr schema <name>` to inspect the contract, e.g.
`bug-create-input`, `bug-update-input`, `product-update-input`,
`component-create-input`, or `error`. Output shapes are published too —
`envelope` (the `{schema_version, data}` wrapper), `history`, `whoami`,
`server-capabilities`, `compound-create-result`, and one per resource. `bzr
schema` with no name lists every published name; treat that as authoritative
rather than any list written in prose.

## Errors: branch on the structured object, never on prose

Under `--json` / `--output ndjson` a failure writes an `error` object to
**stderr** (stdout stays clean) and exits non-zero. Detect failure by exit code,
then read `error.type`, then read the keys that type carries. Never grep the
message: it is server-supplied and changes between deployments.

| `type` | exit | keys beyond `type`/`message`/`exit_code` |
|---|---|---|
| `not_found` | 2 | `resource`, `identifier` |
| `config` | 3 | — |
| `api` | 4 | `api_code` |
| `http` | 5 | `status` |
| `io` | 6 | — |
| `input` | 7 | `field`, `value` |
| `deserialize` | 8 | — |
| `auth` | 9 | — |
| `data_integrity` | 10 | — |
| `batch_partial_failure` | 11 | `succeeded`, `failed` (counts) |
| `keyring` | 12 | — |
| `tls` | 13 | `server`, `expected`, `actual` (pin mismatch or issuer change) |
| `collision` | 14 | `bug_id`, `last_change_time`, `if_match_token` |

That is the whole set. `config` (3) is the one a fresh agent hits first — no
server configured — so handle it before assuming a network or permission
problem.

Three traps worth knowing:

- **Exit 2 no longer means "absent or restricted".** A bug the
  server declines to return for another reason — most often *you are not
  authorized* — exits **4** (`api`) carrying the server's `api_code` and
  message. Exit 2 now means the server returned an empty result with no error
  at all. Code branching on 2 alone to mean "no such bug" will misreport a
  permission failure as a missing bug; branch on **2 or 4**, and read
  `error.message` to tell them apart.
- For a partial batch the error carries only `succeeded`/`failed` **counts**;
  the per-element `failed[]` rows are in the result body on **stdout**.
- **A usage error is not in this table.** clap rejects a malformed invocation
  (unknown flag, bad enum value, out-of-range number) with exit 2 and plain
  text on stderr, with no `error` object at all — even under `--json`. So
  before parsing stderr as JSON on exit 2, check that it parses.

`bzr schema error` is the full contract for the keys.

## Cardinal rules

- **Prefer `--json` when parsing.** Tables are for humans.
- **Read before write.** Never run `bzr bug update` without first viewing the
  bug's current state (`bzr bug view`), or you risk clobbering fields. The
  `bzr-triage-bug` skill walks this through.
- **Keep writes explicit and minimal.** Change only the fields you intend to.

This reference is authored against **bzr 0.8.2**. If `bzr --version` is much
newer and a command here is rejected, the surface may have moved; check
`bzr <group> --help`.
