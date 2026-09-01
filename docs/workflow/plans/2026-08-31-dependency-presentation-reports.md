# Dependency presentation reports implementation plan

**Goal:** Extend the bundled dependency-analysis skill with an optional, safe,
presentation-ready HTML workflow while retaining deterministic Markdown as the fallback.

**Architecture:** Keep collection, analysis, and `render.py` unchanged. The active artifact
capability composes HTML from a dependency-owned template after reading the sibling
project-manager artifact-safety contract. A checked-in hostile/partial/truncated example and a
focused standard-library test define the expected structure; the installed functional phase proves
that all references and the test work from the packaged skill tree.

**Tech stack:** Markdown skill contracts and template, JSON/HTML fixtures, Python 3 standard
library, POSIX/Bash contract tests, and the existing functional shell harness.

## Global constraints

- Input is one validated `bzr-dependency-analysis/v1` snapshot; presentation never recollects or
  combines snapshots.
- HTML is optional and is available only when the active capability can create escaped,
  self-contained HTML and open or render the local result. Missing capability, template, or safety
  guidance routes to Markdown with the missing capability stated.
- Reuse `bzr-project-manager-reporting/reference/artifact-safety.md`; do not copy a second normative
  safety policy.
- Escape every Bugzilla-controlled text node and attribute value. Include no remote scripts,
  styles, fonts, images, frames, media, forms, embeds, event handlers, refresh directives, CSS
  `url()`, or CSS `@import`.
- Create bug links only from validated numeric IDs and an operator-confirmed sanitized Bugzilla
  base. Otherwise render server-qualified identities as plain text.
- Show status/unresolved summaries, stale and unassigned blockers, a compact people-oriented
  dependency map with a text alternative, bottlenecks, oldest actionable bugs, bounds, timestamp,
  unknowns, truncation, and sanitized provenance.
- Do not infer schedules, estimates, delivery dates, or duration-based critical paths.
- Do not add a `bzr` or `render.py` HTML mode, general presentation validator, tool-event oracle,
  claim-by-claim source-binding framework, or candidate-file atomic-promotion protocol.
- Keep the implementation within `content/skills/bzr-dependency-analysis/**`,
  `content/skills/bzr-project-manager-reporting/**`, directly related skill tests/docs, and
  `tests/functional/phases/18d-dependency-analysis.sh`.
- The content and test change is architecture-insensitive. Repository targets remain
  `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `powerpc64le-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, and
  `aarch64-pc-windows-msvc`.
- Guardrails are `make skills-test`, `make lint`, `make test`, and
  `make functional-test-all`.

## File map

- Modify `content/skills/bzr-dependency-analysis/SKILL.md`: capability gate, presentation
  composition workflow, Markdown fallback, required references, and visual inspection contract.
- Modify `content/skills/bzr-project-manager-reporting/SKILL.md`: clarify that dependency-analysis
  owns dependency semantics while reusing the PM capability and artifact-safety contract.
- Create `content/skills/bzr-dependency-analysis/reference/presentation-report.md`: ordered
  dependency presentation sections, evidence mapping, map/text-alternative rules, truncation, and
  no-schedule language.
- Create `content/skills/bzr-dependency-analysis/tests/fixtures/presentation.analysis.json`: one
  deterministic hostile, partial, truncated analysis snapshot.
- Create `content/skills/bzr-dependency-analysis/tests/fixtures/presentation.expected.html`: the
  byte-stable representative self-contained page.
- Create `content/skills/bzr-dependency-analysis/tests/test_presentation.py`: fixture-only HTML
  structure and safety assertions using `html.parser`, `json`, and `unittest`.
- Modify `content/skills/bzr-dependency-analysis/tests/skill-contract.sh`: require presentation
  prose, template, fixture, and focused test references.
- Modify `content/skills/bzr-project-manager-reporting/tests/run.sh`: require the dependency
  composition and ownership guidance.
- Modify `agent-skills/tests/run.sh`: execute the dependency contract and presentation fixture
  tests in the aggregate skills gate.
- Modify `src/skills/embedded_tests.rs`: add the four presentation payload paths to the exact
  embedded dependency-analysis inventory.
- Modify `tests/functional/phases/18c-skills-install.sh`: add the same four paths to the exact
  installed dependency-analysis inventory used for both project layouts.
- Modify `tests/functional/phases/18d-dependency-analysis.sh`: resolve the new files and sibling PM
  safety reference from the installed tree and run the focused test there.

## Task 1: Ship the dependency presentation contract and its installed proof

This is one reviewable deliverable: the prose and template define what the example means, the
fixture test proves the checked-in example, and the functional phase proves the bundled copy.
Splitting those pieces would leave either an unproved contract or a test with no normative owner.

### Interfaces

- Consumes `bzr-dependency-analysis/v1` fields already emitted by `analyze.py`; no schema or Python
  production interface changes.
- Consumes the existing sibling path
  `../bzr-project-manager-reporting/reference/artifact-safety.md` from the installed skills root.
