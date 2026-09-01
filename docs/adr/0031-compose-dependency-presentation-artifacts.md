# ADR 0031: Compose dependency presentation artifacts through active capabilities

## Status

Accepted

## Context

The dependency-analysis skill produces deterministic Markdown and Mermaid source. Issue #612 asks
for a presentation-ready self-contained report when a safe HTML-capable artifact tool is available,
while keeping Markdown fallback, evidence limitations, and no-schedule language. The
project-manager reporting skill already owns capability gating and artifact-safety guidance.

The operator narrowed this change to skill prose, one dependency template, one small deterministic
fixture with proportionate structural checks, installed-copy proof, and visual inspection of one
representative page. General validation, agent-event auditing, formal source-binding, and
transactional candidate promotion are explicitly outside the authorized result.

## Decision

Keep dependency collection, analysis, the version 1 schema, and the Markdown/Mermaid renderer
unchanged. Dependency analysis owns the dependency-specific presentation template and optional
HTML composition workflow. It reuses the sibling project-manager artifact-safety reference rather
than copying shared escaping and self-containment rules. The active artifact capability owns HTML
generation and must open or render the result for visual inspection. Missing capability or
references, or an unsafe/unusable result, routes to Markdown with the limitation stated.

Ship one compact hostile/partial/truncation analysis-and-HTML fixture pair. A focused structural
test verifies that checked-in example and is not a general-purpose runtime validator. Extend the
installed dependency functional phase to prove that the template, fixture, test, and sibling safety
reference exist and work from the installed payload.

## Consequences

- HTML remains capability-dependent and does not expand the `bzr` CLI or renderer contract.
- Dependency presentation semantics stay beside dependency analysis; generic artifact safety keeps
  one owner in project-manager reporting.
- The fixture is a reviewable example and regression oracle, not proof for arbitrary generated HTML.
- Visual inspection remains required because structural checks do not establish readable layout.
- Markdown remains the deterministic portable output and fail-closed fallback.
- The solution intentionally relies on agent instructions and the active capability's workflow
  rather than adding the broader machinery excluded by the operator.

## Considered & rejected

- **Add HTML to the Rust CLI or Python renderer.** verified: at commit
  `74ac47660bb152014b8acf90f526f2ebd9cc9d80`, `render.py` accepts only Markdown and Mermaid; issue
  #612 explicitly asks to reuse an artifact capability rather than add a CLI HTML mode.
- **Put dependency semantics in project-manager reporting.** judgment: capability selection and
  generic sink safety belong there, but edge direction, cycles, boundaries, bottlenecks, and
  no-schedule language change with dependency analysis and belong beside that contract.
- **Copy the artifact-safety rules.** judgment: two normative copies can drift; the bundled
  installation already includes both skills, and missing guidance safely routes to Markdown.
- **Ship a general validator, event-audit harness, or formal source-binding framework.** verified:
  the operator's narrowing decision explicitly excludes that machinery in favor of proportionate
  fixture checks and one visual acceptance run.
- **Use raw Mermaid or keep Markdown only.** verified: issue #612 records that correct raw graph
  evidence still left the user to construct a human-facing visualization manually.
- **Do nothing.** verified: retaining only the version 1 artifacts leaves issue #612's presentation
  gap unresolved.
