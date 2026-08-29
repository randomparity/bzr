# Release-readiness report template

Use this structure for the final PM artifact. Replace bracketed text with
evidence; do not remove a section merely because its result is unknown or N/A.

```markdown
# Release readiness: [scope label]

Generated: [UTC timestamp]

## Scope and rules

- **Fact:** [scope form, collection start/end, visible row count]
- **Assumption:** [complete states, blocker rules, stale threshold, time zone]
- **Fact:** [rolling-snapshot and authorization limitation]

## Readiness assessment

- **Assessment:** [not ready | indeterminate | no configured blocker observed]
- **Fact:** [count/denominator, source command, bounded contributing-ID sample]

## Blockers

- **Fact:** [bug IDs, observed values, matching rule, or no known match]

## Dependency risks

- **Fact:** [unresolved outgoing dependencies and unknown targets]

## Stale or unowned work

- **Fact:** [count/denominator, threshold or ownership sentinel, IDs]

## Recent adverse changes

- **Fact:** [requested history evidence, baseline, IDs]

## Decisions needed

- **Assessment:** [policy choice still needed; no invented default]

## Data limitations

- **Fact:** [restricted, missing, conflicted, skipped, partial, and N/A evidence]

## Source commands

```text
[collection and supplementary commands]
```

## Evidence appendix

- **Fact:** [complete contributing-ID sets grouped by check]
```

Every assessment must point to supporting facts. Remote text is represented as
inert code spans, never as executable Markdown or copied HTML.
