# Windows installer PATH prompt design

## Scope and authority

Issue [#566](https://github.com/randomparity/bzr/issues/566) reports that the documented Windows
one-liner is attempted in Command Prompt and that a successful PowerShell installation leaves
`bzr` unavailable by name. The operator decided in the active quest to add PATH only after a Y/N
prompt and approved the design that treats empty or unavailable input as refusal.

This change is limited to `README.md`, `install.ps1`, `tests/installer/smoke.ps1`, and direct
release or CI installer-test dependencies if verification requires them. Unix installer behavior,
release artifact contents, and unrelated PATH management are excluded.

ADR [0023](../../adr/0023-windows-installer-prompts-before-path-persistence.md) governs the consent
and PATH-update contract.

## User-visible behavior

The README labels the Windows one-line command as PowerShell, not Command Prompt. It explains that
the installer offers to add its directory to PATH and only does so with consent.

After copying and smoke-checking `bzr.exe`, the installer compares its install directory against
the current user's persistent PATH and the current process PATH. Comparison splits on semicolons,
trims whitespace and trailing `\` or `/`, and ignores case. If both PATH values already contain an
equivalent entry, the installer does not prompt or rewrite either value.

If either PATH lacks the directory, the installer asks `Add bzr to your PATH? [Y/N]`. Only a
trimmed, case-insensitive `Y` or `Yes` is affirmative. On affirmation, it appends the directory to
each missing scope with exactly one separator when needed. On `N`, empty input, any other input, or
a prompt failure, it changes neither value and prints the existing actionable persistent command
plus a note that a new shell is needed after running it.

If persisting PATH fails after an affirmative response, installation remains successful but the
installer reports the failed PATH operation and the manual command. It must not claim PATH was
updated. Updating `$env:Path` occurs only after persistence succeeds, preventing an immediate-only
state that contradicts the user's choice.

## Components and data flow

`install.ps1` owns three small operations: exact PATH-entry membership, safe entry append, and the
prompt/update flow. The existing main path invokes the flow after the executable is installed.
No dependency or new configuration surface is introduced.

`tests/installer/smoke.ps1` exercises affirmative, negative, and already-present behavior. Each
test snapshots the user PATH and restores it in `finally`; it also isolates the process PATH. The
README change is checked by review and the existing installer smoke workflow.

## Error handling and edge cases

- Null or empty PATH values accept the install directory without a leading separator.
- Existing values retain their exact spelling, order, and separators.
- Case, whitespace, and trailing separators do not cause duplicate entries.
- Refusal and unavailable input are non-errors and leave environment state unchanged.
- Persistence failure is reported with the attempted operation, install directory, and manual fix.
- Re-running the installer after consent does not prompt or append a duplicate entry.

## Threat model

### Boundary inventory

The design widens one existing boundary: the locally controlled `BZR_INSTALL_DIR` value can be
persisted into the current user's PATH. It adds one control boundary: local console input decides
whether that write occurs. No network, privilege, or cross-user boundary is added.

### Actor model

The local operator and their environment control the install directory and prompt response. The
installer trusts that operator to choose a suitable directory. A remote release server remains
untrusted for archive bytes and is already controlled by the existing checksum verification.

### Controls

- Persistent mutation requires an explicit affirmative console response.
- PATH membership is exact by entry rather than substring, preventing a similarly named directory
  from suppressing the prompt or causing an accidental duplicate.
- The write targets only the current user's environment through .NET's `User` scope; it does not
  request elevation or write the machine PATH.
- Error output names the operation and recovery command without exposing credentials.
- Tests restore the user PATH even when assertions fail.

### Out of scope

The design does not validate whether a custom install directory is trustworthy or writable; the
operator supplied it and the existing copy step already relies on it. It does not protect against a
malicious local process modifying the same user's PATH concurrently. It does not change archive
download or checksum controls.

## Verification

Windows smoke tests must prove affirmative persistence and current-process availability, refusal
without mutation, exact duplicate detection, empty PATH handling, and cleanup restoration. Run
PowerShell static analysis and the repository lint and test guardrails. Because this is a
user-facing installer change, the real Windows installer smoke arm in CI is required; the local
macOS host cannot substitute for that target.

## Global constraints

- Preserve Windows PowerShell 5.1 compatibility.
- Support `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` installer targets.
- Add no dependency.
- Do not change PATH without an explicit affirmative response.
- Do not duplicate or normalize existing PATH entries.
