# ADR 0031: Compose dependency presentation artifacts through active capabilities

## Status

Accepted

## Context

The dependency-analysis skill produces deterministic Markdown and Mermaid source. Issue #612
requires a presentation-ready, self-contained report when a safe HTML-capable artifact tool is
available, while preserving Markdown fallback and the existing evidence, safety, and no-schedule
rules. The project-manager reporting skill already owns a capability-gated artifact-safety
contract. Viable designs differ on whether HTML belongs in the Rust CLI, the Python renderer, a
second report engine, or active artifact composition.

## Decision

Keep dependency collection, analysis, and the `bzr-dependency-analysis/v1` schema unchanged.
Dependency analysis owns a report-template reference that maps that schema to stakeholder sections
and graph semantics. It reuses the sibling project-manager artifact-safety reference for escaping,
validated bug links, and self-containment. Before HTML composition, the skill must resolve and read
the expected sibling reference; an absent, unreadable, or incompatible reference makes safe HTML
unavailable. The active HTML-capable artifact tool owns page creation and must open or render the
exact page for visual verification. When that capability or safety prerequisite is absent or
cannot verify the result, the skill renders Markdown and states the limitation.

Ship a deterministic analysis/HTML fixture pair and contract validator for hostile text, unknown
boundaries, truncation, provenance, graph direction, and forbidden schedule claims. The fixture is
proof of the composition contract, not a runtime renderer or a promise that HTML is always
available.

## Consequences

- HTML remains capability-dependent and does not expand the `bzr` CLI or version 1 renderer
  contract.
- Dependency-specific semantics live beside dependency analysis, while shared artifact safety has
  one owner.
- The canonical payload contains both skills, but a partial or version-skewed installation fails
  closed to Markdown rather than improvising artifact-safety rules.
- A safe page requires both automated contract checks and visual wide/narrow inspection; successful
  file creation alone is insufficient.
- Markdown remains the deterministic portable output and fallback.
- The example HTML fixture adds maintained presentation markup, but no runtime dependency, remote
  asset, JavaScript, or second rendering implementation.

## Considered & rejected

- **Add HTML to the Rust CLI.** verified: `src/cli/skills.rs` installs every bundled skill and
  `content/skills/bzr-dependency-analysis/scripts/render.py` currently owns only Markdown and
  Mermaid output at commit `74ac47660bb152014b8acf90f526f2ebd9cc9d80`; a CLI HTML mode would
  duplicate agent artifact capability and violate issue #612's proposed boundary.
- **Add HTML to `render.py`.** judgment: a fixed renderer would turn a capability-dependent
  presentation workflow into a new versioned output contract and duplicate the existing artifact
  capability's layout and verification responsibilities.
- **Copy the project-manager safety rules into dependency analysis.** judgment: duplicated security
  guidance can drift; the canonical payload already includes the sibling skill and reference, and
  the HTML route fails closed when the installed reference is unavailable or incompatible.
- **Put the dependency template and validator in project-manager reporting.** judgment: artifact
  capability selection and generic sink safety belong to project-manager reporting, while the
  meaning of components, edge direction, cycles, boundary nodes, bottlenecks, and no-schedule
  language is the dependency-analysis domain contract. Keeping those semantics beside the analyzer
  avoids making a general PM skill own a schema-specific presentation that changes with dependency
  analysis; the explicit fail-closed prerequisite bounds the cross-skill safety dependency.
- **Use raw Mermaid as the presentation artifact.** verified: issue #612 records that a production
  exercise produced correct graph evidence but still required a separate human-facing
  visualization, so raw source does not close the reported gap.
- **Generate HTML whenever a file writer exists.** judgment: writing bytes does not establish
  escaping, self-containment, or usable layout; capability detection and open/render verification
  are part of the requested outcome.
- **Do nothing and retain Markdown only.** verified: issue #612 explicitly tracks the post-delivery
  presentation gap left by the version 1 Markdown/Mermaid contract.
