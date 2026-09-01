# Implementation plan — functional-harness capabilities (issue #617)

**Goal.** Land the five harness capabilities epic #616's conformance entries depend on, without
correcting a single fixture and without touching a `src/` production path.

**Architecture.** Everything lives in `tests/functional/` plus three project files. The proxy
(`redhat-shape-proxy.py`, python3 stdlib only) gains a rewrite-hook registry; the shell harness
(`lib.sh`, phase scripts) gains a fixture user and three helpers; `Makefile`,
`.github/workflows/ci.yml`, and `CONTRIBUTING.md` gain a target, a CI step, and a procedure. Four
Rust test files gain comment-only markers.

**Design record.** `docs/workflow/specs/2026-08-31-functional-harness-capabilities-design.md`.

**Tech stack.** bash 5 (the harness), python3 stdlib (the proxy, `unittest` for its self-tests),
GNU Make, Rust 2021 (comments only).

## Global constraints

Every task's requirements implicitly include this section.

- **No fixture value changes and no `src/` production-path changes.** Rust edits in this plan are
  comment lines only. Changing an asserted value would make this change red on arrival; epic #616
  requirement R4 assigns that correction to the dependent entry.
- **Do not edit `src/cli/product.rs` or `tests/functional/phases/03-products.sh`.** A concurrent
  run owns them (issue #618). `03-products.sh` is nevertheless a consumer of the proxy and its
  assertions must keep passing.
- **Preserve the proxy's stderr marker format byte-for-byte.**
  `03-products.sh:71-83` counts lines matching
  `/metadata-sort-keys shaped route=field count=[1-9][0-9]*/` and the `route=product` variant;
  `:81-84` additionally requires `server capabilities` to raise the `route=field` count.
- **Functional test IDs.** `test_begin "<slug>" "<description>"` on one line, two literal
  arguments, slug matching `^[a-z0-9]+(-[a-z0-9]+)*$`, unique within its phase.
  `make check-functional-test-ids` enforces it.
- **Shell style.** `make check-shell` runs `shellcheck -s bash` and `bash -n` over
  `tests/functional/lib.sh` and every `tests/functional/phases/*.sh`. It does **not** run `shfmt`
  over `tests/functional/`; match the surrounding 4-space indentation by hand.
- **Guardrails.** `make lint`, `make test`, `make functional-test-all`. Never bare `cargo test`.
- **Commits.** Conventional Commits, imperative, subject ≤72 chars, one logical change each,
  ending with the trailer `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  Infra scopes (`test`, `docs`, `ci`, `build`) are excluded from the generated changelog by
  design, which is correct for this change — it alters no compiled `bzr` behavior.

## File map

| File | Created / modified | Answerable for |
|---|---|---|
| `Makefile` | modified | the `functional-test-bz50` target and the `check-proxy-self-test` guard |
| `.github/workflows/ci.yml` | modified | running `check-proxy-self-test` on every pull request |
| `tests/functional/redhat-shape-proxy.py` | modified | the rewrite-hook registry, dispatcher, and self-tests |
| `tests/functional/README.md` | modified | documenting how to add a rewrite hook |
| `tests/functional/lib.sh` | modified | the non-member fixture global and its three helpers |
| `tests/functional/phases/07-groups.sh` | modified | provisioning the fixture; `TODO(#625)` markers |
| `tests/functional/phases/02-server-auth.sh` | modified | the `TODO(#626)` marker |
| `CONTRIBUTING.md` | modified | the controlled-fault procedure |
| `src/commands/bug/clone_tests.rs` | modified | `TODO(#621)` markers (comments only) |
| `src/client/resources/group_tests.rs` | modified | `TODO(#625)` markers (comments only) |
| `src/client/resources/server_tests.rs` | modified | `TODO(#626)` markers (comments only) |
| `src/xmlrpc/resources/mappers_tests.rs` | modified | `TODO(#622)` marker (comments only) |

## Task 1 — the `functional-test-bz50` target

**Modifies:** `Makefile`. **Tests:** `make -n functional-test-bz50`.

**Interfaces.** Consumes nothing. Later tasks rely on nothing from it; Task 5's CONTRIBUTING text
names the target by name.

### Steps

1. In the `.PHONY` list, change the line
   `        functional-test-bz52 functional-test-bz53 functional-test-all functional-stop-all \`
   to
   `        functional-test-bz50 functional-test-bz52 functional-test-bz53 functional-test-all functional-stop-all \`

2. Immediately before the `functional-test-bz52:` rule, insert:

```make
# `make functional-test` is the unpinned form of this target: it runs whatever
# tests/functional/container-env.sh defaults BZ_VERSION to, which is bz50 today.
# The two agree only while that default is bz50; moving it makes them diverge
# silently, since both still succeed.
functional-test-bz50: ## Run functional tests against Bugzilla 5.0
	BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh start
	BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
```

   Recipe lines are **tab**-indented, matching the sibling rules.

3. Verify: `make -n functional-test-bz50`. Expect exactly two echoed lines,
   `BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh start` and
   `BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh`, and exit status 0.

4. Verify the rule and its help text landed:
   `rg -n 'functional-test-bz50: ## Run functional tests against Bugzilla 5.0' Makefile`.
   Expect one hit.

   Do **not** verify through `make help`. Its filter at `Makefile:223` is
   `grep -E '^[a-zA-Z_-]+:.*##'`, whose character class has no digit range, so no target with a
   digit in its name is listed — `functional-test-bz52` and `functional-test-bz53` are already
   invisible there today (`make help | grep -c functional-test-bz52` prints `0`). Widening that
   class would change `make help` output for targets this change does not own, so it stays out
   of scope.

   **Knowingly unowned.** No issue is filed for the help-filter gap and none is planned: it is
   pre-existing (two sibling targets are already invisible), this change adds a third rather than
   creating the condition, and the discovery path contributors actually use is the
   `CONTRIBUTING.md` procedure, which names all three arms. Recorded here so it is a decision
   rather than an omission.

5. Commit: `feat(test): add a functional-test-bz50 make target`.

### Acceptance criteria

- `make -n functional-test-bz50` prints the two commands above and exits 0.
- `functional-test-bz50` is in `.PHONY`.
- `run-all-versions.sh` is unchanged — it remains the only list of matrix versions.

## Task 2 — gate the proxy self-tests

**Modifies:** `Makefile`, `.github/workflows/ci.yml`. **Tests:** `make check-proxy-self-test`.

This task must land **after** Task 3 in the working tree if you run the guard before committing —
the guard runs the self-tests, which Task 3 extends. Landing it first is fine because the existing
self-tests already pass; run `make check-proxy-self-test` after each of the two tasks.

**Interfaces.** Consumes `python3 tests/functional/redhat-shape-proxy.py --self-test` (exists
today at `redhat-shape-proxy.py:391`, exit 0 on success, 1 on failure). Task 3's comment block
refers to this target by name.

### Steps

1. In `Makefile`'s `.PHONY` list, `Makefile:6` currently reads exactly
   `        check-no-spawn check-release-security-notes check-shell clean help man \`.
   Insert one token, keeping every existing one:
   `        check-no-spawn check-release-security-notes check-shell check-proxy-self-test clean help man \`

   Do not drop `clean help man`. `man` in particular must stay phony: `Makefile:156-157` writes
   into a `man/` directory that `.gitignore:5` ignores, so once `make man` has run the directory
   exists and a non-phony `man` target reports `make: 'man' is up to date.` and silently stops
   regenerating the pages.

2. **Leave the `lint` rule alone.** `check-proxy-self-test` is deliberately not a `lint`
   prerequisite: four of the suite's cases bind a real loopback socket, so adding it would give
   the repository's primary guardrail a `python3` and TCP requirement where every existing
   prerequisite is offline filesystem work — and buy nothing, since no workflow runs `make lint`
   and step 4's CI step is what actually blocks a pull request. See the design record's "The
   target is deliberately *not* a `make lint` prerequisite".

3. Immediately after the `check-shell` rule (the one ending with the `shfmt -d -ln bash` line),
   insert:

```make
check-proxy-self-test: ## Run the production-shape proxy self-tests
	@command -v python3 >/dev/null || { echo "ERROR: python3 is required for this guard"; echo "  Install your platform's python3 package"; exit 1; }
	python3 tests/functional/redhat-shape-proxy.py --self-test
```

4. In `.github/workflows/ci.yml`, in the `test-layout` job, after the line
   `      - run: make check-no-spawn`, add:

```yaml
      - run: make check-proxy-self-test
```

   Match the surrounding six-space indentation exactly. Add no `setup-python` step: `python3` is
   preinstalled on `ubuntu-latest`.

5. Verify: `make check-proxy-self-test`. Expect the unittest runner to print one `... ok` line per
   case and a final `OK`, and the command to exit 0. Run it bare — no pipe — so the exit status is
   the make target's own.

6. Verify the workflow edit with `git diff .github/workflows/ci.yml`: the added line must sit
   inside the `test-layout` job's `steps:` list, aligned with its siblings. The repository has no
   YAML linter in its guardrails, so this is a read, not a command.

7. Commit: `ci: run the production-shape proxy self-tests as a guard`.

### Acceptance criteria

- `make check-proxy-self-test` exits 0 and runs every case in `ShapeTests`.
- `make lint`'s prerequisite list is **unchanged**: `git diff Makefile` shows no edit to the
  `lint:` rule.
- `ci.yml`'s `test-layout` job has four `make check-*` steps.
- `git diff Makefile` shows the `.PHONY` block gained exactly one token and lost none — in
  particular `clean`, `help`, and `man` are still there.

## Task 3 — per-endpoint rewrite hooks in the proxy

**Modifies:** `tests/functional/redhat-shape-proxy.py`, `tests/functional/README.md`.
**Tests:** `python3 tests/functional/redhat-shape-proxy.py --self-test`.

**Interfaces.**

- Consumes: nothing from earlier tasks.
- Provides, for later entries (#620, #626, #627, #629) and for Task 2's guard:
  - `ResponseHook = collections.namedtuple("ResponseHook", "name matches route rewrite")`
  - `RESPONSE_HOOKS: tuple[ResponseHook, ...]`
  - `apply_response_hooks(path: str, body: bytes) -> tuple[bytes, list[tuple[str, str, int]]]`
  - `is_metadata_sort_key_route(path: str) -> bool`
  - `metadata_sort_key_route(path: str) -> str`
  - rewriter signature `rewrite(path: str, body: bytes) -> tuple[bytes, int]`, adopted by
    `shape_bug_response`, `shape_product_ids_response`, and
    `shape_metadata_sort_keys_response` (the last already had it).

### Steps

1. Add `import collections` to the import block, in alphabetical position — before
   `import http.client`.

2. Change `shape_bug_response` to the uniform signature and a change count. Replace the whole
   function with:

```python
def shape_bug_response(path, data):
    """Return JSON bytes with bug component/version values represented as arrays.

    The count is every field whose scalar value was replaced by a list, the empty
    string included — `""` becomes `[]`, which is a rewrite even though it gains
    no secondary entry.
    """
    del path  # every /rest/bug body is shaped the same way
    value = json.loads(data)
    bugs = value.get("bugs") if isinstance(value, dict) else None
    changed = 0
    if isinstance(bugs, list):
        for bug in bugs:
            if not isinstance(bug, dict):
                continue
            for field in ("component", "version"):
                field_value = bug.get(field)
                if isinstance(field_value, str):
                    values = [] if field_value == "" else [field_value]
                    if values:
                        values.append(f"{field_value}-redhat-secondary")
                    bug[field] = values
                    changed += 1
    return json.dumps(value, separators=(",", ":")).encode(), changed
```

3. Change `shape_product_ids_response` the same way. Replace the whole function with:

```python
def shape_product_ids_response(path, data):
    """Return JSON bytes with product IDs represented as decimal strings.

    Some Bugzilla stacks (e.g. bugzilla.kernel.org) serialize
    `get_{accessible,selectable,enterable}_products` ids as strings rather than
    numbers. This rewrites every `ids` element to its decimal string form so the
    client is exercised against the string wire shape the same endpoint serves
    in the wild. The count is the number of non-string elements rewritten.
    """
    del path  # every product_* body is shaped the same way
    value = json.loads(data)
    changed = 0
    if isinstance(value, dict) and isinstance(value.get("ids"), list):
        ids = value["ids"]
        changed = sum(1 for item in ids if not isinstance(item, str))
        value["ids"] = [str(item) for item in ids]
    return json.dumps(value, separators=(",", ":")).encode(), changed
```

4. Extract the sort-key route predicate. Immediately **above**
   `def shape_metadata_sort_keys_response`, insert:

```python
def is_metadata_sort_key_route(path):
    """Return whether <path> is a metadata route whose sort keys are rewritten."""
    parsed_path = urllib.parse.urlsplit(path).path
    return parsed_path.startswith("/rest/field/bug") or (
        parsed_path == "/rest/product" or parsed_path.startswith("/rest/product/")
    )


def metadata_sort_key_route(path):
    """Return `field` or `product`, the marker sub-route for a sort-key path."""
    if urllib.parse.urlsplit(path).path.startswith("/rest/field/bug"):
        return "field"
    return "product"
```

5. Rewrite `shape_metadata_sort_keys_response`'s own guard to use the new predicate, keeping the
   `is_field` / `is_product` locals it needs afterwards. **Apply this edit before step 4**, or
   quote the whole span below as the anchor — after step 4 lands, the single line
   `    parsed_path = urllib.parse.urlsplit(path).path` occurs twice in the file and
   `        return data, 0` already occurs twice inside this function, so neither anchor is unique
   on its own. Replace exactly this span:

```python
    parsed_path = urllib.parse.urlsplit(path).path
    is_field = parsed_path.startswith("/rest/field/bug")
    is_product = parsed_path == "/rest/product" or parsed_path.startswith(
        "/rest/product/"
    )
    if not is_field and not is_product:
        return data, 0
```

   with:

```python
    if not is_metadata_sort_key_route(path):
        return data, 0
    parsed_path = urllib.parse.urlsplit(path).path
    is_field = parsed_path.startswith("/rest/field/bug")
    is_product = not is_field
```

   The rest of the function is unchanged. The guard is now redundant with the hook's matcher and
   is kept deliberately so the function stays correct called directly, which its self-tests do.

6. Immediately after `metadata_sort_key_route` and the three shape functions — that is, just
   before `def make_handler` — insert the registry, its documentation, and the dispatcher:

```python
# ── Per-endpoint response rewrite hooks ──────────────────────────────
#
# Each hook rewrites one endpoint family's successful (2xx) response into a
# shape a real deployment serves, so the compiled CLI is exercised against that
# shape without patching the container. Accepted ADR 0028 is the governing
# record: a leniency finding is proved by rewriting the endpoint's response
# here, with self-tests, rather than by assertion.
#
# A hook has four fields:
#
#   name     Stable marker token. When a hook changes something the proxy
#            writes "<name> shaped route=<route> count=<n>" to stderr, and
#            phase scripts count those lines. Treat a shipped name as a
#            contract: tests/functional/phases/03-products.sh matches
#            "metadata-sort-keys shaped route=field count=[1-9][0-9]*".
#   matches  matches(path) -> bool, over the raw request path including its
#            query string, exactly as the handler receives it.
#   route    route(path) -> str, the marker's sub-route label, so one hook
#            spanning two endpoints reports which one fired.
#   rewrite  rewrite(path, body) -> (bytes, int). The count governs the marker
#            only: apply_response_hooks always adopts the returned body, zero
#            count included, so a hook that changed nothing must still return
#            a body it is happy to serve.
#
# To add one: write the transform with that signature, append a ResponseHook
# below, and add self-tests that dispatch a real path for it through
# apply_response_hooks — not just the transform directly, which would leave the
# matcher unguarded. `make check-proxy-self-test` runs them, and CI runs that
# target on every pull request, which is what enforces the obligation.
ResponseHook = collections.namedtuple("ResponseHook", "name matches route rewrite")

RESPONSE_HOOKS = (
    ResponseHook(
        name="bug-multivalue",
        matches=lambda path: path.startswith("/rest/bug"),
        route=lambda path: "bug",
        rewrite=shape_bug_response,
    ),
    ResponseHook(
        name="product-ids",
        matches=lambda path: path.startswith(
            ("/rest/product_accessible", "/rest/product_selectable",
             "/rest/product_enterable")
        ),
        route=lambda path: "product-ids",
        rewrite=shape_product_ids_response,
    ),
    ResponseHook(
        name="metadata-sort-keys",
        matches=is_metadata_sort_key_route,
        route=metadata_sort_key_route,
        rewrite=shape_metadata_sort_keys_response,
    ),
)


def apply_response_hooks(path, body):
    """Apply every hook matching <path> to a successful response body.

    Returns the rewritten body and one (name, route, count) entry per hook that
    changed something, in registry order.
    """
    applied = []
    for hook in RESPONSE_HOOKS:
        if not hook.matches(path):
            continue
        body, count = hook.rewrite(path, body)
        if count:
            applied.append((hook.name, hook.route(path), count))
    return body, applied
```

7. Replace the three inline rewrite blocks in `_forward`. Delete everything from
   `            if 200 <= status < 300 and self.path.startswith("/rest/bug"):` through the
   `                    sys.stderr.flush()` line, and put in its place:

```python
            if 200 <= status < 300:
                try:
                    body, applied = apply_response_hooks(self.path, body)
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    self.send_error(502, f"Bugzilla returned malformed JSON: {error}")
                    return
                for name, route, count in applied:
                    sys.stderr.write(f"{name} shaped route={route} count={count}\n")
                if applied:
                    sys.stderr.flush()
```

   The `self.send_response(status)` line that follows is unchanged.

8. Update the three existing self-tests that call the two changed transforms. In `ShapeTests`,
   replace `test_shapes_scalar_empty_and_multi_values`, `test_leaves_non_bug_payload_untouched`,
   and `test_rejects_malformed_json` with:

```python
    def test_shapes_scalar_empty_and_multi_values(self):
        body, count = shape_bug_response("/rest/bug", json.dumps({"bugs": [
            {"component": "Backend", "version": "rawhide"},
            {"component": [], "version": ["40", "41"]},
            {"component": "", "version": ""},
        ]}).encode())
        shaped = json.loads(body)
        self.assertEqual(count, 4)
        self.assertEqual(
            shaped["bugs"][0]["component"],
            ["Backend", "Backend-redhat-secondary"],
        )
        self.assertEqual(
            shaped["bugs"][0]["version"], ["rawhide", "rawhide-redhat-secondary"]
        )
        self.assertEqual(shaped["bugs"][1]["version"], ["40", "41"])
        self.assertEqual(shaped["bugs"][2]["component"], [])

    def test_leaves_non_bug_payload_untouched(self):
        body, count = shape_bug_response("/rest/bug", b'{"version":"5.2"}')
        self.assertEqual(json.loads(body), {"version": "5.2"})
        self.assertEqual(count, 0)

    def test_rejects_malformed_json(self):
        with self.assertRaises(json.JSONDecodeError):
            shape_bug_response("/rest/bug", b"not json")
```

   The count is 4: bug 0 rewrites both fields, bug 1 rewrites neither (both are already lists),
   bug 2 rewrites both empty strings to `[]`.

9. Update the three product-ids self-tests. Replace `test_products_shape_to_string_ids`,
   `test_products_shape_preserves_existing_string_ids`, and
   `test_products_shape_leaves_non_ids_payload_untouched` with:

```python
    def test_products_shape_to_string_ids(self):
        body, count = shape_product_ids_response(
            "/rest/product_accessible", json.dumps({"ids": [12, 1, 21]}).encode()
        )
        self.assertEqual(json.loads(body), {"ids": ["12", "1", "21"]})
        self.assertEqual(count, 3)

    def test_products_shape_preserves_existing_string_ids(self):
        body, count = shape_product_ids_response(
            "/rest/product_accessible", b'{"ids":["2","3","19"]}'
        )
        self.assertEqual(json.loads(body), {"ids": ["2", "3", "19"]})
        self.assertEqual(count, 0)

    def test_products_shape_leaves_non_ids_payload_untouched(self):
        body, count = shape_product_ids_response(
            "/rest/product_accessible", b'{"products":[]}'
        )
        self.assertEqual(json.loads(body), {"products": []})
        self.assertEqual(count, 0)
```

10. Add the six registry self-tests. Insert them into `ShapeTests` immediately after
    `test_metadata_sort_key_shape_leaves_unrelated_payload_untouched`:

```python
    def test_response_hooks_leave_unmatched_paths_untouched(self):
        payload = b'{"version":"5.2"}'
        body, applied = apply_response_hooks("/rest/version", payload)
        self.assertEqual(body, payload)
        self.assertEqual(applied, [])

    def test_response_hooks_report_bug_route_and_count(self):
        payload = json.dumps({"bugs": [
            {"component": "Backend", "version": "rawhide"},
            {"component": ["already"], "version": "f40"},
        ]}).encode()
        body, applied = apply_response_hooks("/rest/bug?limit=2", payload)
        self.assertEqual(applied, [("bug-multivalue", "bug", 3)])
        self.assertEqual(json.loads(body)["bugs"][1]["component"], ["already"])

    def test_response_hooks_report_product_ids_route(self):
        body, applied = apply_response_hooks(
            "/rest/product_accessible", b'{"ids":[7,8]}'
        )
        self.assertEqual(applied, [("product-ids", "product-ids", 2)])
        self.assertEqual(json.loads(body), {"ids": ["7", "8"]})
        _, unmatched = apply_response_hooks("/rest/product", b'{"ids":[7,8]}')
        self.assertEqual(unmatched, [])

    def test_response_hooks_report_metadata_routes(self):
        field_payload = json.dumps({"fields": [{"values": [
            {"id": 1, "sort_key": 10},
            {"id": 2, "sort_key": 20},
            {"id": 3, "sort_key": 30},
        ]}]}).encode()
        _, applied = apply_response_hooks("/rest/field/bug/status", field_payload)
        self.assertEqual(applied, [("metadata-sort-keys", "field", 3)])
        product_payload = json.dumps(
            {"products": [{"versions": [{"id": 9, "sort_key": 5}]}]}
        ).encode()
        _, applied = apply_response_hooks("/rest/product", product_payload)
        self.assertEqual(applied, [("metadata-sort-keys", "product", 1)])

    def test_matching_hook_that_changes_nothing_reports_no_entry(self):
        body, applied = apply_response_hooks(
            "/rest/product_selectable", b'{"ids":["7"]}'
        )
        self.assertEqual(applied, [])
        self.assertEqual(json.loads(body), {"ids": ["7"]})

    def test_every_hook_honors_the_registry_contract(self):
        for hook in RESPONSE_HOOKS:
            with self.subTest(hook=hook.name):
                self.assertIsInstance(hook.matches("/rest/bug"), bool)
                self.assertIsInstance(hook.route("/rest/bug"), str)
                body, count = hook.rewrite("/rest/bug", b'{"bugs":[]}')
                self.assertIsInstance(body, bytes)
                self.assertEqual(count, 0)
```

11. Verify: `python3 tests/functional/redhat-shape-proxy.py --self-test`. Expect every case to
    print `... ok`, a final `OK`, and exit status 0.

12. Controlled fault, per the procedure Task 5 documents. Temporarily **add one character to the
    first prefix** in the `product-ids` hook's `matches` tuple — `"/rest/product_accessible"` →
    `"/rest/product_accessibles"` — re-run the self-test, and expect
    `test_response_hooks_report_product_ids_route` to FAIL with
    `AssertionError: Lists differ: [] != [('product-ids', 'product-ids', 2)]`. Restore the
    character, re-run, expect `OK`. Record both observations for the pull-request body.

    The direction rule matters and is easy to get backwards: **shortening a `startswith` prefix
    widens the match.** `'/rest/product_accessible'.startswith('/rest/product_accessibl')` is
    `True`, so dropping a character leaves the hook firing and the self-test green — a fault that
    proves nothing. Lengthening the prefix past the real path is what stops the match.

12a. Verify no proxy consumer regressed, before committing:
    `unset BZR_BIN && cargo build --release && make functional-test-bz50` (Task 1's target, so
    run Task 1 first). Expect
    `[03-products/production-shaped-product-and-field-metadata] ... PASS` and the
    `18e-release-readiness` proxy test to PASS. The self-test alone proves the registry;
    only an arm proves the three phases that consume it, and discovering a break here costs one
    container instead of unpicking six commits after `functional-test-all`.

13. Add the README section. In `tests/functional/README.md`, immediately before the
    `## Config Isolation` heading, insert:

```markdown
## Production-Shape Proxy (`redhat-shape-proxy.py`)

`tests/functional/redhat-shape-proxy.py` fronts the running container and rewrites successful
responses into shapes real deployments serve, so the compiled CLI is exercised against them
without patching Bugzilla. Accepted ADR 0028 is the governing record. Three phases consume it:
`03-products.sh`, `18d-dependency-analysis.sh`, and `18e-release-readiness.sh`.

### Adding a production-shape rewrite

Each rewrite is a `ResponseHook` in the `RESPONSE_HOOKS` registry, with a `name`, a
`matches(path)` predicate, a `route(path)` label, and a
`rewrite(path, body) -> (bytes, count)` transform. The comment block above `RESPONSE_HOOKS`
states the contract in full. Two rules are worth repeating here:

- The count governs the stderr marker only. `apply_response_hooks` always adopts the body a
  hook returns, so a hook that changed nothing must still return a body it is happy to serve.
- Add self-tests that dispatch a real path through `apply_response_hooks`, not only the
  transform in isolation — a direct-transform test leaves the matcher unguarded.

When a hook changes something the proxy writes `<name> shaped route=<route> count=<n>` to
stderr, which `lib.sh` captures into `$REDHAT_SHAPE_LOG`; that is how a phase proves its rewrite
actually fired. Run the self-tests with
`python3 tests/functional/redhat-shape-proxy.py --self-test`, or through
`make check-proxy-self-test`, which CI runs on every pull request.
```

14. Correct the README prerequisite line. `tests/functional/README.md:11-12` currently reads:

```markdown
- `python3` and `openssl` — only for the ad-hoc TLS phase; when either is
  missing that phase skips cleanly (see the TLS Fixture section below)
```

    Replace those two lines with:

```markdown
- `python3` — the production-shape proxy and the ad-hoc TLS phase; `make check-proxy-self-test`
  needs it too
- `openssl` — only for the ad-hoc TLS phase; when it or `python3` is missing that phase
  skips cleanly (see the TLS Fixture section below)
```

    The existing wording calls `python3` TLS-phase-only, which was already inaccurate — the
    production-shape proxy has always needed it — and this file gains a section below telling
    contributors to run `make check-proxy-self-test`. This is a correction inside a file the
    change already edits, not a new prerequisite: `make lint` is unaffected.

15. Commit: `test(functional): add per-endpoint rewrite hooks to the shape proxy`.

### Acceptance criteria

- `python3 tests/functional/redhat-shape-proxy.py --self-test` exits 0.
- `_forward` contains exactly one rewrite block, calling `apply_response_hooks`.
- The `metadata-sort-keys` marker line is byte-identical to the one at `redhat-shape-proxy.py:195`
  before this change: `f"metadata-sort-keys shaped route={route} count={changed}\n"` with `route`
  in `{"field", "product"}`.
- `git diff` shows no change to `tests/functional/phases/03-products.sh`.

## Task 4 — the enabled non-member user fixture

**Modifies:** `tests/functional/lib.sh`, `tests/functional/phases/07-groups.sh`.
**Tests:** the new `07-groups/fixture-enabled-non-member-user` functional test.

**Interfaces.**

- Consumes: `run_bzr`, `test_fail`, `assert_json`, `$BZR_EXIT`, `$BZR_STDERR` — all defined in
  `tests/functional/lib.sh` (`run_bzr` at `:168`, `test_fail` at `:94`, `assert_json` at `:212`).
- Provides, for issue #625 and any later phase:
  - `NONMEMBER_EMAIL` — string global, `functest-nonmember@test.bzr`.
  - `ensure_enabled_nonmember_user` — no arguments; returns 0 on success, 1 with a diagnostic on
    stderr otherwise. Overwrites the `BZR_*` capture globals.
  - `assert_user_login_enabled <login>` — returns 0 when the server reports `can_login` true for
    that exact login; otherwise calls `test_fail` and returns 1. Overwrites the `BZR_*` globals.
  - `assert_user_group_membership <login> <group> <in|out>` — returns 0 when `<login>`'s own
    `groups` array does (`in`) or does not (`out`) contain `<group>`; otherwise calls `test_fail`
    and returns 1. Overwrites the `BZR_*` globals. This is the helper #625 consumes to assert the
    group filter, so its name and three-argument shape are the interface, not an internal detail.

**Tests, restated:** two new functional tests —
`07-groups/fixture-enabled-non-member-user` and `07-groups/fixture-non-member-is-not-in-the-group`.

### Steps

1. In `tests/functional/lib.sh`, immediately after the `run_bugzilla_sql_file` function (it ends
   with the `"$runtime" exec -i "$container" mysql -u root bugs <"$sql_file"` line and its closing
   brace) and before the `# ── TLS fixture (issue #406) ──` banner, insert:

```bash
# ── Group non-member fixture (issue #617) ────────────────────────────
# A second, *enabled* user who is not a member of the functional test group.
# Asserting that `group list-users --group <g>` honors its filter needs one: an
# added member appears in the listing whether or not the filter is applied, and
# the absence half proves nothing against a user the server would have hidden
# anyway. The login deliberately shares no substring with `testuser`, so the
# existing `user search testuser` and `assert_stdout_not_contains
# "testuser@test.bzr"` assertions cannot match it.
NONMEMBER_EMAIL="functest-nonmember@test.bzr"

# ensure_enabled_nonmember_user — create $NONMEMBER_EMAIL if absent and make
# sure it can log in. Idempotent in both halves: an "already exists" create is
# success, and the enable runs unconditionally so a prior run that disabled the
# user is repaired. The password is explicit because omitting it makes the
# server generate one and mail it, and this harness configures no mail path.
# Overwrites the BZR_* capture globals. Returns non-zero with a diagnostic on
# stderr when either half fails.
#
# This establishes *exists* and *enabled*, not *not a member* — that half holds
# only because nothing adds this login to a group, and containers are reused
# between runs (setup-bugzilla.sh reuses one per checkout+version), so group
# membership survives. A phase that adds $NONMEMBER_EMAIL to a group must
# remove it, or it breaks the next run's non-membership assertion.
ensure_enabled_nonmember_user() {
    run_bzr user create --email "$NONMEMBER_EMAIL" \
        --full-name "Enabled Non-Member" --password "TestPass1!"
    if [[ $BZR_EXIT -ne 0 ]] && ! grep -q "already" "$BZR_STDERR" 2>/dev/null; then
        echo "ensure_enabled_nonmember_user: create failed (exit $BZR_EXIT):" \
            "$(tail -1 "$BZR_STDERR" 2>/dev/null)" >&2
        return 1
    fi
    run_bzr user update "$NONMEMBER_EMAIL" --disable-login false --login-denied-text ""
    if [[ $BZR_EXIT -ne 0 ]]; then
        echo "ensure_enabled_nonmember_user: enable failed (exit $BZR_EXIT):" \
            "$(tail -1 "$BZR_STDERR" 2>/dev/null)" >&2
        return 1
    fi
    return 0
}

# assert_user_login_enabled <login> — fail the current test unless the server
# reports can_login true for exactly <login>. This is the half a dependent
# assertion rests on: a fixture that silently degraded to disabled would make an
# absence assertion pass for the same wrong reason the current one does.
# Overwrites the BZR_* capture globals, so call it before capturing output you
# still need.
assert_user_login_enabled() {
    local login="$1"
    run_bzr user search "$login" --details
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "user search '$login' --details exited $BZR_EXIT"
        return 1
    fi
    assert_json "[.[] | select(.name == \"$login\")][0].can_login" "true"
}

# assert_user_group_membership <login> <group> <in|out> — assert that <login>'s
# own membership set does or does not contain <group>, read from the `groups`
# field `user search --details` already returns (USER_FIELDS_DETAILED,
# src/client/mod.rs:22). This reads the *user* resource, not
# `group list-users`, so it is independent of the group filter #625 owns.
#
# An empty `groups` array would make an `out` assertion pass for the wrong
# reason, so callers pair it with an `in` assertion on a user known to be a
# member: that positive control is what proves the harness can see membership
# at all. Overwrites the BZR_* capture globals.
assert_user_group_membership() {
    local login="$1"
    local group="$2"
    local want="$3"
    local expected=0
    [[ "$want" == "in" ]] && expected=1
    run_bzr user search "$login" --details
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "user search '$login' --details exited $BZR_EXIT"
        return 1
    fi
    assert_json \
        "[[.[] | select(.name == \"$login\")][0].groups[]? | select(.name == \"$group\")] | length" \
        "$expected"
}
```

2. In `tests/functional/phases/07-groups.sh`, immediately after the
   `user-re-enable-for-group-tests` test block (it ends with
   `if assert_success; then test_pass; fi` followed by a blank line) and before
   `test_begin "group-add-user"`, insert:

```bash
# The enabled non-member fixture (issue #617). It exists so #625 can assert that
# `group list-users --group functest-grp` excludes a user the server would
# otherwise return. That assertion is red until #625 lands the group-filter fix,
# so this phase provisions and validates the fixture and stops there.
# Invariant: nothing may leave $NONMEMBER_EMAIL in a group. Containers are
# reused between runs, so a membership added here survives into the next one.
test_begin "fixture-enabled-non-member-user" "fixture enabled non-member user"
if ! ensure_enabled_nonmember_user; then
    test_fail "could not provision the enabled non-member fixture user"
elif assert_user_login_enabled "$NONMEMBER_EMAIL"; then
    test_pass
fi
```

2a. Assert the fixture's defining property, with a positive control. Insert this **immediately
    after the existing `group-add-user` test block** (so `testuser@test.bzr` is a known member).

    Steps 2a and 3 both add text between `group-add-user` and `group-list-users`, so fix the
    order once: `group-add-user` → this new test → step 3's `TODO(#625)` comment →
    `test_begin "group-list-users"`. Anchoring both on "before `group-list-users`" would let the
    marker land above this new test instead of above the assertions it indicts.

```bash
# The fixture's non-membership is what #625's assertion will rest on, so assert
# it rather than trusting that nothing added the user to a group — containers
# are reused across runs, so a stray membership persists indefinitely. The
# testuser half is the positive control: it proves the harness can see
# membership at all, so the nonmember half cannot pass on an empty `groups`.
# Both read the user resource, not `group list-users`, so neither depends on
# the group filter #625 owns.
test_begin "fixture-non-member-is-not-in-the-group" "fixture non-member is not in the group"
if assert_user_group_membership "testuser@test.bzr" functest-grp in &&
    assert_user_group_membership "$NONMEMBER_EMAIL" functest-grp out; then
    test_pass
fi
```

    **Verify this against a live container before relying on it.** Bugzilla returns `groups` on
    `User.get` only to a caller permitted to see them; the harness runs as `admin@test.bzr`, which
    holds `editusers`, so it should be populated.

    If the positive control fails — `testuser` shows an empty `groups` — **diagnose before
    deleting anything**, because two causes produce the identical symptom and only one is a
    harness limitation. Run `run_bzr user search admin@test.bzr --details` and inspect `.groups`:
    a non-empty array on any user proves the field *is* visible to this credential, which means
    `group add-user` never created the membership. That is a product defect, not a harness gap —
    stop and report it. Note that this assertion is the first evidence in the repository that
    `group add-user` does anything: `07-groups.sh:87` asserts only that `testuser` appears in a
    listing, which by this change's own `TODO(#625)` reasoning holds whether or not the group
    filter is honored.

    Only when `.groups` is empty for every user does the field genuinely fail to reach this
    credential. Then delete this test, restore the comment-only invariant, and record in this plan
    and in the spec which arm withheld it and what would prove non-membership instead. Do **not**
    keep the `out` half alone: without the control it passes on an empty array, which is the
    pass-for-the-wrong-reason failure this whole epic exists to remove.

3. In the same file, immediately before `test_begin "group-list-users"` — and **below** the test
   step 2a inserted — insert:

```bash
# TODO(#625): these list-users assertions pass whether or not the group filter is
# honored. An added member appears in an unfiltered listing too, and the absence
# assertion below is only reached after the user is disabled, which hides it from
# user search regardless. #625 owns the `groups=` fix and the replacement
# assertion, which uses the enabled $NONMEMBER_EMAIL fixture above.
```

4. In the same file, extend the existing comment above the re-disable line (currently `:106-107`,
   `# Re-disable testuser so it's excluded from list-users results ...`) by appending one line to
   that comment block:

```bash
# TODO(#625): this re-disable is what makes the absence assertion below pass; it
# is not evidence that `group remove-user` worked.
```

5. Verify the test ID guard: `make check-functional-test-ids`. Expect exit 0. It is not silent —
   it prints three success banners ending `functional test semantic IDs are valid`.

6. Verify shell lint: `make check-shell`. Expect exit 0. It echoes its six recipe lines (none is
   `@`-prefixed); exit status is the signal, not silence.

7. Verify the phase against a live container:
   `unset BZR_BIN && cargo build --release && make functional-test-bz50`. Expect
   `TEST  [07-groups/fixture-enabled-non-member-user] fixture enabled non-member user ... PASS`
   and a final `FAILED: 0`.

8. Controlled fault: in step 1's helper, replace the **whole flag pair**
   `--disable-login false --login-denied-text ""` with
   `--disable-login true --login-denied-text "fault disabled"` — the idiom `06-users.sh:46`
   already uses to disable a user. Re-run `make functional-test-bz50` and expect
   `07-groups/fixture-enabled-non-member-user` to FAIL with
   `jq '[.[] | select(.name == "functest-nonmember@test.bzr")][0].can_login' = 'false', expected 'true'`.
   Restore the pair, confirm with `git diff tests/functional/lib.sh`, re-run, expect PASS. Record
   both observations for the pull-request body.

   Replace the pair, not just the boolean. `resolve_login_denied_text`
   (`src/commands/user/update.rs:95-105`) maps `(Some(true), Some(text))` to that text, and an
   empty `text` is byte-identical to what `(Some(false), _)` sends — so
   `--disable-login true --login-denied-text ""` **re-enables** the user and the fault is inert,
   costing a full container arm to observe PASS where a FAIL was promised. This is the same class
   of trap as Task 3 step 12's `startswith` direction: a fault that does not fault.

9. Commit: `test(functional): add an enabled non-member group fixture user`.

### Acceptance criteria

- `07-groups/fixture-enabled-non-member-user` passes on bz50, bz52, and bz53.
- `07-groups/fixture-non-member-is-not-in-the-group` passes on all three arms, **with its
  positive control passing** — or the test is removed and step 2a's fallback is recorded.
- No existing assertion in `07-groups.sh` changed — only insertions.
- `make check-functional-test-ids` and `make check-shell` are clean.

## Task 5 — the controlled-fault procedure in CONTRIBUTING

**Modifies:** `CONTRIBUTING.md`. **Tests:** none executable; reviewed against the commands it
names, each of which must exist.

**Interfaces.** Consumes the target names Tasks 1 and 2 add (`make functional-test-bz50`,
`make check-proxy-self-test`). Write this task after them so every command it names exists.

### Steps

1. **Do not touch `## Development setup`.** An earlier draft added `python3` to it, to provision a
   `make lint` prerequisite that Task 2 no longer creates. `make lint` gains no new requirement,
   so the setup paragraph stays as it is.

2. In `CONTRIBUTING.md`, immediately after the paragraph ending
   `Do not describe an omitted check as passing.` and before the
   `Documentation-only changes should also confirm...` paragraph, insert everything between the
   four-backtick fences below (the inner three-backtick `bash` block is part of the insert):

````markdown
### Controlled-fault verification

A test that passes both before and after a fix has proved nothing about the fix. When a change
corrects a defect, demonstrate the test goes red against the pre-fix code and green after, and
record both observations in the pull-request body.

1. Write or strengthen the test first.
2. Remove the fix from the working tree — `git stash push` the source paths, or invert the one
   line under test. Do not weaken the test.
3. Run the narrowest command that covers it:
   - a unit test: `make test-one T=<name-substring>`;
   - a production-shape proxy rewrite: `python3 tests/functional/redhat-shape-proxy.py --self-test`,
     or `make check-proxy-self-test`;
   - a single functional arm: `make functional-test-bz50`, `make functional-test-bz52`,
     `make functional-test-bz53`, or `make functional-test` for the unpinned default.
4. Observe the failure. Record the exact command and the failing assertion.
5. Restore the fix, confirm the tree really is restored (`git stash list`, `git status`), re-run
   the same command, and observe green.
6. Put both observations in the pull-request body.

**A functional arm needs a fresh binary and a fresh container.** Two things can make the run
report on something other than your fault:

- **A stale binary.** `tests/functional/phases/00-build.sh:16` uses `$BZR_BIN` verbatim when it is
  set and executable, so an exported `BZR_BIN` runs the whole arm against a binary that never
  received your fault. A *failed* build is not the hazard: `run-tests.sh` runs under
  `set -euo pipefail`, so a non-zero `cargo build` aborts the run rather than falling through to
  a stale artifact.
- **A stale container.** `tests/functional/setup-bugzilla.sh` reuses an already-running container
  for this checkout and version, so users, groups, and bugs from earlier runs persist. Residue
  can satisfy the assertion under test in the faulted state, or fail it in the restored state.

So run the functional arm as one gated chain, before and after removing the fault:

```bash
unset BZR_BIN
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset \
  && cargo build --release \
  && make functional-test-bz50
```

Chain the commands rather than pasting them as separate lines, so a failed reset or build stops
before the arm runs instead of testing the previous state.
````

3. Verify every command the section names exists:
   `make -n test-one T=x`, `make -n check-proxy-self-test`, `make -n functional-test-bz50`,
   `make -n functional-test-bz52`, `make -n functional-test-bz53`, `make -n functional-test`.
   Each must exit 0. (`make -n test-one` without `T=` errors by design; pass `T=x`.)

4. Verify the relative link target still resolves: the section adds no new links, so
   `rg -n '\]\(' CONTRIBUTING.md` should show the same link set as before the edit.

5. Commit: `docs: record the controlled-fault verification procedure`.

### Acceptance criteria

- Every command named in the section exists and `make -n` accepts it.
- The section sits under `## Verification`, per accepted ADR 0021.

## Task 6 — the `TODO(#N)` fixture inventory

**Modifies:** `src/commands/bug/clone_tests.rs`, `src/client/resources/group_tests.rs`,
`src/client/resources/server_tests.rs`, `src/xmlrpc/resources/mappers_tests.rs`,
`tests/functional/phases/02-server-auth.sh`. **Tests:** `make test`, `make lint`.

**Interfaces.** Consumes nothing, provides nothing. Comment lines only: **no asserted value
changes.**

### Steps

1. `src/commands/bug/clone_tests.rs` — above **both** occurrences of
   `                "rep_platform": "x86_64",` (at `:91` and `:329`), insert:

```rust
                // TODO(#621): the server emits this key as `platform`; this
                // fixture mirrors the client's own misconception, which is why
                // the test passes. #621 owns the rename on read and write.
```

   Do not touch `:62` (`rep_platform: None`, a Rust field name) or `:383`
   (`parsed["changes"]["rep_platform"]`, the update payload).

2. `src/client/resources/group_tests.rs` — above each of the four
   `        .and(query_param("group", ...))` lines (`:16`, `:52`, `:87`, `:106`), insert:

```rust
        // TODO(#625): Bugzilla ignores an unrecognized `group` param; #625 switches to `groups`.
```

   That line is 97 characters including its indentation, within the 100-character limit.

3. `src/client/resources/server_tests.rs` — above both
   `            "parameters": {"maxattachmentsize": 1000}` lines (`:93`, `:149`), insert:

```rust
            // TODO(#626): every stock server stringifies /parameters values; #626 owns the fix.
```

4. `src/xmlrpc/resources/mappers_tests.rs` — above
   `    m.insert("dt".into(), Value::DateTime("2024-01-01T00:00:00".into()));` (`:60`), insert:

```rust
    // TODO(#622): XMLRPC.pm strips the dashes (20240101T00:00:00); #622 owns the fix.
```

5. `tests/functional/phases/02-server-auth.sh` — immediately before
   `test_begin "server-capabilities"` (`:15`), insert:

```bash
# TODO(#626): this credentialed assertion never checks max_attachment_size, so a
# permanently null value has always passed here. The credentialless `null`
# assertion further down is correct under accepted ADR 0005 and stays; #626 owns
# adding the non-null credentialed case.
```

6. Verify formatting is untouched by the comments: `cargo fmt --check`. Expect no output, exit 0.
   If rustfmt reflows a comment, accept its output and re-run.

7. Verify no behavior changed: `make test`. Expect the same pass count as before the edit and
   exit 0.

8. Verify the shell edit: `make check-shell` and `make check-functional-test-ids`. Expect exit 0
   from each; both print success banners rather than staying silent.

9. Verify no asserted value moved. Filter on **comment syntax**, not on the marker token — the
   markers are multi-line and only their first line carries `TODO(#`, so a token filter reports
   every continuation line as a violation on correct work:

   ```bash
   git diff -U0 src/ tests/functional/phases/02-server-auth.sh \
     | rg '^[+-]' | rg -v '^[+-][+-]' | rg -v '^[+-]\s*(//|#)'
   ```

   must print nothing — every changed line is a comment. Then confirm the markers are actually
   present: `rg -c 'TODO\(#' src/commands/bug/clone_tests.rs src/client/resources/group_tests.rs
   src/client/resources/server_tests.rs src/xmlrpc/resources/mappers_tests.rs
   tests/functional/phases/02-server-auth.sh` should report 2, 4, 2, 1, and 1.

10. Commit: `test: mark fixtures that encode the conformance defects they hide`.

### Acceptance criteria

- This task lands ten markers: two `#621` (`clone_tests.rs`), four `#625` (`group_tests.rs`),
  two `#626` (`server_tests.rs`), one `#622` (`mappers_tests.rs`), and one `#626`
  (`02-server-auth.sh`). Task 4 lands the two `#625` markers in `07-groups.sh`, bringing the
  change to twelve and covering all six sites issue #617 names.
- `git diff` on `src/` shows only added comment lines.
- `make test` and `make lint` are green.

## Final verification

Run in this order, each bare (no pipe), after all six tasks:

1. `make lint` — expect exit 0. Its prerequisite list is unchanged by this task set.
1a. `make check-proxy-self-test` — expect exit 0, ending `OK`. Run it separately; it is a CI step,
   not a `lint` prerequisite.
2. `make test` — expect exit 0.
3. `unset BZR_BIN && cargo build --release` — expect exit 0.
4. `make functional-test-bz50` — expect the phase-0 banner to read
   `║  bzr functional tests (bz50)` and the suite to reach `FAILED: 0`. **Read the banner**: it
   is the only observable that distinguishes this target from a recipe mis-copied to
   `BZR_BZ_VERSION=bz52`, which would start the 5.2 container, run the identical phase list, and
   also exit 0. An unknown token needs no check — `setup-bugzilla.sh:23-35` rejects it before any
   container work. This is Task 1's only proof: `run-all-versions.sh:20-40` invokes
   `setup-bugzilla.sh` and `run-tests.sh` directly and never shells out to a Make target, so
   step 5 does not traverse the new recipe.
5. `make functional-test-all` — expect `bz50: PASSED`, `bz52: PASSED`, `bz53: PASSED` and exit 0.
6. `make functional-stop-all` — clean up the three containers.

Record the two controlled-fault observations (Task 3 step 12, Task 4 step 8) in the pull-request
body, and disclose there that `.github/workflows/ci.yml` was added to the change surface beyond
the file list issue #617 suggests, with the one-line reason.
