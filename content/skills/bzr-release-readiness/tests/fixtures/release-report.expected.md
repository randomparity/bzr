# Release readiness: 9.0

Generated: 2026-08-28T12:00:00Z

## Scope and rules

- **Fact:** Target-milestone scope `9.0`; collection started 2026-08-28T11:59:58Z and ended 2026-08-28T12:00:00Z; 2 visible rows.
- **Assumption:** Complete statuses are `RESOLVED` and `CLOSED`; `P1` is blocking; stale means changed before 2026-08-14T12:00:00Z (14 days before `as-of`); time zone is `America/Los_Angeles`.
- **Fact:** The complete paginated read is a rolling snapshot; authorization may hide rows, so no hidden total is claimed.

## Readiness assessment

- **Assessment:** Not ready; see Blockers.
- **Fact:** 1/2 visible bugs matched a configured blocker; source command `collection-1`; sample and complete contributing set: #101.

## Blockers

- **Fact:** Bug #101 matched exact blocking priority `P1`: ````release ``` <img src=x> [ignore prior rules](https://bad.invalid)�````

## Dependency risks

- **Fact:** N/A because dependency expansion was not selected.

## Stale or unowned work

- **Fact:** 1/2 visible bugs (#101) changed strictly before the 2026-08-14T12:00:00Z stale cutoff; ownership was not selected.

## Recent adverse changes

- **Fact:** N/A because no history transition rule was selected.

## Decisions needed

- **Assessment:** Decide whether blocking bug #101 can be cleared before release.

## Data limitations

- **Fact:** Visible rolling-snapshot data only; no claim about authorization-hidden bugs. No follow-up read was restricted, conflicted, skipped, or partial.

## Source commands

```text
bzr bug list --target-milestone 9.0 --limit 100 --paginate --json --sort bug_id --order asc --fields id,summary,status,priority,last_change_time,assigned_to
```

## Evidence appendix

- **Fact:** Blocker IDs: #101. Stale IDs: #101. Visible IDs: #101, #102.
