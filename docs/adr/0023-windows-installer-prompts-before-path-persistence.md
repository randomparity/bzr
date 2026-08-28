# ADR 0023: Windows installer prompts before PATH persistence

## Status

Accepted

## Context

The Windows installer places `bzr.exe` under `%LOCALAPPDATA%\Programs\bzr` by default but
does not make that directory discoverable as `bzr` in the shell. It prints a manual persistent
PATH command instead. Users can therefore complete installation successfully and still receive a
command-not-found error. Changing a user's persistent PATH is useful but must remain an explicit
choice.

## Decision

When the install directory is absent from either the user or current-process PATH value, the
Windows installer prompts the user to complete the missing PATH state. A `Y` response appends one
path entry to each scope where it is absent. A `N` response, empty response, or unavailable
interactive input leaves both values unchanged and prints the manual command. When both scopes
already contain the directory, the installer does not prompt or rewrite either value.

PATH membership is determined by case-insensitive comparison of semicolon-delimited entries after
trimming surrounding whitespace and trailing directory separators. The installer never rewrites or
normalizes existing entries.

## Consequences

- A consenting interactive user can invoke `bzr` immediately and in later shells.
- The installer performs no environment mutation without an affirmative response.
- Existing PATH spelling and ordering are preserved, and equivalent entries are not duplicated.
- Automated or noninteractive installation remains possible, but PATH stays unchanged unless an
  affirmative response is available.

## Considered & rejected

- **Continue printing instructions only.** verified: issue #566 records a successful installation
  followed by `bzr` not being found in the same session after the existing guidance was printed.
  judgment: the operator explicitly chose a consent prompt instead of relying on users to notice
  and execute follow-up instructions.
- **Modify PATH unconditionally.** judgment: changing persistent user environment state without
  consent is disproportionate to installing one executable.
- **Modify only the persistent user PATH.** verified: Windows processes inherit an environment
  snapshot at startup, so changing the user value does not update the PowerShell process running
  the installer; the reported immediate `bzr --help` workflow would still fail in that session.
