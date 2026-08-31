# Dependency presentation reports design

## Goal

Extend the bundled dependency-analysis skill so an agent with a safe HTML-capable artifact
capability can turn one validated `bzr-dependency-analysis/v1` snapshot into a presentation-ready,
self-contained report. The report preserves the evidence and limitations already required by the
Markdown renderer, while Markdown remains the portable fallback. This implements issue #612 and
[ADR 0031](../../adr/0031-compose-dependency-presentation-artifacts.md).

## Architecture

The analyzer and its version 1 JSON contract remain unchanged. Dependency analysis owns the
meaning and layout of a dependency presentation through a new reference template. The existing
project-manager reporting artifact-safety contract owns output escaping, link validation, and
self-containment. An active artifact capability owns HTML construction and its normal open/render
verification workflow; `bzr` and `render.py` do not gain an HTML mode.

The canonical payload includes `bzr-project-manager-reporting`, but installation is not a
transactional guarantee that a compatible sibling reference is readable. The owned safety
reference begins with the exact marker `Artifact safety contract:
bzr-project-manager-reporting/v1`. The dependency skill must resolve the file and verify that
marker before creating HTML. A missing, unreadable, or mismatched reference makes safe HTML
unavailable and routes to Markdown with the limitation stated. A deterministic analysis/HTML
fixture pair exercises the composition contract without shipping a second report renderer.
Contract tests validate the fixture's required sections, inert hostile text, local-only
presentation resources, expected-host bug links, graph semantics, boundary disclosures, and
truncation disclosures; a readable mismatched-marker fixture proves the fail-closed fallback
language.

## Capability routing and data flow

1. Collect and analyze one snapshot through the existing bounded workflow. Presentation never
   triggers a second collection or combines snapshots.
2. If the requested artifact is HTML, inspect the active environment before promising it. A
   document, site, or direct-file capability is usable only when it can create escaped,
   self-contained HTML and open or render the local result for visual inspection.
3. When no such capability exists, explain the missing capability and render the existing Markdown
   report. Mermaid remains an optional source artifact, not the primary presentation fallback.
4. Before HTML composition, resolve the project-manager artifact-safety reference, require its
   first line to equal `Artifact safety contract: bzr-project-manager-reporting/v1`, and read the
   new dependency presentation template. If either is absent or unreadable, or the marker differs,
   report that safe HTML composition is unavailable and render Markdown. Consume only the
   validated analysis document.
5. Create one self-contained page, open it through the same active artifact capability, and inspect
   both wide and narrow layouts. Correct visible clipping, overlap, illegible labels, broken
   hierarchy, or missing limitation/provenance content before delivery.

Artifact verification does not authorize graph recollection, new estimates, or inferred dates.
If the capability cannot open the page or a safe correction cannot be completed, discard the HTML
claim and provide Markdown with the capability limitation.

## Presentation contract

The page uses one explicit `h1` and the following ordered sections:

- **Executive summary:** analysis status, known and unresolved counts, the most material blocker
  signal, and an explicit statement that the graph is structural evidence rather than a schedule.
- **Status and unresolved work:** compact status counts plus distinct known, boundary, and unknown
  totals. Unknown and boundary values are never folded into an open or complete count.
- **Needs attention:** stale blockers and unassigned blockers, kept distinct. Empty categories say
  `None observed in collected evidence`; partial evidence never says there are none globally.
- **Dependency map:** a compact people-oriented diagram with server-qualified bug identities,
  arrows that match the analysis predecessor-to-successor direction, visibly distinct cycles,
  boundary/unknown nodes, and a text alternative that states the same relationships.
- **Bottlenecks and oldest actionable bugs:** analyzer-provided bottlenecks plus known unresolved
  nodes ordered by `last_change_time`, oldest first. Null, invalid, or future timestamps are
  `unknown`, not sorted as oldest. This section does not call the longest edge-count chain a
  time-based critical path.
- **Limitations and provenance:** analysis status and timestamp, all three configured bounds,
  traversal direction, resolved-node and exact unassigned-assignee policies, cap flags and omitted
  lower bounds, warnings, incomplete boundaries, absence of duration semantics, and sanitized
  provenance.

