# Implementation plan — functional-harness capabilities (issue #617)

**Goal.** Land the five harness capabilities epic #616's conformance entries depend on, without
correcting a single fixture and without touching a `src/` production path.

**Architecture.** Everything lives in `tests/functional/` plus three project files. The proxy
(`redhat-shape-proxy.py`, python3 stdlib only) gains a rewrite-hook registry; the shell harness
(`lib.sh`, phase scripts) gains a fixture user and one assertion helper; `Makefile`,
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
| `tests/functional/lib.sh` | modified | the non-member fixture global and its two helpers |
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

4. Verify the help entry appears: `make help | grep functional-test-bz50`. Expect one line
   containing `Run functional tests against Bugzilla 5.0`.

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

1. In `Makefile`'s `.PHONY` list, add `check-proxy-self-test` to the line that already carries
   `check-no-spawn check-release-security-notes check-shell`, producing:
   `        check-no-spawn check-release-security-notes check-shell check-proxy-self-test \`

2. Change the `lint` rule from

```make
lint: fmt clippy check-build-script check-test-layout check-functional-test-ids check-no-spawn check-release-security-notes check-shell ## Run all linters
```

   to

```make
lint: fmt clippy check-build-script check-test-layout check-functional-test-ids check-no-spawn check-release-security-notes check-shell check-proxy-self-test ## Run all linters
```

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

6. Verify the workflow still parses: `make check-shell` is not a YAML linter, so instead run
   `python3 -c "import sys; print('ok')"` is **not** a check — use
   `git diff .github/workflows/ci.yml` and confirm the added line sits inside the `test-layout`
   job's `steps:` list, aligned with its siblings.

7. Commit: `ci: run the production-shape proxy self-tests as a guard`.

### Acceptance criteria

- `make check-proxy-self-test` exits 0 and runs every case in `ShapeTests`.
- `make lint` includes it (visible in `make -n lint`, or by the guard's output during a real run).
- `ci.yml`'s `test-layout` job has four `make check-*` steps.

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
   `is_field` / `is_product` locals it needs afterwards. Replace its first eight lines — from
   `    parsed_path = urllib.parse.urlsplit(path).path` through
   `        return data, 0` — with:

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
# matcher unguarded. `make check-proxy-self-test`, a `make lint` prerequisite
# and a ci.yml step, is what enforces that.
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

12. Controlled fault, per the procedure Task 5 documents. Temporarily change the `product-ids`
    hook's `matches` to `lambda path: path.startswith("/rest/product_accessibl")` (one dropped
    character), re-run the self-test, and expect
    `test_response_hooks_report_product_ids_route` to FAIL with
    `AssertionError: [] != [('product-ids', 'product-ids', 2)]`. Restore the character, re-run,
    expect `OK`. Record both observations for the pull-request body.

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
`make check-proxy-self-test`, which `make lint` and CI both run.
```

14. Commit: `test(functional): add per-endpoint rewrite hooks to the shape proxy`.

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

3. In the same file, immediately before `test_begin "group-list-users"` (currently at `:85`),
   insert:

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

5. Verify the test ID guard: `make check-functional-test-ids`. Expect no output and exit 0.

6. Verify shell lint: `make check-shell`. Expect no output and exit 0.

7. Verify the phase against a live container:
   `unset BZR_BIN && cargo build --release && make functional-test-bz50`. Expect
   `TEST  [07-groups/fixture-enabled-non-member-user] fixture enabled non-member user ... PASS`
   and a final `FAILED: 0`.

8. Controlled fault: change step 1's `--disable-login false` to `--disable-login true`, re-run
   `make functional-test-bz50`, and expect that test to FAIL with
   `jq '[.[] | select(.name == "functest-nonmember@test.bzr")][0].can_login' = 'false', expected 'true'`.
   Restore, confirm with `git diff tests/functional/lib.sh`, re-run, expect PASS. Record both
   observations for the pull-request body.

9. Commit: `test(functional): add an enabled non-member group fixture user`.

### Acceptance criteria

- `07-groups/fixture-enabled-non-member-user` passes on bz50, bz52, and bz53.
- No existing assertion in `07-groups.sh` changed — only insertions.
- `make check-functional-test-ids` and `make check-shell` are clean.

## Task 5 — the controlled-fault procedure in CONTRIBUTING

**Modifies:** `CONTRIBUTING.md`. **Tests:** none executable; reviewed against the commands it
names, each of which must exist.

**Interfaces.** Consumes the target names Tasks 1 and 2 add (`make functional-test-bz50`,
`make check-proxy-self-test`). Write this task after them so every command it names exists.

### Steps

1. In `CONTRIBUTING.md`, immediately after the paragraph ending
   `Do not describe an omitted check as passing.` and before the
   `Documentation-only changes should also confirm...` paragraph, insert:

```markdown
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

**Rebuild explicitly before each functional run.** `tests/functional/phases/00-build.sh` uses
`$BZR_BIN` verbatim when it is set and executable, and when it does build it discards `cargo`'s
exit status, checking only that `target/release/bzr` exists. Either path can run the whole arm
against a stale binary that never received your fault, which reports green and tells you your test
does not bite when it was never exercised. So run the functional arm as:

```bash
unset BZR_BIN
cargo build --release   # bare: its exit status is the gate
make functional-test-bz50
```

Run the build unpiped. A pipeline returns the last command's status, so `| tail` hides a failed
build — the same rule the repository's guardrails follow everywhere else.
```

2. Verify every command the section names exists:
   `make -n test-one T=x`, `make -n check-proxy-self-test`, `make -n functional-test-bz50`,
   `make -n functional-test-bz52`, `make -n functional-test-bz53`, `make -n functional-test`.
   Each must exit 0. (`make -n test-one` without `T=` errors by design; pass `T=x`.)

3. Verify the relative link target still resolves: the section adds no new links, so
   `rg -n '\]\(' CONTRIBUTING.md` should show the same link set as before the edit.

4. Commit: `docs: record the controlled-fault verification procedure`.

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

8. Verify the shell edit: `make check-shell` and `make check-functional-test-ids`. Expect no
   output and exit 0.

9. Verify no asserted value moved:
   `git diff -U0 src/ tests/functional/phases/02-server-auth.sh | rg '^[+-]' | rg -v '^[+-][+-]' | rg -v 'TODO\(#'`
   must print nothing — every changed line is a `TODO` comment.

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

1. `make lint` — expect exit 0, including the new `check-proxy-self-test` output ending `OK`.
2. `make test` — expect exit 0.
3. `unset BZR_BIN && cargo build --release` — expect exit 0.
4. `make functional-test-bz50` — expect the bz50 container to start and the suite to reach
   `FAILED: 0`. This is Task 1's only proof: `run-all-versions.sh:20-40` invokes
   `setup-bugzilla.sh` and `run-tests.sh` directly and never shells out to a Make target, so
   step 5 does not traverse the new recipe.
5. `make functional-test-all` — expect `bz50: PASSED`, `bz52: PASSED`, `bz53: PASSED` and exit 0.
6. `make functional-stop-all` — clean up the three containers.

Record the two controlled-fault observations (Task 3 step 12, Task 4 step 8) in the pull-request
body, and disclose there that `.github/workflows/ci.yml` was added to the change surface beyond
the file list issue #617 suggests, with the one-line reason.
