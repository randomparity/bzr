# ADR 0023: Weekly-status snapshots are skill-owned immutable records

## Status

Accepted

## Context

Issue #569 requires an installable agent skill to compare current Bugzilla project state with the
most recent compatible prior observation. The core `bzr` CLI exposes the required query and history
data but deliberately does not own a reporting cadence or snapshot store. A failed report must not
destroy the last usable baseline, and snapshots may contain untrusted Bugzilla text.

## Decision

The weekly-status skill owns a versioned run directory outside `bzr` configuration. Each completed
run is an immutable directory containing its snapshot and every requested report. A per-report
`latest` symlink is replaced atomically only after a fully staged run directory validates and is
renamed into place. The skill
compares only snapshots whose format version, server identity, effective-scope fingerprint, and
required field set are compatible; it rejects all other prior snapshots with an explicit diagnostic.

The snapshot stores provenance, effective rules, stable bug IDs, and only the selected state needed
for comparison. It never stores credentials, comments, attachment bodies, or unrelated config.
Bugzilla-controlled strings remain data: spreadsheet cells that could be formulas are neutralized,
HTML is escaped, and generated links accept only validated `http` or `https` URLs.

## Consequences

- Snapshot history survives failed report generation and supports auditing a comparison.
- Compatibility is deterministic rather than inferred from age or filenames.
- The skill, not the CLI configuration model, owns retention and storage-policy guidance.
- Consumers must use the shipped publisher's atomic same-directory replacement for `latest`; direct overwrite is
  outside the supported workflow.
- Scope removal remains an observed membership change until current Bugzilla evidence proves a
  resolution.

## Considered & rejected

- **Store the baseline in `bzr` configuration.** verified: issue #569 explicitly makes this a
  non-goal and identifies snapshots as generated skill-owned artifacts.
- **Overwrite a single snapshot in place.** judgment: it cannot preserve the prior baseline across
  a partial write or failed report.
- **Compare any newest file regardless of provenance.** judgment: a concise result is not worth
  silently comparing different servers, scopes, formats, or insufficient field sets.
- **Require a fixed weekly interval.** verified: issue #569 requires comparison with the latest
  compatible snapshot regardless of elapsed time.
