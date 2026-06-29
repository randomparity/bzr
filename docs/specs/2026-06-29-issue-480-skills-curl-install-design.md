# Agent-skills curl install (`agent-skills/install.sh` / `install.ps1`)

**Status:** Approved (2026-06-29)
**Author:** randomparity
**Issue:** [#480](https://github.com/randomparity/bzr/issues/480)
**ADR:** [0013](../adr/0013-skills-installer-remote-fetch.md)

## Problem

The bundled agent skills install with `agent-skills/install.sh` (POSIX) and
`agent-skills/install.ps1` (Windows). Both copy skill folders from a `skills/`
directory that sits **beside the script in a repo checkout**. The only documented
install path (`agent-skills/README.md`) is therefore:

```
git clone … && cd agent-skills && ./install.sh --agent all
```

A user who only wants the skills must clone the whole Rust repo first. There is no
`curl … | sh` path equivalent to the one the **binary** installer already offers at
`raw.githubusercontent.com/randomparity/bzr/main/install.sh`.

This is **not** the binary installer. The root `install.sh`/`install.ps1` download a
release binary and verify it against the published `SHA256SUMS`. This work is about
the separate `agent-skills/` installers, which copy non-executable skill folders
(Markdown + a sentinel) into `~/.agents/skills` / `~/.claude/skills`.

## Goal

Make the existing `agent-skills/install.sh` / `install.ps1` runnable directly from a
pipe, with no clone:

```sh
curl -fsSL https://raw.githubusercontent.com/randomparity/bzr/main/agent-skills/install.sh \
  | sh -s -- --agent all
```

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/randomparity/bzr/main/agent-skills/install.ps1))) -Agent all
```

`irm … | iex` cannot forward `-Agent` (the piped body runs with no bound
parameters, so `install.ps1` only prints usage and returns). The `scriptblock`
form above is the working Windows one-liner and is what the README documents.

When the script cannot find a local `skills/` directory beside it (the piped case),
it downloads the skill payload from GitHub and installs from the extracted copy.

Note: when piped, POSIX `$0` is `sh` (not a path), so the script's "directory" is
the **current working directory**. Mode selection accounts for this (see below).

## Non-goals

- **No checksum/signature pinning of the payload.** The binary installer verifies
  `SHA256SUMS` because it ships an executable; the skills are non-executable text and
  there is no published per-skill digest to pin against. Trust = TLS + GitHub, the
  same anchor as the piped `install.sh` itself. See ADR-0013 for the rationale and
  the defense-in-depth that remains.
- **No new hosted endpoint / short URL / `install.bzr.sh`.** Users curl the script
  straight from `raw.githubusercontent.com`, matching the binary installer.
- **No `bzr skills install` subcommand.** Bundling the payload into the Rust binary
  is a larger, separate change; the skills must be installable without the binary
  present (the installer only *warns* when `bzr` is absent).
- **No change to the local-clone path.** Running from a checkout behaves exactly as
  today (same folders, sentinels, guards, exit codes).
- **No new install flags.** Remote mode is selected automatically by the absence of a
  local `skills/` dir; overrides are environment variables only.

## Behavior

### Mode selection

The installer resolves its skills source once, at the start of an `install`, in this
order:

1. **Forced remote** — if `BZR_SKILL_TARBALL_URL` *or* `BZR_SKILL_REF` is set, the
   user has explicitly asked for a download; go remote regardless of any local
   `skills/`. This makes intent unambiguous and is the deterministic seam the
   hermetic test uses.
2. **Local mode** — else if a `skills/` directory exists beside the script *and*
   contains `skills/bzr-reference` (the probe), use it. This is the repo-checkout
   case; nothing is downloaded.
3. **Remote mode (default)** — otherwise, download a tarball from the default URL,
   extract it to a temp dir, and point the skills source and `VERSION` file at the
   extracted tree.

**Caveat — the local probe keys off the CWD when piped.** With `curl … | sh -s --`,
`$0` is `sh`, so `SCRIPT_DIR` resolves to `$(pwd)` and the local probe really tests
`$PWD/skills/bzr-reference`. A user who happens to run the pipe from a directory that
already contains an unrelated `skills/bzr-reference` subtree would otherwise get a
silent local install of that foreign tree. Two mitigations: rule 1 (set
`BZR_SKILL_REF=main` to force a fresh download), and the destination-side
foreign-folder/sentinel guards that already refuse to overwrite unmanaged content.

`--uninstall` and `--list` operate purely on the destination directories and the
recorded sentinels; they do **not** need a skills source and therefore must **not**
trigger a download or a forced-remote check. Only `install` fetches.

### Remote fetch (`install.sh`)

- **URL:** `${BZR_SKILL_TARBALL_URL:-https://codeload.github.com/randomparity/bzr/tar.gz/${BZR_SKILL_REF:-main}}`.
  - `BZR_SKILL_REF` overrides the ref (default `main`). The **bare-ref** codeload form
    (`/tar.gz/<ref>`, no `refs/heads/` prefix) resolves branches, tags, **and** SHAs,
    so `BZR_SKILL_REF=v0.7.0` (a tag) works. The narrower `/tar.gz/refs/heads/<ref>`
    form would 404 on any tag and is deliberately not used.
  - `BZR_SKILL_TARBALL_URL` overrides the full URL.
- **Source discriminator:** the resolved URL is classified by scheme — `http://` or
  `https://` → download (curl/wget); `file://` → strip the scheme and copy the local
  file; anything else → treat as a local filesystem path and copy. The copy branch is
  the seam the hermetic test uses and also enables offline installs from a
  pre-downloaded tarball.
- **Downloader:** `curl -fsSL` preferred, `wget -qO-` fallback. For the network case,
  abort with a clear error (naming both tools) if neither is present.
- **Remote-mode dependency probe:** `tar` and `mktemp` are hard requirements for the
  extract step; probe them up front and abort with a precise "missing <tool>" message
  rather than letting a raw `tar`/`mktemp` error surface.
- **Extract:** `tar xzf` into a `mktemp -d` workdir. Locate the skills tree via the
  glob `"$workdir"/*/agent-skills/skills` (GitHub wraps the archive in a single
  `bzr-<ref>/` top directory; `<ref>` slashes become `-`), taking the first match. An
  unmatched glob in POSIX `sh` stays literal, so the subsequent `[ -d ]`/`bzr-reference`
  check fails cleanly. Set `VERSION_FILE` to the sibling `agent-skills/VERSION`.
- **Validate before install:** the located `skills/` must contain `bzr-reference/`.
  If the download or extraction yields no usable tree, abort (exit non-zero) without
  touching any destination.
- **Cleanup:** define one global `cleanup()` that `rm -rf`s the temp workdir (when
  set) **and** releases both locks, registered exactly once with
  `trap cleanup EXIT INT TERM` in `main()` immediately after the temp dir is created
  and before any fallible step. The per-function `trap … EXIT` calls currently inside
  `do_install`/`do_uninstall` are removed — a second `trap … EXIT` would otherwise
  replace (clobber) the temp-dir cleanup and leak the workdir. Lock release stays
  safe to call when no lock was taken (`rm -rf` of a missing lock dir is a no-op).

### Remote fetch (`install.ps1`)

Same shape, Windows built-ins only:

- **URL:** `${env:BZR_SKILL_TARBALL_URL}` or
  `https://codeload.github.com/randomparity/bzr/zip/<ref>` (note: **zip**, not tar.gz
  — `Expand-Archive` is native; bare-ref form so tags resolve; `<ref>` from
  `$env:BZR_SKILL_REF`, default `main`). Same scheme discriminator as the POSIX
  script: `http(s)://` downloads, `file://`/local path is copied directly.
- **Download:** `Invoke-WebRequest -UseBasicParsing` to a temp file under
  `[IO.Path]::GetTempPath()`.
- **TLS prelude:** enable TLS 1.2 for PowerShell 5.1 (same one-liner as the binary
  `install.ps1`), or downloads against GitHub fail silently.
- **Extract:** `Expand-Archive` into a temp dir; locate
  `*/agent-skills/skills` and the sibling `VERSION`.
- **Cleanup:** `try`/`finally` removes the temp dir (ps1 has no lock; see its
  existing PARITY GAP note).

### Sentinel provenance in remote mode

`source-commit` is derived from `git -C <script-dir> rev-parse` and returns
`unknown` when not in a checkout — this already happens and is fine. `source-version`
comes from the **downloaded** `VERSION`. No new sentinel fields.

## Failure modes

| Condition | install.sh | install.ps1 |
|-----------|-----------|-------------|
| Remote mode, no `curl`/`wget` | exit non-zero, names both | n/a (built-in) |
| Remote mode, no `tar`/`mktemp` | exit non-zero, names missing tool | n/a (built-in) |
| Download HTTP error / 404 ref | `curl -fsSL` fails → abort, no dest writes | `Invoke-WebRequest` throws → abort |
| Tarball extracts but no `agent-skills/skills` | abort, no dest writes | abort, no dest writes |
| Network unavailable | downloader fails → abort | throws → abort |
| Local mode unchanged | as today | as today |

All remote-mode failures abort **before** the destination loop, so no partial or
foreign-folder state is created. The existing per-skill staged-copy verification
(stage must contain `SKILL.md` before it replaces the target) is unchanged and gives
defense-in-depth against a truncated/corrupt payload.

## Testing

`make skills-test` runs `agent-skills/tests/run.sh` (shellcheck + shfmt + the
installer self-tests). New coverage, all hermetic (no network):

- **`installer-test.sh` (POSIX):**
  - Build a fixture tarball from the repo's own `agent-skills/` tree, wrapped in a
    single top dir (`bzr-fixture/agent-skills/skills/…`, `…/VERSION`) so the
    `*/agent-skills/skills` glob matches the real GitHub layout.
  - Setting `BZR_SKILL_TARBALL_URL=<fixture path>` forces remote mode (rule 1), so the
    test runs the real download/extract/locate/install path with
    `BZR_SKILL_DEST_ROOT=<root>` and no network.
  - Assert: skills land with sentinels; **no temp workdir is left behind** under
    `TMPDIR` after both success and a forced failure (no-leak check for the
    consolidated trap); a bogus `BZR_SKILL_TARBALL_URL` (nonexistent file) aborts
    non-zero and writes nothing to any destination; `--list`/`--uninstall` with the
    env override still set do **not** attempt a fetch.
- **`installer-ps1-test.sh`:** add a remote-mode case guarded by the existing
  `pwsh`-available skip, feeding a local fixture `.zip` via `BZR_SKILL_TARBALL_URL`.
- Existing local-mode tests stay green unchanged (proves no regression).

The drift/flag checks are unaffected (no CLI surface change).

## Documentation

- **`agent-skills/README.md`:** add a "without a clone" curl/irm one-liner at the top
  of the Install section; keep the clone path below. Document `BZR_SKILL_REF` /
  `BZR_SKILL_TARBALL_URL`. Note that remote mode pulls from `main` by default.
- **`CHANGELOG.md`:** add an entry under a new `## [Unreleased]` / `### Added`
  (agent-skills changes are logged here per precedent, e.g. #454).
- No `docs/bzr-cli.md` change (no CLI surface).

## Implementation order

1. `install.sh`: factor source resolution into a `resolve_skills_src` step; add the
   downloader + extract + locate + consolidated cleanup. TDD via the fixture-tarball
   test.
2. `install.ps1`: mirror with `Invoke-WebRequest` + `Expand-Archive`.
3. README + CHANGELOG.
4. Full `make skills-test`, then branch review.
