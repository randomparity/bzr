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
irm https://raw.githubusercontent.com/randomparity/bzr/main/agent-skills/install.ps1 | iex
# then run the installed install.ps1 with -Agent, or use the documented one-liner
```

When the script cannot find a local `skills/` directory beside it (the piped case),
it downloads the skill payload from GitHub and installs from the extracted copy.

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

The installer resolves its skills source once, before any action
(install/uninstall/list):

1. **Local mode** — if a `skills/` directory exists beside the script *and* contains
   the expected skill folders (`skills/bzr-reference` is the probe), use it. This is
   the repo-checkout case; nothing is downloaded.
2. **Remote mode** — otherwise (piped via curl, `$0` is `sh`/`-`, no sibling
   `skills/`), download a tarball, extract it to a temp dir, and point the skills
   source and `VERSION` file at the extracted tree.

`--uninstall` and `--list` operate purely on the destination directories and the
recorded sentinels; they do **not** need a skills source and therefore must **not**
trigger a download. Only `install` fetches.

### Remote fetch (`install.sh`)

- **URL:** `${BZR_SKILL_TARBALL_URL:-https://codeload.github.com/randomparity/bzr/tar.gz/refs/heads/${BZR_SKILL_REF:-main}}`.
  - `BZR_SKILL_REF` overrides the branch/tag (default `main`).
  - `BZR_SKILL_TARBALL_URL` overrides the full URL. A `file://` URL or a plain local
    path is honored by copying instead of invoking a downloader — this is the seam
    the hermetic test uses and also enables offline installs from a pre-downloaded
    tarball.
- **Downloader:** `curl -fsSL` preferred, `wget -qO-` fallback. Abort with a clear
  error (naming both tools) if neither is present.
- **Extract:** `tar xzf` into a `mktemp -d` workdir. Locate the skills tree via the
  glob `"$workdir"/*/agent-skills/skills` (GitHub wraps the archive in a single
  `bzr-<ref>/` top directory; `<ref>` slashes become `-`). Set `VERSION_FILE` to the
  sibling `agent-skills/VERSION`.
- **Validate before install:** the located `skills/` must contain `bzr-reference/`.
  If the download or extraction yields no usable tree, abort (exit non-zero) without
  touching any destination.
- **Cleanup:** the temp workdir is removed by the same `trap` that releases the
  install locks (a single consolidated cleanup handler), on `EXIT INT TERM`.

### Remote fetch (`install.ps1`)

Same shape, Windows built-ins only:

- **URL:** `${env:BZR_SKILL_TARBALL_URL}` or
  `https://codeload.github.com/randomparity/bzr/zip/refs/heads/<ref>` (note: **zip**,
  not tar.gz — `Expand-Archive` is native; `<ref>` from `$env:BZR_SKILL_REF`, default
  `main`). A local path / `file://` is copied directly.
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
  - Build a fixture tarball from the repo's own `agent-skills/` tree
    (`bzr-fixture/agent-skills/skills/…`, `…/VERSION`).
  - Copy `install.sh` **alone** into an empty temp dir (no sibling `skills/`), so it
    takes the remote path faithfully. Run it with
    `BZR_SKILL_TARBALL_URL=<fixture path>` and `BZR_SKILL_DEST_ROOT=<root>`.
  - Assert: skills land with sentinels; temp workdir is cleaned up; a bogus
    `BZR_SKILL_TARBALL_URL` (nonexistent file) aborts non-zero and writes nothing;
    `--list`/`--uninstall` from the standalone copy do **not** attempt a download.
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