- Defines `reference/presentation-report.md` as the dependency-specific composition guide.
- Defines `tests/test_presentation.py` as a no-argument executable unittest module. It reads
  `fixtures/presentation.analysis.json` and `fixtures/presentation.expected.html` relative to its
  own file and returns exit 0 only when the checked-in pair satisfies the contract.
- The installed functional phase relies on those exact relative paths; later packaging code relies
  only on the existing recursive skill embedding and installer behavior.

### Step 1: Add failing contract and installed-path assertions

Modify `content/skills/bzr-dependency-analysis/tests/skill-contract.sh` to require these exact
contract fragments and files:

```bash
presentation="$skill_root/reference/presentation-report.md"
presentation_test="$skill_root/tests/test_presentation.py"
presentation_analysis="$skill_root/tests/fixtures/presentation.analysis.json"
presentation_html="$skill_root/tests/fixtures/presentation.expected.html"

require_words 'safe HTML-capable artifact tool'
require_words 'Markdown fallback'
require_literal '../bzr-project-manager-reporting/reference/artifact-safety.md'
require_literal 'reference/presentation-report.md'
for path in "$presentation" "$presentation_test" "$presentation_analysis" "$presentation_html"; do
  [[ -f "$path" ]] || fail "missing presentation contract file: $path"
done
```

Modify `content/skills/bzr-project-manager-reporting/tests/run.sh` to require dependency composition
guidance:

```sh
grep -Fq 'dependency-analysis owns dependency-specific presentation semantics' "$SKILL"
grep -Fq 'reference/artifact-safety.md' "$SKILL"
```

Modify `agent-skills/tests/run.sh` so the aggregate gate runs both new dependency checks:

```sh
bash "$HERE/../../content/skills/bzr-dependency-analysis/tests/skill-contract.sh" || rc=1
python3 "$HERE/../../content/skills/bzr-dependency-analysis/tests/test_presentation.py" || rc=1
```

Add these exact relative paths, in lexical order, to both the dependency-analysis vector in
`src/skills/embedded_tests.rs` and `DEPENDENCY_ANALYSIS_PAYLOAD` in
`tests/functional/phases/18c-skills-install.sh`:

```text
reference/presentation-report.md
tests/fixtures/presentation.analysis.json
tests/fixtures/presentation.expected.html
tests/test_presentation.py
```

The Rust inventory proves recursive embedding and phase 18c proves both installed project layouts;
neither exact-inventory assertion may be left stale.

Extend the `_DA_PATH` installed-root loop in
`tests/functional/phases/18d-dependency-analysis.sh` with only the dependency-owned template,
fixture pair, and focused test; keep its existing containment rule under
`$_DA_SKILL_ROOT_CANONICAL`. Check the sibling PM safety reference separately: require its exact
installed location at
`$SKILLS_PROJECT/.agents/skills/bzr-project-manager-reporting/reference/artifact-safety.md`, require
it to be a regular file, canonicalize it beneath `$SKILLS_PROJECT/.agents/skills`, and do not widen
the dependency-root loop to admit arbitrary sibling paths. Then run:

```bash
python3 "$_DA_PRESENTATION_TEST"
```

and pass only on exit 0. The test ID is
`installed-presentation-contract-preserves-safe-partial-evidence`.

Run:

```sh
bash content/skills/bzr-dependency-analysis/tests/skill-contract.sh
```

Expected result: non-zero with the first missing presentation contract fragment or file. This is
the required red proof; do not add production prose or fixtures before observing it.

### Step 2: Write the minimum skill and template contract

Modify the two `SKILL.md` files and create `reference/presentation-report.md`. The dependency skill
must say, in order:

1. finish the existing bounded analysis once;
2. inspect active artifact capabilities;
3. read the PM safety reference and dependency template before HTML composition;
4. use the validated snapshot as the only report evidence;
5. generate escaped self-contained HTML or take the Markdown fallback;
6. open the exact generated page, inspect wide and narrow layouts, correct visible defects, and
   fall back to Markdown if safe readable HTML cannot be delivered.

The template must define these ordered sections: Executive summary; Status and unresolved work;
Needs attention; Dependency map; Bottlenecks and oldest actionable bugs; Limitations and
provenance. It must require a text alternative adjacent to the diagram, explicit empty-observation
wording, server-qualified identities, predecessor-to-successor arrows, visible unknown/boundary
nodes and cycles, valid-timestamp ordering with invalid/null timestamps shown as unknown, all
analysis bounds/caps/omission lower bounds/policies/warnings/provenance, and no schedule language.

The PM skill addition must keep one safety owner and direct dependency-specific semantics back to
the dependency template. It must not copy the dependency section list or HTML restrictions.

### Step 3: Add the deterministic fixture and focused test