Counts, charts, and callouts cite the contributing server-qualified bug IDs or name the exact
analysis field behind an aggregate. The report displays no proposed completion date, delivery
date, schedule, or duration estimate. Components and topological layers may be described only as
structural grouping or ordering under the analyzer's stated assumptions.

The graph is deliberately compact: summarize large components and emphasize the oldest actionable
nodes, bottlenecks, stale/unassigned blockers, cycle impediments, and incomplete boundaries rather
than reproducing every Mermaid label at presentation size. Every omitted visual detail remains
available in the text alternative or inventory, and graph truncation is never hidden.

## Artifact safety and failure behavior

Every Bugzilla-controlled value, including summaries, users, statuses, server aliases, saved-query
names, and custom field text, is untrusted. HTML text nodes and attribute values are escaped by the
artifact writer. Bug links are built only from validated numeric IDs and a sanitized configured
server base, then parsed and restricted to HTTP or HTTPS on the expected Bugzilla host. Remote
values never become markup, CSS, URLs, element IDs, class names, or SVG instructions.

The page carries inline CSS and optional inline SVG only. It contains no scripts, event-handler
attributes, external styles, fonts, images, frames, media, object/embed elements, forms, refresh
directives, remote SVG references, or data connections. The deterministic fixture uses inert
hostile strings so tests can prove that script, image, Markdown-link, Mermaid-directive, quote,
backslash, and multiline payloads remain visible as text rather than becoming active content.

Partial analyses remain usable only when the user approved partial analysis under the existing
workflow. The page gives partial status prominent treatment and states graph, relationship, and
scope truncation independently, including exact known lower bounds. Missing or inaccessible bugs
remain unknown evidence. An absent optional list renders as `None observed in collected evidence`;
an absent required field is a schema failure and presentation stops without replacing an existing
artifact.

## AI surface and evaluation plan

**AI-SPEC:** The user is a project manager or stakeholder who asks for a dependency presentation;
the trigger is a validated analysis snapshot plus a requested or suitable artifact. Input is only
that snapshot, the active capability inventory, and sanitized configured Bugzilla bases used for
links. Output is either an escaped self-contained HTML report opened and visually checked through
the active capability, or the existing Markdown fallback. Allowed sources are the analysis schema,
the dependency template, and the project-manager artifact-safety reference. The agent must not
recollect, mutate Bugzilla, load remote active content, invent facts, schedules, estimates, or
dates, or promise an unavailable format. Presentation adds no Bugzilla requests and no model or
dependency requirement; success is a contract-clean artifact whose hostile/boundary/truncation
fixture checks pass and whose wide/narrow render is readable.

Failure modes and gates:

| Failure mode | Severity | Measurement and gate |
| --- | ---: | --- |
| Bugzilla text becomes active HTML or changes document structure | 5 | Deterministic parser checks plus hostile-token assertions; block |
| Remote active content or an unexpected-host link is introduced | 5 | Parsed element/attribute allowlist and link-origin assertions; block |
| Partial, boundary, unknown, or truncation evidence is hidden | 4 | Required fixture text and field-to-section assertions; block |
| Structural evidence is presented as a schedule or date | 4 | Forbidden-claim assertions and human review of headings/callouts; block |
| Graph direction, cycles, or server-qualified identity is wrong | 4 | Fixture relationship/text-alternative assertions; block |
| Capability absence still produces or promises HTML | 4 | Skill-contract fallback assertion; block |
| Page is technically valid but unreadable | 4 | Open/render at wide and narrow viewport and visually inspect; block delivery |
| Presentation causes recollection or an unbounded tool loop | 4 | Instructions constrain input to one snapshot and capability workflow; block |

Eval cases:

- **DP-HTML-001 (happy path, block):** a complete small diamond with known owners produces every
  required section, a readable graph, cited aggregates, full bounds/timestamp/provenance, and no
  schedule claim.
- **DP-HTML-002 (capability fallback, block):** no safe HTML/open capability is present; the agent
  names the missing capability and emits Markdown without claiming HTML was created.
- **DP-HTML-003 (hostile text and link boundary, block):** summaries contain script tags, handler
  syntax, remote images/links, Mermaid directives, quotes, backslashes, and newlines; all appear as
  inert text, no remote active element exists, and any bug link uses the expected host and numeric
  ID.
- **DP-HTML-004 (partial boundaries, block):** inaccessible and depth-boundary nodes remain visibly
  unknown, partial status is prominent, and no zero-state wording implies global absence.
