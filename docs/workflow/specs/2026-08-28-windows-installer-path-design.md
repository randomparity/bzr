# Windows installer PATH prompt design

## Scope

Issue [#566](https://github.com/randomparity/bzr/issues/566) reports that the Windows one-liner was
run in Command Prompt and that a successful PowerShell installation left `bzr` unavailable by
name. The operator chose a Y/N prompt and later directed the implementation to follow the small,
established pattern used by mature PowerShell installers.

The change is limited to `install.ps1`, its Windows smoke test, the README installation text, and
the release workflow invocation that must remain noninteractive. Generalized PATH handling and
hypothetical custom-path edge cases are explicitly excluded.

## Behavior

After installing `bzr.exe`, the Windows installer checks the semicolon-delimited user and current
process PATH values case-insensitively. If the install directory is already present in both, it
does nothing. Otherwise it prompts `Add bzr to your PATH? [Y/N]`.

On `Y`, it appends the directory to whichever of the user PATH and `$env:Path` does not already
contain it. The user value is persisted with `[Environment]::SetEnvironmentVariable` and the
process value makes `bzr` available immediately in the PowerShell session evaluating the script.
On any other response, neither value changes and the existing manual guidance is printed.

The README labels the one-liner as PowerShell-only and describes the prompt. The release workflow
supplies a deterministic negative response to its installer smoke invocation.

## Verification

Windows smoke coverage proves affirmative update, negative no-op, and already-present no-duplicate
behavior. Existing checksum and unsupported-platform cases remain green. PowerShell static analysis,
repository lint, unit/integration tests, and the Windows CI installer arm remain required.

## Constraints

- Preserve Windows PowerShell 5.1 compatibility.
- Add no dependency or new documented configuration.
- Modify only the current user's PATH, never the machine PATH.
