# 0002 — `bug close` / `bug reopen` target stock-workflow statuses, validated

- Status: Accepted
- Date: 2026-06-20
- Issue: [#349](https://github.com/randomparity/bzr/issues/349)

## Context

The convenience verbs `bug close` and `bug reopen` hardcoded the target
statuses `CLOSED` and `REOPENED`. Neither is part of the **default Bugzilla
5.x workflow**, whose status set is `UNCONFIRMED, CONFIRMED, IN_PROGRESS,
RESOLVED, VERIFIED`. `CLOSED` and `REOPENED` are optional statuses some
installations add; `REOPENED` was dropped from the default workflow entirely.

Against any stock install both verbs therefore failed with Bugzilla API
error 51 (`There is no status named 'CLOSED'.` / `'REOPENED'.`), exit code 4.
The sibling verbs `resolve` (targets `RESOLVED`) and `dup` (uses `dupe_of`)
work because their targets are in the default workflow.

The failure surfaced only against real containers; `verbs_tests.rs` mocked a
server that accepted `CLOSED`, so the unit tests passed (see #350 / #349).

## Decision

1. **Change the defaults to stock-workflow statuses.**
   - `reopen` targets `CONFIRMED` (a default open status; Bugzilla clears the
     resolution automatically when moving to an open status).
   - `close` targets `VERIFIED` (the default terminal status). `close` already
     preserves any existing resolution and is intended for already-resolved
     bugs, so `VERIFIED` is the natural stock target.

2. **Add a `--status <STATUS>` override** to both verbs so installations that
   *do* define `CLOSED` / `REOPENED` (or any other custom status) can still
   reach them: `bzr bug close <id> --status CLOSED`.

3. **Validate the target status against the server before writing.** Before
   the `Bug.update` call, fetch the legal `status` field values
   (`get_field_values("status")`) and confirm the chosen status is among them.
   The match is **exact and case-sensitive** against the names the server
   returns (Bugzilla statuses are uppercase, e.g. `CONFIRMED`), and the value
   that passed validation is sent verbatim — so a wrong-case override such as
   `--status confirmed` is rejected up front rather than passing validation and
   failing server-side. On mismatch, fail with `InputValidation` (exit 7) and a
   message naming the invalid status and listing the server's valid statuses —
   instead of letting the request reach the server and surface as the opaque
   API error 51 (exit 4).

   Validation proves the status *exists*; it does not prove the *transition*
   from the bug's current status to the target is legal in the server's
   workflow. An existing-but-illegal transition (e.g. a workflow that forbids
   `RESOLVED → CONFIRMED`) still surfaces from the server. The validation
   specifically replaces the nonexistent-status case (the error 51 in #349),
   not transition-legality enforcement, which is intentionally left to the
   server.

   Both the write (`update_bug` → `put_json`) and this validation
   (`get_field_values` → `get_json`) use the REST path, so the verbs already
   require REST to write at all; the pre-validation adds no new API-mode
   dependency.

   Validation runs only on the real-write path. Under `--dry-run` it is
   skipped: dry-run is a local preview that performs no mutation, and the
   preview already shows the exact status that would be sent.

## Consequences

- On a stock install `bzr bug close <id>` and `bzr bug reopen <id>` now succeed
  with no flags. This is a **behavior change** to the status these verbs send
  (`CLOSED`→`VERIFIED`, `REOPENED`→`CONFIRMED`), reflected in the verb doc
  comments, `docs/bzr-cli.md`, and `CHANGELOG.md`.
- Installations relying on the old `CLOSED` / `REOPENED` targets must now pass
  `--status CLOSED` / `--status REOPENED` explicitly. This is surfaced in the
  changelog as a behavior change.
- A wrong `--status` value is caught client-side with an actionable error and a
  distinct exit code (7, input validation) rather than the server's generic
  error 51 (4, API). The trade-off is one extra `GET field/bug/bug_status`
  round-trip per `close` / `reopen` invocation on the real-write path.
- `resolve` and `dup` are unchanged: their targets are always in the default
  workflow, so they need neither an override nor pre-validation.

## Considered & rejected

- **Change defaults only, no override, no validation.** Simplest, but
  installations that use `CLOSED` / `REOPENED` lose any way to reach those
  statuses through the verbs, and a typo'd status still surfaces as the opaque
  server error 51.
- **Add `--status` but keep the `CLOSED` / `REOPENED` defaults.** Backward
  compatible, but leaves the verbs broken-by-default on every stock install —
  the exact bug being fixed.
- **Per-server config key for the close/reopen target status.** More machinery
  than the problem needs; an explicit `--status` flag plus stock defaults
  covers the same ground without a new persisted config surface.
- **Validate inside `--dry-run` too.** Rejected to keep dry-run a pure local
  preview with no extra network reads; the preview already shows the status.