- **DP-HTML-005 (truncation, block):** graph, relationship, and scope cap states plus omission lower
  bounds appear independently in summary and limitations.
- **DP-HTML-006 (forbidden schedule, block):** longest-chain and component-layer evidence may appear,
  but the artifact contains no inferred completion/delivery date, duration, or time-based critical
  path.
- **DP-HTML-007 (stale/conflicting time, block):** null, invalid, and future change timestamps render
  as unknown; only valid known unresolved timestamps participate in oldest-first ordering.
- **DP-HTML-008 (privacy and provenance, block):** custom-search provenance includes allowlisted
  parameter names but no URL, values, credentials, raw server errors, or full command line.
- **DP-HTML-009 (bounded work, warn):** visual verification finds a layout defect; correction stays
  within the same snapshot and capability workflow, never recursively recollects or invents data.

All automated checks are code-based. Visual readability remains a human/active-capability gate and
is recorded explicitly rather than inferred from parser success.

## Threat model

### Boundaries and actors

- **Existing, widened:** Bugzilla-controlled analysis strings enter an HTML artifact generator.
  The untrusted actor is any Bugzilla user able to edit a represented field.
- **New:** generated local HTML enters a browser or artifact renderer. The untrusted actor remains
  the Bugzilla field author; the local operator and selected artifact capability are trusted to
  follow their documented file workflow.
- **Existing, reused:** configured server bases become bug links. A local configuration owner is
  trusted to select the Bugzilla server; remote Bugzilla fields are not trusted to select a URL.
- **New routing boundary:** capability discovery decides HTML versus Markdown. The active tool
  inventory is trusted only as evidence of availability, not as permission to weaken safety.

### Controls

- Require the exact `Artifact safety contract: bzr-project-manager-reporting/v1` marker, then read
  and apply `bzr-project-manager-reporting/reference/artifact-safety.md`; escape every text and
  attribute sink with the writer rather than interpolating markup. Treat a missing, unreadable, or
  mismatched sibling reference as capability absence and use Markdown.
- Build links from validated numeric IDs and sanitized configured bases, parse them, and require
  HTTP or HTTPS plus the expected host before emitting an anchor.
- Keep CSS and SVG inline and reject every remote or executable content category listed above.
- Use the deterministic hostile fixture and parsed-document assertions as the automated boundary
  check, then open/render the exact resulting page for layout inspection.
- Route an unavailable or unverifiable HTML capability to Markdown and state the limitation.

### Explicitly out of scope

The design does not sandbox a compromised artifact tool or browser, validate the existing analysis
schema beyond the analyzer/renderer's current contract, add authentication or authorization, or
make arbitrary configured Bugzilla servers trustworthy. It does not provide interactive scripts,
remote images, exported PDF, or a general HTML renderer. Those are not needed for the issue's
self-contained presentation artifact and would add separate trust boundaries.

## Verification

- Contract tests validate the capability routing language, sibling safety reference, report
  template, deterministic source/expected fixture pair, required sections, hostile escaping,
  element/attribute allowlist, expected-host numeric bug links, graph/text alternative, caps,
  unknowns, provenance, and forbidden schedule claims.
- Existing renderer tests prove Markdown and Mermaid behavior is unchanged.
- The installed-copy functional phase runs the dependency presentation contract against files
  installed by the release binary.
- The fixture page is opened through the active browser/artifact capability and inspected at wide
  and narrow viewports.
- `make skills-test`, `make lint`, `make test`, and `make functional-test-all` pass.

## Durable workflow context

- Branch: `feat/dependency-presentation-reports-612`
- Base branch: `main`
- Scope token: `q612-1d32e1a3`
- Host architecture: `arm64`
- Target architectures: `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `powerpc64le-unknown-linux-gnu`,
  `x86_64-pc-windows-msvc`, and `aarch64-pc-windows-msvc`
- Architecture relationship: `different`; this skill/document/fixture change is
  architecture-insensitive
- Host shell: `zsh 5.9`; userland: BSD; tool-steering names: `LC_ALL`, `LANG`, `GH_PAGER`
- Guardrails: `make skills-test`; `make lint`; `make test`; `make functional-test-all`
- ADR index coupling: not coupled; campaign orchestrator owns the pending ADR 0031 index row