Create `presentation.analysis.json` as a valid compact analysis snapshot containing a hostile
summary with literal `<script>`, `onerror`, a remote image, Markdown-link, and Mermaid directive
text; known, unassigned, stale, boundary, and unknown nodes; one bottleneck; a short directed path;
`status: partial`; a reached cap; a positive omission lower bound; bounds, timestamp, policy,
warnings, and sanitized provenance.

Create the self-contained expected HTML from that fixture. Use semantic headings and lists, inline
CSS, and a compact inline SVG with `role="img"` plus an adjacent textual edge list. Hostile source
text must appear only escaped in text content. Do not add JavaScript or remote resources.

Create `test_presentation.py` with a private `HTMLParser` subclass that records start tags,
attributes, headings, visible text, `<style>` element data, and SVG/title/description presence.
Tests must:

- run the existing `scripts/render.py` against the analysis fixture in a temporary directory and
  require a successful Markdown render, proving the JSON passes the existing strict v1 validator;
- load and assert the analysis fixture is partial and truncated;
- reject forbidden active tags and `on*`, `src`, `srcset`, remote `href`, refresh, `style`
  attributes containing case-insensitive `url(` or `@import`, `<style>` element data containing
  either token case-insensitively, and unescaped hostile markup;
- require the six ordered report sections, status/known/boundary/unknown counts, stale and
  unassigned blockers, bottleneck and oldest-actionable identities, graph role and adjacent text
  alternative, bounds, timestamp, policies, cap flags, omission lower bound, warnings, unknowns,
  and provenance; and
- reject the phrases `delivery date`, `project schedule`, and `critical path` from rendered text.

This parser accepts no CLI arguments and validates no caller-supplied file. It is intentionally a
fixture check, not a runtime validator.

Run:

```sh
python3 content/skills/bzr-dependency-analysis/tests/test_presentation.py
bash content/skills/bzr-dependency-analysis/tests/skill-contract.sh
cargo build --locked
BZR_BIN=target/debug/bzr sh content/skills/bzr-project-manager-reporting/tests/run.sh
```

Expected result: all presentation unittests pass, the dependency contract prints
`dependency-analysis skill contract: ok`, the debug binary is built, and the PM contract exits 0.

Verify the new test bites with two controlled faults in scratch copies: replace one escaped hostile
marker with an active tag, then separately add `@import url("https://evil.invalid/x.css")` inside
the inline `<style>` block. Point an equally temporary copy of the test module at each scratch
fixture. Expected result: the hostile-content assertion fails for the first and the style-content
assertion fails for the second. Remove the scratch directory after both faults; do not alter the
checked-in fixture for this proof.

### Step 4: Run aggregate, installed, and visual acceptance

Run:

```sh
make skills-test
```

Expected result: exit 0 including the dependency contract, presentation fixture test, PM contract,
package-content checks, and installer checks.

Run the repository functional harness after confirming Docker or podman and the target host meet
the existing requirements:

```sh
make functional-test-all
```

Expected result: exit 0 for every supported Bugzilla container, including
`installed-presentation-contract-preserves-safe-partial-evidence`. The installed phase must resolve
and execute only paths below `$SKILLS_PROJECT/.agents/skills`; it must not fall back to the source
tree.

Use the active file-capable artifact workflow to generate
`content/skills/bzr-dependency-analysis/tests/fixtures/presentation.expected.html` from
`presentation.analysis.json` after reading the dependency template and PM safety reference. Do not
copy an unrelated page or treat the template as report evidence. Then open that exact generated
file through the active browser/artifact capability and inspect one wide viewport and one narrow
viewport for clipping, overlap, illegible labels, hierarchy, diagram/text agreement, limitations,
and provenance. Record the generation capability, inspection capability, and both viewport results
in the quest handoff. If any view is unusable, correct the generated fixture/template, rerun the
focused and aggregate tests, and repeat the visual inspection; if a safe readable page cannot be
produced, the acceptance result is Markdown fallback rather than a claimed HTML success.

### Step 5: Commit the logical implementation

After the focused and aggregate checks are green, stage only the paths in the file map and commit:

```sh
git commit -m 'feat(skills): add dependency presentation reports'
```

Acceptance criteria for the commit:

- all six report sections and evidence limitations are visible in the example;
- hostile Bugzilla text remains inert and the page has no remote active content;
- Markdown fallback and no-schedule semantics are explicit;
- the aggregate skills gate runs the focused fixture check;
- the installed-copy phase proves the template, fixture, test, and reused PM safety contract; and
- one representative page has passed wide and narrow active-capability visual inspection.

Rollback is deletion/reversion of this single content/test commit; no schema, runtime interface,
dependency, persisted data, or Bugzilla state needs migration or cleanup.

## Final branch verification

Run bare, without output-hiding pipelines:

```sh
make lint
make test
make functional-test-all
```

Expected result: all commands exit 0. `make lint` includes Rust, shell, and functional-phase syntax
checks; `make test` is the quiet complete Rust suite; `make functional-test-all` is the live
installed-copy proof across supported Bugzilla versions.
