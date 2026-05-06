# bzl → bzr Workflow-Parity Review

**Date:** 2026-05-06
**Branch:** `docs/bzl-parity-review`
**Author:** review of `reference/bzl/` against `bzr` 0.4.0-dev (a90c4b00)

## 1. Summary

`reference/bzl/` is a checked-in copy of bzl, a third-party Bugzilla
script collection (Python, `bzl-*`, version 2.2.1, dated 2018-era). After
a tester opened #152 against `bzr bug search` while comparing it to
`bzl-search`, this review walks the entire bzl surface to surface
*workflow* gaps proactively — not flag-for-flag parity, but cases where
bzl can accomplish a tester workflow that bzr cannot.

**Scope rules** (chosen during brainstorming):

- **Workflow parity, not capability parity.** Renamed flags, restructured
  output, and printf-style format strings are explicitly *not* gaps. We file
  an issue only when bzr cannot accomplish the workflow at all.
- **Bzl-internal carve-outs.** Scripts and flags tied to bzl's origin
  environment (custom statuses, custom resolutions, custom fields, the
  `backlog±N` whiteboard convention, cookie-based auth against the
  script's hardcoded server hosts) are noted as 🔒 out-of-scope and not
  filed.
- **Issues filed in batch.** Each gap becomes a tracked GitHub issue with a
  proposed functional-test addition. Tests ship with the implementation PRs
  that close the issues, not as a separate stream.

## 2. At-a-glance matrix

| bzl script                    | Purpose                              | bzr command(s)                                | Status | Notes                                                              |
|-------------------------------|--------------------------------------|-----------------------------------------------|--------|--------------------------------------------------------------------|
| `bzl-attachment-add`          | Upload attachment to bug(s)          | `bzr attachment upload`                       | ❌    | No `--comment` alongside upload; no `--is-patch` at upload         |
| `bzl-attachment-get`          | Download attachment(s)               | `bzr attachment download`                     | ❌     | One attachment per call; no bulk-by-bug                            |
| `bzl-attachment-list`         | List attachments by bug or attach-id | `bzr attachment list`                         | ⚠️    | Single bug only; can't list by attachment-id alone                 |
| `bzl-backlog`                 | Whiteboard `backlog±N` priority list | —                                             | 🔒    | Bzl-internal whiteboard convention                                  |
| `bzl-backlog-update`          | Whiteboard `backlog±N` mutation      | —                                             | 🔒    | Bzl-internal whiteboard convention                                  |
| `bzl-clone`                   | Clone with bzl-specific custom field | `bzr bug clone`                               | ✅     | bzr clone is strictly more capable                                 |
| `bzl-clones`                  | Recursive clone-tree walk            | —                                             | 🔒    | Depends on a bzl-specific custom field                              |
| `bzl-comments`                | List comments                        | `bzr comment list`                            | ✅/⚠️  | `--since` matches `--new-since`; `--nowrap` to verify              |
| `bzl-config`                  | Local config (env toggle)            | `bzr config show`/`set-server`/`set-default`  | ✅     | bzr's multi-server config is broader                               |
| `bzl-create`                  | Create bug, `$EDITOR` fallback       | `bzr bug create`                              | ❌     | No `$EDITOR` fallback; no `--description-file`                     |
| `bzl-get`                     | View bug(s)                          | `bzr bug view`                                | ❌     | Single bug only; no `--permissive`                                 |
| `bzl-group`                   | Add/remove/list group membership     | `bzr group add-user`/`remove-user`/`list-users` | ✅   | bzr is strictly more capable                                       |
| `bzl-log`                     | Bug change history                   | `bzr bug history`                             | ✅/⚠️  | bzr is single-bug; bzl accepts list                                |
| `bzl-login`                   | Cookie auth                          | `bzr config set-server` + `set-keyring`       | 🔒    | Replaced by API-key + keyring model                                |
| `bzl-product-list`            | List products                        | `bzr product list` / `view`                   | ✅     |                                                                    |
| `bzl-search`                  | Structured + URL bug search          | `bzr bug list` / `search`                     | ❌     | Missing date-range and several field filters                       |
| `bzl-update`                  | Update bug fields                    | `bzr bug update`                              | ❌     | No `--comment`, `--dupe-of`; missing list mutations & many fields  |
| `contrib/bzl-close`           | Bzl-internal status-chain wrapper    | —                                             | 🔒    | Custom statuses specific to bzl's origin                            |

Legend: ✅ covered · ⚠️ caveat (no issue) · ❌ workflow gap (issue filed) · 🔒 Bzl-internal / out-of-scope (not filed)

## 3. Workflow review

### 3.1 Auth & config

**bzl path.** `bzl-login` (`reference/bzl/bzl-login:1`) prompts for a
username/password and writes a Bugzilla cookie (`Bugzilla_login`,
`Bugzilla_logincookie`) to a legacy `.rc`-style config file in `$HOME`
(`reference/bzl/bzlrpc.py:228`). `bzl-config` toggles between two
hardcoded server hostnames in a `production` / `test` map
(`reference/bzl/bzlrpc.py:14`).

**bzr path.** `bzr config set-server <name> --url ... --api-key-env BZR_API_KEY`
(or `--api-key`/`set-keyring`); multiple named servers; `bzr config
set-default` picks one. `bzr whoami` confirms identity.

**Gaps.** None. The bzl model (cookie auth, two hardcoded servers) is
obsolete relative to Bugzilla 5.x's API-key model that bzr targets, and the
multi-server config strictly subsumes the production/test toggle.

**Status.** ✅ covered.

### 3.2 Read-only inspection

**bzl path.** `bzl-get`, `bzl-search`, `bzl-comments`, `bzl-log`,
`bzl-product-list`. All accept variable inputs and use the pre-Bugzilla-5
XML-RPC `Bug.search` / `Bug.get` / `Bug.history` / `Product.get` endpoints.

**bzr path.** `bzr bug view`, `bzr bug list`, `bzr bug search`, `bzr bug
history`, `bzr comment list`, `bzr product list` / `view`.

**Gaps.**

- **`bzr bug view` is single-bug.** `bzl-get` (`reference/bzl/bzl-get:91`)
  accepts `nargs='+'` for bug IDs and a `--permissive` flag that returns
  partial results when some are inaccessible. Tester workflow: "show me bugs
  100, 200, 300 in one command." Today: three calls or `xargs`. Filed as
  **Issue A** below.
- **`bzr bug list` is missing date-range filters.** `bzl-search` accepts
  `--creation-time` and `--last-change-time`
  (`reference/bzl/bzl-search:36-52`). Tester workflow: "bugs modified since
  2026-04-01." `bzr bug list` has no equivalent. Filed as **Issue B**.
- **`bzr bug list` is missing several field filters.** `bzl-search` accepts
  `--whiteboard`, `--target-milestone`, `--version`, `--op-sys`,
  `--platform`, `--resolution`, `--qa-contact`, `--url`, `--alias`
  (`reference/bzl/bzl-search:58-169`). `bzr bug list` covers `--alias` but
  not the others. Filed as **Issue C** (umbrella).
- **`bzr bug history` is single-bug.** `bzl-log` (`reference/bzl/bzl-log:25`)
  accepts `nargs='+'`. Marked ⚠️ (history output is naturally per-bug; the
  workflow value of multi-bug history is small) — *not* filed.
- **`bzr comment list` raw output.** `bzl-comments --nowrap`
  (`reference/bzl/bzl-comments:42`) prints unwrapped comment text. Whether
  bzr's table output already does this needs confirmation; if it does, no
  gap. Marked ⚠️.

**Status.** ❌ Two filed gaps (A, B), one umbrella (C), two caveats.

### 3.3 Bug lifecycle (create / clone / update / close)

**bzl path.** `bzl-create`, `bzl-clone`, `bzl-update`, and the bzl-internal
shell wrapper `contrib/bzl-close`. `bzl-create` opens `$EDITOR` with a
templated buffer (summary + commented field info) when no description is given
(`reference/bzl/bzl-create:163-227`). `bzl-update` supports rich list-mutation
syntax for `blocks`/`depends_on`/`keywords`/`cc`/`groups`/`see_also` using
`+`/`-`/bare-set prefixes (`reference/bzl/bzl-update:263-290`), private
comments via `@@PRIVATE@@` prefix or `--private-comment`, and reading
comment bodies from a file via `--comment-file`.

**bzr path.** `bzr bug create` (requires `--summary`, optional `--template`),
`bzr bug clone` (richer than `bzl-clone`: `--add-depends-on`, `--add-blocks`,
`--no-cc`, `--no-keywords`, `--no-comment`), `bzr bug update`.

**Gaps.**

- **`bzr bug create` has no `$EDITOR` fallback.** `bzr comment add` opens
  `$EDITOR` when `--body` and stdin are absent (`bzr comment add --help`),
  but `bzr bug create` does not — `--summary` is strictly required and
  description must be given via `--description`. Tester workflow: typing
  `bzr bug create --product P --component C` and composing the summary +
  description in `$EDITOR` is a regression vs `bzl-create`. Filed as
  **Issue D**.
- **`bzr bug create` has no `--description-file`.** `bzl-create
  --description-file FILE` (`reference/bzl/bzl-create:142-158`) reads from
  a file. Tester workflow: scripted bug-creation pipelines that prepare
  long-form descriptions in files. Filed as **Issue E**.
- **`bzr bug update` cannot post a comment as part of the update.** `bzl-update
  --comment` (`reference/bzl/bzl-update:44`) and `--comment-file`
  (`reference/bzl/bzl-update:249`) and `--private-comment`
  (`reference/bzl/bzl-update:248`) post a comment atomically with the field
  changes. `bzr bug update --help` says "see bzr-comment-add(1) for adding
  a comment as part of a status change" — i.e. two API calls. This is the
  most common tester workflow regression: closing a bug with a comment is a
  single command in bzl. Filed as **Issue F**.
- **`bzr bug update` has no `--dupe-of`.** Marking a bug as duplicate is a
  routine triage operation (`reference/bzl/bzl-update:61-68`). Filed as
  **Issue G**.
- **`bzr bug update` is missing list-mutation flags for `keywords`, `cc`,
  `groups`, `see_also`.** bzr supports `--blocks-add`/`--blocks-remove` and
  `--depends-on-add`/`--depends-on-remove` but no equivalents for the other
  four list-typed fields covered by bzl
  (`reference/bzl/bzl-update:74-78,279-290`). Filed as **Issue H** (umbrella).
- **`bzr bug update` is missing several field flags.** `--alias`,
  `--deadline`, `--estimated-time`, `--remaining-time`, `--work-time`,
  `--reset-assigned-to`, `--reset-qa-contact` are all in `bzl-update`
  (`reference/bzl/bzl-update:16-22,56-60,69-73,124-144`) but not in `bzr
  bug update`. Filed as **Issue I** (umbrella).

**Status.** ❌ Six filed gaps (D, E, F, G, H, I).

### 3.4 Attachments

**bzl path.** `bzl-attachment-add` accepts multiple bug IDs/aliases for a
single upload (`reference/bzl/bzl-attachment-add:64-70`), an optional
`--comment` to post alongside the attachment
(`reference/bzl/bzl-attachment-add:27-29`), and `--is-patch` to mark the file
as a patch at upload time (`reference/bzl/bzl-attachment-add:30-34`).
`bzl-attachment-list` and `bzl-attachment-get` accept either bug IDs or
attachment IDs and operate in bulk (`reference/bzl/bzl-attachment-list:59-75`,
`reference/bzl/bzl-attachment-get:59-79`).

**bzr path.** `bzr attachment upload <BUG_ID> <FILE>` (single bug, single
file), `bzr attachment list <BUG_ID>` (single bug), `bzr attachment download
<ID>` (single attachment id), `bzr attachment update <ID>`.

**Gaps.**

- **`bzr attachment upload` cannot post a comment with the attachment.**
  Tester workflow: posting a patch with explanatory commentary is a
  one-call operation in bzl. Filed as **Issue J**.
- **`bzr attachment upload` has no `--is-patch` flag at upload time.** Today
  the patch flag must be set in a follow-up `bzr attachment update --is-patch
  true` call. Filed as **Issue K**.
- **`bzr attachment download` lacks bulk download.** No way to download all
  attachments for a bug, or multiple attachments by ID, in one call. bzl
  saves to a per-bug subdirectory (`reference/bzl/bzl-attachment-get:32-48`).
  Filed as **Issue L**.

Multi-bug *upload* (same file to many bugs) and multi-bug *list* are noted
as ⚠️ caveats — wrappable in a shell loop and rarely used; *not* filed
under the workflow-parity bar.

**Status.** ❌ Three filed gaps (J, K, L), two caveats.

### 3.5 Backlog / batch ops

**bzl path.** `bzl-backlog` is a search wrapper that sorts results by a
`backlog+N`/`backlog-N` token in the whiteboard
(`reference/bzl/bzl-backlog:131-188`); `bzl-backlog-update` is a shell
helper that mutates the same whiteboard token
(`reference/bzl/bzl-backlog-update:1-32`).

**bzr path.** None — the convention is bzl-internal.

**Gaps.** None to file. Testers using this convention can:

- Save the search side as a bzr query: `bzr query save backlog --whiteboard
  '+backlog'` (once Issue C is fixed).
- Use `bzr bug update --whiteboard <text>` for the mutation side once Issue
  H lands (whiteboard editing is already supported as an overwrite).

**Status.** 🔒 Bzl-internal.

### 3.6 Group / user management

**bzl path.** `bzl-group --add`/`--remove`/`--list` against a single group
(`reference/bzl/bzl-group:43-89`). `--list` filters users by a hardcoded
email-domain substring (`reference/bzl/bzl-group:80`).

**bzr path.** `bzr group add-user`, `remove-user`, `list-users`, `view`,
`create`, `update`, plus `bzr user search`.

**Gaps.** None. bzr is strictly more capable. The hardcoded email-domain
filter in `bzl-group --list` is bzl-internal.

**Status.** ✅ covered.

## 4. Per-script appendix

This section confirms every script in `reference/bzl/` was read in full and
records where it landed in the workflow review. Line citations are to the
bzl source.

- **`bzl-attachment-add`** (`reference/bzl/bzl-attachment-add:1-114`).
  Multi-bug upload, `--comment`, `--is-patch`, `--is-private`. Mapped to
  §3.4. Two filed gaps (J, K).
- **`bzl-attachment-get`** (`reference/bzl/bzl-attachment-get:1-96`). Bulk
  download to per-bug directory. Mapped to §3.4. One filed gap (L).
- **`bzl-attachment-list`** (`reference/bzl/bzl-attachment-list:1-103`).
  Bulk list, printf-style `--format`. Mapped to §3.4. Single-bug-only
  caveat noted; not filed.
- **`bzl-backlog`** (`reference/bzl/bzl-backlog:1-274`). Bzl-internal
  whiteboard `backlog±N` convention. §3.5. 🔒 not filed.
- **`bzl-backlog-update`** (`reference/bzl/bzl-backlog-update:1-34`).
  Bzl-internal whiteboard mutation. §3.5. 🔒 not filed.
- **`bzl-clone`** (`reference/bzl/bzl-clone:1-88`). Requires a bzl-specific
  custom field. §3.3. ✅ covered (bzr `bug clone` is strictly more capable).
- **`bzl-clones`** (`reference/bzl/bzl-clones:1-179`). Recursive walk over
  a bzl-specific custom field. §3.3. 🔒 not filed.
- **`bzl-comments`** (`reference/bzl/bzl-comments:1-89`). Word-wrapping
  comment listing with `--new-since` date filter. §3.2. `--nowrap` caveat
  pending verification; not filed.
- **`bzl-config`** (`reference/bzl/bzl-config:1-62`). Hardcoded env switch /
  cfg backup. §3.1. ✅ covered.
- **`bzl-create`** (`reference/bzl/bzl-create:1-238`). `$EDITOR` fallback,
  `--description-file`. §3.3. Two filed gaps (D, E).
- **`bzl-get`** (`reference/bzl/bzl-get:1-107`). Multi-bug `--permissive`
  view. §3.2. One filed gap (A).
- **`bzl-group`** (`reference/bzl/bzl-group:1-92`). Group membership.
  §3.6. ✅ covered.
- **`bzl-log`** (`reference/bzl/bzl-log:1-51`). Multi-bug history. §3.2.
  ⚠️ (multi-bug); not filed.
- **`bzl-login`** (`reference/bzl/bzl-login:1-100`). Cookie auth. §3.1.
  🔒 not filed.
- **`bzl-product-list`** (`reference/bzl/bzl-product-list:1-79`). Product
  enumeration. §3.2. ✅ covered.
- **`bzl-search`** (`reference/bzl/bzl-search:1-325`). Structured filters +
  `--from-url`. §3.2. Two filed gaps (B, C).
- **`bzl-update`** (`reference/bzl/bzl-update:1-327`). Field mutation,
  list mutation, comment-with-update. §3.3. Four filed gaps (F, G, H, I).
- **`contrib/bzl-close`** (`reference/bzl/contrib/bzl-close:1-13`).
  Bzl-internal status chain. §3.3. 🔒 not filed.
- **`bzlrpc.py`** (`reference/bzl/bzlrpc.py:1-477`). XML-RPC helper library:
  cookie persistence, format primitives, server allowlist. Not a script —
  reviewed for context (§3.1, server pinning) and to confirm bzl exclusively
  uses XML-RPC.

## 5. Issues to file

Every gap below is filed under the new label `bzl-parity` plus the
appropriate kind label (`enhancement` or `bug`). Priorities are advisory —
labels in the repo are limited, so priority is recorded in the issue body.

For each issue, the **Test plan** describes the new functional-test case
that should ship with the implementing PR. Tests slot into
`tests/functional/run-tests.sh` at the indicated phase (per `tests/functional/README.md`).

### Issue A — `bug view`: accept multiple bug IDs and `--permissive` flag

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** medium
- **Body:**
  > `bzl-get` (`reference/bzl/bzl-get:91`) accepts multiple bug IDs in a
  > single call and offers `--permissive` to return partial results when
  > some bugs are inaccessible. `bzr bug view` accepts a single ID, forcing
  > testers to script multiple invocations and handle errors per call.
  >
  > Proposed: extend `bzr bug view` to accept `<IDS>...` (variadic) and add
  > `--permissive` (continue on per-bug errors, surfacing them as inline
  > rows instead of a non-zero exit). Single-ID behavior is unchanged.
- **Test plan (Phase 8 — Bugs):**
  - Setup: at least three bugs available (some private to a different user)
  - Action: `bzr bug view <id1> <id2> <inaccessible> --permissive`
  - Assert: exit 0, output table contains `<id1>` and `<id2>`,
    `<inaccessible>` row marked with an error string

### Issue B — `bug list`: add `--creation-time` / `--last-change-time` filters

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** high
- **Body:**
  > Common tester workflow: "find bugs filed/modified since DATE." `bzl-search`
  > exposes both filters (`reference/bzl/bzl-search:36-52`); `bzr bug list`
  > has neither. Without these, testers fall back to ad-hoc URL parsing or
  > full-result-set client-side filtering.
  >
  > Proposed: add `--created-since <ISO 8601>` and `--changed-since <ISO 8601>`
  > to `bzr bug list` (and `bzr query save` for parity). Single-value
  > filters; ISO-8601-or-bare-date input; reject non-parseable values with
  > exit code 7.
- **Test plan (Phase 8):**
  - Setup: create two bugs, modify one, capture timestamps
  - Action: `bzr bug list --product <P> --changed-since <timestamp> --json`
  - Assert: only the modified bug appears

### Issue C — `bug list`: add missing field filters (umbrella)

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** medium
- **Body:**
  > `bzl-search` supports filters that `bzr bug list` does not:
  > `--whiteboard`, `--target-milestone`, `--version`, `--op-sys`,
  > `--platform`, `--resolution`, `--qa-contact`, `--url`
  > (`reference/bzl/bzl-search:111-169`). Each is a real workflow filter
  > on common Bugzilla installations.
  >
  > Proposed: add all eight to `bzr bug list` and `bzr query save`, with
  > the same repeatability + `!`-prefix semantics as existing filters.
  > Implementer may split into per-flag PRs if convenient.
- **Test plan (Phase 8):**
  - Action: `bzr bug list --whiteboard <substring> --json`
  - Assert: only bugs whose whiteboard contains the substring appear

### Issue D — `bug create`: open `$EDITOR` when `--summary` and `--description` are absent

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** high
- **Body:**
  > `bzl-create` (`reference/bzl/bzl-create:163-227`) opens `$EDITOR` with a
  > templated buffer (summary line + commented field reminders + description
  > area) when no description is given, mirroring `git commit`'s flow.
  > `bzr comment add` already does this for comment bodies, but `bzr bug
  > create` requires `--summary` and a description source on the command
  > line. Tester ergonomic regression.
  >
  > Proposed: when `--description` and stdin are both absent, `bzr bug
  > create` opens `$EDITOR` (or `vi` fallback) with a template that
  > pre-fills the summary line from `--summary` (if supplied) and provides
  > a commented-out reminder of the resolved field values. The first
  > `\n\n`-delimited block becomes the summary (overrides `--summary` if
  > the user edited it); the rest becomes the description. An empty buffer
  > aborts with exit code 7. `--summary` becomes optional whenever the
  > editor flow is active; it remains required when both `--description`
  > and the editor flow are bypassed.
- **Test plan (Phase 8):**
  - Setup: write a deterministic fake-editor script that overwrites the
    file path it receives as `$1`. Portable across BSD/GNU sed and not
    sensitive to template content:
    ```sh
    cat > "$TMPDIR/fake-editor.sh" <<'SH'
    #!/bin/sh
    printf 'Test summary\n\nTest description\n' > "$1"
    SH
    chmod +x "$TMPDIR/fake-editor.sh"
    ```
  - Action: `EDITOR="$TMPDIR/fake-editor.sh" bzr bug create --product <P>
    --component <C> --version <V> --json`
    (note: no `--summary` — exercises the optional-summary editor path)
  - Assert: exit 0; JSON `summary == "Test summary"`; follow-up
    `bzr bug view $(jq -r .id) --json` shows description starts with
    `Test description`
  - Negative: a fake-editor script that leaves `$1` empty must cause `bzr
    bug create` to exit 7 (input validation, "empty buffer aborts" per
    the proposed semantics)

### Issue E — `bug create`: add `--description-file FILE`

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** medium
- **Body:**
  > `bzl-create --description-file FILE` (`reference/bzl/bzl-create:142-158`)
  > reads the description from a file. Useful for scripted bug-creation
  > pipelines that compose long-form descriptions outside the terminal.
  >
  > Proposed: add `--description-file <PATH>` to `bzr bug create`. Mutually
  > exclusive with `--description`. Reads UTF-8; rejects non-existent paths
  > with exit code 7.
- **Test plan (Phase 8):**
  - Setup: write description to `/tmp/desc.txt`
  - Action: `bzr bug create --product <P> --component <C> --summary 'X'
    --description-file /tmp/desc.txt --json`
  - Assert: created bug's description matches file contents

### Issue F — `bug update`: post a comment atomically with field changes

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** high
- **Body:**
  > `bzl-update --comment` (`reference/bzl/bzl-update:44`) posts a comment
  > as part of the update API call, including support for private comments
  > (`reference/bzl/bzl-update:248,291-298`) and reading from a file
  > (`reference/bzl/bzl-update:249,299-309`). The Bugzilla `Bug.update` API
  > supports this directly (single round-trip).
  >
  > `bzr bug update` requires a separate `bzr comment add` call for the
  > same workflow, doubling the API requests and breaking atomicity for
  > status-change-with-explanation, the most common tester workflow.
  >
  > Proposed: add `--comment <BODY>`, `--comment-file <PATH>`, and
  > `--comment-private` to `bzr bug update`. Folded into the same
  > `Bug.update` request payload.
- **Test plan (Phase 9 — Comments):**
  - Action: `bzr bug update <id> --status RESOLVED --resolution FIXED
    --comment "see #other"`
  - Assert: bug status updated AND comment listed by `bzr comment list <id>`,
    with creation timestamp matching the update

### Issue G — `bug update`: add `--dupe-of` to mark duplicate

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** high
- **Body:**
  > Marking a bug as duplicate is a routine triage operation. `bzl-update
  > --dupe-of <id>` (`reference/bzl/bzl-update:61-68`) sets the
  > `dupe_of` field; the API automatically sets `RESOLVED`/`DUPLICATE`.
  >
  > `bzr bug update` exposes neither a `--dupe-of` flag nor a way to set
  > the `dupe_of` field at all. Workaround today is the Bugzilla web UI.
  >
  > Proposed: add `--dupe-of <ID>` to `bzr bug update`. Forwards to the
  > REST `dupe_of` parameter; bzr does not need to set status/resolution
  > explicitly (Bugzilla handles that).
- **Test plan (Phase 8):**
  - Setup: create bugs A and B
  - Action: `bzr bug update <A> --dupe-of <B>`
  - Assert: `bzr bug view <A> --json` shows `status=RESOLVED`,
    `resolution=DUPLICATE`, `dupe_of=<B>`

### Issue H — `bug update`: list-mutation flags for keywords / cc / groups / see-also (umbrella)

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** medium
- **Body:**
  > `bzl-update` supports `+`/`-`/bare-set syntax for `keywords`, `cc`,
  > `groups`, `see_also` (`reference/bzl/bzl-update:74-78,279-290`). `bzr
  > bug update` supports add/remove only for `blocks` and `depends_on`.
  >
  > Proposed: add the same `*-add` / `*-remove` flag pairs to `bzr bug
  > update` for the four list-typed fields:
  >   - `--keywords-add` / `--keywords-remove`
  >   - `--cc-add` / `--cc-remove`
  >   - `--groups-add` / `--groups-remove`
  >   - `--see-also-add` / `--see-also-remove`
  > Comma-separated values, matching the existing `--blocks-add`/etc.
  > convention. Implementer may split per field if convenient.
- **Test plan (Phase 8):**
  - Action: `bzr bug update <id> --keywords-add fix-needed --cc-add
    test@example.com`
  - Assert: subsequent `bzr bug view --json` shows the new keyword and CC

### Issue I — `bug update`: add miscellaneous field flags (umbrella)

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** medium
- **Body:**
  > `bzl-update` exposes seven scalar flags that `bzr bug update` does not:
  > `--alias`, `--deadline`, `--estimated-time`, `--remaining-time`,
  > `--work-time`, `--reset-assigned-to`, `--reset-qa-contact`
  > (`reference/bzl/bzl-update:16-22,56-60,69-73,124-144`). Each maps to a
  > documented Bugzilla `Bug.update` field.
  >
  > Proposed: add the seven flags to `bzr bug update` with one-to-one
  > mapping. The two reset flags accept no value (presence-only).
- **Test plan (Phase 8):**
  - Action: `bzr bug update <id> --deadline 2026-12-31`
  - Assert: `bzr bug view --json` shows `deadline=2026-12-31`

### Issue J — `attachment upload`: post a comment with the attachment

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** medium
- **Body:**
  > `bzl-attachment-add --comment` (`reference/bzl/bzl-attachment-add:27-29`)
  > posts a comment alongside the attachment in a single API call (Bugzilla
  > `Bug.add_attachment` supports this directly). Tester workflow: posting
  > a patch with explanatory commentary.
  >
  > Proposed: add `--comment <BODY>` to `bzr attachment upload`, folded
  > into the same `add_attachment` request payload. Optional `--comment-private`
  > to mark it private.
- **Test plan (Phase 10 — Attachments):**
  - Action: `bzr attachment upload <bug> /tmp/file.txt --comment "see this"`
  - Assert: `bzr attachment list <bug>` shows the new attachment AND
    `bzr comment list <bug>` shows the new comment, both with matching
    creation timestamps

### Issue K — `attachment upload`: add `--is-patch` flag at upload time

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** low
- **Body:**
  > `bzl-attachment-add --is-patch` (`reference/bzl/bzl-attachment-add:30-34`)
  > marks the attachment as a patch at upload time, which also defaults
  > `content_type` to `text/plain`. `bzr attachment upload` only exposes
  > the patch flag via a follow-up `bzr attachment update --is-patch true`,
  > requiring two API calls.
  >
  > Proposed: add `--is-patch` (boolean flag, presence-only) to `bzr
  > attachment upload`.
- **Test plan (Phase 10):**
  - Action: `bzr attachment upload <bug> /tmp/p.diff --is-patch`
  - Assert: `bzr attachment list <bug> --json` shows `is_patch=true` for the
    new entry

### Issue L — `attachment download`: bulk download by bug or by attachment-id list

- **Labels:** `enhancement`, `bzl-parity`
- **Priority:** medium
- **Body:**
  > `bzl-attachment-get` (`reference/bzl/bzl-attachment-get:51-91`) accepts
  > a mix of bug IDs and attachment IDs and saves every matched attachment,
  > organized into per-bug subdirectories. `bzr attachment download` takes
  > a single attachment ID. Tester workflow: "save all attachments for
  > bug 12345 to a directory" requires `bzr attachment list --json | jq |
  > xargs` plumbing today.
  >
  > Proposed: extend `bzr attachment download` to accept either:
  >   - a single attachment ID (existing behavior), or
  >   - one or more bug IDs via `--bug <ID>` (repeatable), in which case
  >     all attachments for each bug are downloaded into `--out-dir <DIR>`
  >     (default: `./attachments/<bug-id>/`).
  > Existing `--out` flag remains for the single-attachment case; mutually
  > exclusive with `--bug`.
- **Test plan (Phase 10):**
  - Setup: bug with two attachments
  - Action: `bzr attachment download --bug <id> --out-dir /tmp/att`
  - Assert: `/tmp/att/<id>/` contains both files with original names

## 6. Issue-filing process

After this spec is approved:

1. Confirm the `bzl-parity` label exists in the repo; create it via
   `gh label create bzl-parity --description "Tracking gaps surfaced by
   the bzl → bzr workflow-parity review (2026-05-06)"` if not.
2. File issues A–L using `gh issue create`, each with body matching the
   text above and labels `enhancement` (or `bug` where appropriate) +
   `bzl-parity`.
3. Link this spec from each filed issue body so reviewers can see the
   broader context (`docs/superpowers/specs/2026-05-06-bzl-parity-review-design.md`).
4. Tests are *not* added in this stream. Each test plan ships with the PR
   that closes its issue.

## 7. Out of scope

The following are deliberately not addressed by this review:

- Behavioral parity with bzl's printf-style `--format` output (bzr uses
  `--output table|json` instead).
- Bzl-specific custom fields (referenced by name in `reference/bzl/`).
- Bzl-specific custom statuses and resolutions (referenced by name in
  `reference/bzl/`).
- Cookie-based auth and bzl's legacy `.rc`-style config file format.
- Multi-process / concurrent attachment downloads (`bzl-clones` uses a
  process pool; bzr's REST client is async-tokio and does not need it).

If a future tester surfaces a gap rooted in any of the above, it gets
re-evaluated under the same workflow-parity bar.
