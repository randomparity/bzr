# Release-readiness contract cases

These deterministic cases name the skill decision and report section protected
by each fixture. They are contract examples, not evidence that CI executes an
agent.

| Case | Skill section | Rule protected |
| --- | --- | --- |
| RR-HAPPY | Assess without inventing policy | Open P1 and stale work cite IDs, rules, source commands, and timestamp. |
| RR-ROLLUP | Assess without inventing policy | A known blocker wins over unknown; otherwise an unknown blocking check is indeterminate. |
| RR-EMPTY | Collect one bounded rolling snapshot | Zero rows is no visible evidence, never an unqualified ready claim. |
| RR-COMPLETE | Assess without inventing policy | Only non-complete bugs contribute to blocker, stale, and ownership checks. |
| RR-BLOCKER-TYPES | Start with scope and policy | Scalar, list, flag-tuple, and custom-field operator grammar remains exact. |
| RR-AMBIGUOUS | Start with scope and policy | Missing complete states or stale duration prompts for policy or records assumptions. |
| RR-INJECTION | Write a safe artifact | Hostile summaries/whiteboards remain inert code-span text in the Markdown golden. |
| RR-STALE-SOURCE | Supplement only requested evidence | History uses a strict post-baseline event rule and does not overclaim timestamp ordering. |
| RR-RESTRICTED | Collect one bounded rolling snapshot | Failed follow-up reads stay unknown in the denominator and visibility remains limited. |
| RR-BOUNDED | Supplement only requested evidence | Default cap skips root 101 and reports it as a limitation. |
| RR-PAGING | Collect one bounded rolling snapshot | URL/query limits are overridden to 100; offsets make a scope partial; divergent duplicates conflict. |
| RR-DIRECTION | Supplement only requested evidence | Only outgoing `depends_on` links are prerequisites; unreadable targets are unknown. |
| RR-NO-ARTIFACT | Write a safe artifact | Markdown remains available when HTML/document capability is absent. |
| RR-READ-ONLY | Collect one bounded rolling snapshot | Every command belongs to the read-only allowlist. |
| RR-SECRET-URL | Start with scope and policy | Credential aliases, duplicate aliases, encoded names, malformed encoding, and userinfo stop before execution. |

The committed fixture includes one hostile `RR-INJECTION` report. Its expected
Markdown is compared byte-for-byte by `tests/run.sh`; the structured source
records operator input, command trace, and visible data separately.
