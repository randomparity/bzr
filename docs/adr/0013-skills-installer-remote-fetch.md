# 0013 — Agent-skills installer fetches its payload from a GitHub tarball, unverified

**Status:** Accepted

> **Superseded by [0018](0018-embed-canonical-skills-in-binary.md)**
> (2026-08-05)

## Context

The `agent-skills/install.sh` / `install.ps1` scripts copy skill folders from a
`skills/` directory beside the script in a repo checkout. Issue #480 asks for a
`curl … | sh` install that needs no clone. When piped, there is no sibling `skills/`,
so the installer must obtain the payload some other way.

The repo already has a *binary* installer (root `install.sh`) that downloads a
release archive and verifies it against a published `SHA256SUMS`. The obvious
question is whether the skills installer should do the same. It deliberately does
not, and that divergence is worth recording so a future reader does not "fix" it.

Two coupled decisions:

1. **How to obtain the payload remotely.**
2. **Whether to cryptographically verify it.**

## Decision

**1. Fetch a full-repo tarball from GitHub `codeload` and extract the
`agent-skills/skills` subtree.**

- `install.sh` pulls `…/tar.gz/<ref>` (bare-ref codeload form, so branches, tags, and
  SHAs all resolve) via `curl -fsSL` (or `wget` fallback) and `tar xzf`. `install.ps1`
  pulls the `…/zip/<ref>` form and `Expand-Archive` (native, no external tool).
- Default `<ref>` is `main`, overridable via `BZR_SKILL_REF`; the whole URL is
  overridable via `BZR_SKILL_TARBALL_URL` (which also accepts a local path /
  `file://` for offline and test use).
- The script selects remote mode automatically when no local `skills/` dir is found;
  the local-clone path is unchanged.

**2. Do not checksum- or signature-verify the downloaded payload.**

Trust anchor is TLS + GitHub — identical to the trust the user already extends to the
`install.sh` they piped from the same origin. Defense-in-depth that remains:

- The installer never *executes* the payload; it copies Markdown skill folders.
- Each skill is staged and checked for `SKILL.md` before it atomically replaces a
  target; a truncated/corrupt download fails that check per-skill.
- The symlink-destination and foreign-folder (`--force`-gated) guards are unchanged.

## Consequences

- A user installs skills with one piped command, no clone, no new dependency beyond
  `curl`/`wget` + `tar` (POSIX) or PowerShell built-ins (Windows).
- Default install tracks `main`, matching the served script and the existing
  "pull and re-run to update" model. A user wanting a frozen payload pins
  `BZR_SKILL_REF`/`BZR_SKILL_TARBALL_URL`.
- The skills installer's trust model is weaker than the binary installer's
  (no checksum). This is acceptable because the payload is non-executable and there
  is no published digest; it is **intentional**, not an oversight.
- Remote mode downloads the whole repo tarball, not just the skills subtree (GitHub
  offers no subtree archive without the API). The payload is small relative to a full
  `git clone` and is discarded after extraction.

## Considered & rejected

- **Per-file download via `raw.githubusercontent.com`.** Needs a hard-coded file
  manifest that drifts as skills change, multiplies HTTP round-trips, and is more
  exposed to unauthenticated rate limits. The tarball is one atomic fetch.
- **`git clone` fallback.** Reintroduces the dependency (`git`) and the cost the
  issue is trying to remove.
- **Checksum/signature pinning of the payload.** No published per-skill digest exists
  to pin; generating one would be circular (served by the same origin as the script)
  and adds release-pipeline surface for a non-executable text payload. Revisit only
  if releases begin publishing a skills digest.
- **Default to the latest release tag or to `agent-skills/VERSION`.** Adds an API
  call to resolve "latest" and can lag the skills on `main`; pinning to `VERSION` can
  reference a tag not yet released when the script is served from `main`. `main` keeps
  the served script and its payload in lockstep. (See spec, user decision 2026-06-29.)
- **A `bzr skills install` subcommand / bundling the payload into the binary.** Larger
  change; the skills must install without the binary present. Separate ticket if ever
  wanted.
