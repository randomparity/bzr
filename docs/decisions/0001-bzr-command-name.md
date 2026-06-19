# 0001 — Keep `bzr` as the command name despite the GNU Bazaar collision

- Status: Accepted
- Date: 2026-06-19
- Issue: [#322](https://github.com/randomparity/bzr/issues/322)

## Context

`bzr` was historically the command name for **GNU Bazaar**, a distributed
version-control system sponsored by Canonical. Anyone with Bazaar installed,
or with muscle memory from it, hits a literal name clash with this Bugzilla
CLI. The question is whether to (a) keep `bzr` and document the collision,
(b) ship an alternate alias such as `bugzilla` alongside `bzr`, or
(c) rename the primary command.

## Decision

Keep `bzr` as the primary (and only) command name. Document the collision and
a shell-alias workaround in the README for the rare user who also has GNU
Bazaar installed.

## Rationale

- **The VCS world is vacating the name.** GNU Bazaar's last release was 2.7.0
  in February 2016. Canonical announced Bazaar's retirement in 2025 and is
  sunsetting Bazaar code hosting on Launchpad (December 2025). Bazaar's
  maintained successor, **Breezy, deliberately renamed its command from `bzr`
  to `brz`** to avoid clashes, and distributions have followed: Fedora 32
  (2019) replaced the `bzr` package with Breezy, shipping `/usr/bin/bzr` as a
  symlink to `brz`. In practice the `bzr` command name is being abandoned by
  the VCS ecosystem, so the long-term collision risk is low and shrinking.
- **`bzr` is established for this project.** The crate, binary, repository,
  man pages, agent skills, and every doc and example are built around `bzr`.
  It is short, memorable, and reads naturally as "Bugzilla" the way `gh` reads
  as "GitHub" — the explicit model for this tool. Renaming would churn the
  entire surface for little benefit.
- **A second binary is maintenance burden, not a fix.** Shipping a `bugzilla`
  alias alongside `bzr` doubles the packaged-binary surface (deb/rpm/Homebrew,
  completions, man pages) and does not remove the `bzr` collision it is meant
  to address; users would still type `bzr`. The collision is best handled per
  user, by the user who actually has both tools, via a shell alias.

## Consequences

- No code or packaging change. `bzr` remains the single command name.
- The README gains a short note about the name and a copy-paste shell-alias
  workaround for users who also use GNU Bazaar / Breezy.
- If a future, widely-installed tool reintroduces a `bzr` command, this
  decision can be revisited; nothing here forecloses adding an alias later.
