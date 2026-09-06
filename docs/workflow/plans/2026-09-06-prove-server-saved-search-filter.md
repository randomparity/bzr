# Plan: prove the comparison harness's server-side saved search filters

**Goal.** Make `compare/01-bug-lifecycle/saved-search` able to fail for the right reason, by
giving the shaped proxy a Red-Hat saved-search fixture and restating the row's assertions as
a filtered set against an unfiltered control that is asserted to exceed it.

**Architecture.** `tests/functional/redhat-shape-proxy.py` is a host-side forwarding proxy in
front of the Bugzilla container, with response-rewrite hooks in a `REWRITE_HOOKS` tuple of
`(matcher, transformer)` pairs selected by `BZR_FUNC_REDHAT_MODE`. A new `saved-search` mode
advertises the `RedHat` extension and resolves `savedsearch`/`sharer_id`.
`compare/01-bug-lifecycle.sh` runs two bzr calls through it and asserts the control contains
what the filtered call equals. `pybz/container-tests.sh` sources the real phase script against
stubs, and is where the controls are proven without a container.

Expected implementation size: 350–430 changed lines (M) — from the file map below: ~205 in
the proxy (three transformers, hook, wiring, seven `ShapeTests` cases and two test helpers),
~65 in the lifecycle phase, ~32 in `run-compare.sh`, ~81 in the self-test, 2 record lines. The
range moved up from 330–410 because the two review remedies added real code — the
`make_handler` round-trip helper and the seeder fixture — not to move any ratio.

ADR 0061 is the single home for **why**; the spec states the shape. This file is build
instructions only.

## Global Constraints

- **Bash 3.2**, macOS/BSD userland. No `declare -A`, `${var^^}`, or `mapfile`. Harness scripts
  use **4-space indent** and are **not** CI-linted; bare `shfmt` will disagree — match the file.
- **Python standard library only** (`http.server`, `urllib.parse`, `json`, `unittest`,
  `unittest.mock`); the proxy runs as a bare script on the host and in a container.
- **No `src/` change** (ADR 0052 ships separately, #670) and **do not touch
  `docs/adr/README.md`** — the ADR 0061 index row is the orchestrator's.
- **Sibling-owned files are off limits:** `src/client/auth/*`, `src/client/version.rs`,
  `src/client/resources/{comment,attachment,bug}.rs`, `src/client/mod.rs`, `src/cli/mod.rs`,
  `docs/bzr-cli.md`, phase scripts `02`, `08`, `08e`, `09c`, `15b`, `16b` (#713, #714, #719).
  Report anything adjacent; do not fix it.
- **Guardrails** run bare — no `| tail`, no `>/dev/null`, no `|| true`: `make lint`,
  `make test` (never bare `cargo test`), and `make functional-compare-all`, the only gate that
  reaches this change.
- Branch `feat/prove-saved-search-filter-710`, base `main`, worktree `../bzr-worktrees/quest-710`.
- Names that must match exactly: `BZR_FUNC_SAVED_SEARCH_{NAME,IDS,SHARER}`; extension `RedHat`
  (`src/commands/runtime/shared/capability.rs:21`) with value `{"version": "1.0"}`, matching
  `ExtensionInfo { version: Option<String> }` (`src/types/server_info.rs:27`).

## File map

| File | Change | Answerable for |
|---|---|---|
| `tests/functional/redhat-shape-proxy.py` | modify | the `saved-search` mode and its seven `ShapeTests` cases |
| `tests/functional/run-compare.sh` | modify | `seed_server_saved_search` accepting a variable number of bug ids |
| `tests/functional/compare/01-bug-lifecycle.sh` | modify | the row's assertions, the new id helper, the proxied-call wrapper |
| `tests/functional/pybz/container-tests.sh` | modify | proxy stubs, three failure controls, the seeder fixture, count and row updates |
| `docs/dev/python-bugzilla-parity.md` | modify | the `Server saved search` row text |

Existing names this plan borrows, each confirmed present with the assumed signature:

- `_hook_matches(prefixes=(), *, method=None, mode=None)` — `redhat-shape-proxy.py:377`
- `REWRITE_HOOKS` — `:460`; `apply_rewrite_hooks(method, path, body, enabled_modes)` — `:478`
- `emit_rewrite_evidence` over `(marker, route, count)` — `:489`; `make_handler` — `:540`
- `ShapeTests._start_server(backend_port)` (staticmethod, returns `(server, thread)`) — `:1178`
- `redhat_shape_start <backend_port>` / `redhat_shape_stop`, setting `REDHAT_SHAPE_PORT` —
  `lib.sh:1438`, `:1460`
- `lifecycle_bzr_probe` `:57`, `lifecycle_bzr` `:98`, `lifecycle_bzr_refusal_gap` `:113`,
  `lifecycle_expect_gap` `:162`, `lifecycle_pybz` `:171`, `lifecycle_ids_are` `:208`,
  `lifecycle_transport_is` `:228` — all `compare/01-bug-lifecycle.sh`
- `expect_gap` converting FAIL→GAP and PASS→FAIL — `lib.sh:209`
- `run_lifecycle_failure_control <flag> <capability> <label> [value]` — `container-tests.sh:371`
- `run_gap_ineligible_control <flag> <capability> <label>` — `:406`
- `assert_equals <expected> <actual> <label>` — `:11`; `PYBZ_DIR` — `:7`
- counts 7/0/3 at `:831-833`; `stale gap fail count` 3 at `:979`; parity fixture row at `:1001`

## Task 1 — the proxy's `saved-search` mode

Modifies `tests/functional/redhat-shape-proxy.py`. Every later task depends on this fixture.

**Interfaces produced.** `RED_HAT_EXTENSION`; `_saved_search_fixture()`;
`shape_saved_search_extensions(path, data)`; `shape_saved_search_response(path, data, fixture)`;
`_run_saved_search_hook(method, path, body)`; the `saved-search` mode string.

`main()` dispatches on exactly `--self-test` (`:1234`), loading `ShapeTests` via
`unittest.defaultTestLoader`. The file runs **44** tests today. It does `import unittest` at
line 21 but **not** `import unittest.mock`, and `import unittest` alone does not bind the
`mock` submodule — Step 1.1 adds it or the new cases raise `AttributeError`.

### Verification

Every contract below is `Mode: focused-test`, proven by one `ShapeTests` case, with the same
green command: `python3 tests/functional/redhat-shape-proxy.py --self-test`. Step 1.8 carries
each case's code; the expected red for each is the case's own assertion failing on an
unfiltered `[1, 2, 3]` body.

| Contract | Case |
|---|---|
| Advertises `RedHat` at `/rest/extensions` | `test_advertises_red_hat_extension` |
| `savedsearch` matching the fixture filters to its ids | `test_saved_search_filters_to_fixture_ids` |
| An unknown `savedsearch` resolves to no bugs | `test_unknown_saved_search_resolves_empty` |
| `sharer_id` qualifies resolution both ways | `test_sharer_id_qualifies_resolution` |
| No `savedsearch` parameter leaves the response alone | `test_absent_saved_search_is_untouched` |
| A malformed ids list disables the fixture | `test_malformed_fixture_ids_disable_filtering` |
| The hook is reached only when the mode is enabled | `test_mode_gates_the_saved_search_hook` |

The last one is the only case that goes through `apply_rewrite_hooks` **and** `make_handler`,
which is what makes it cover Steps 1.6 and 1.7 rather than the transformer alone. It was
verified by construction: applying Steps 1.1–1.8 to a scratch copy gives `Ran 51 tests ... OK`;
deleting only Step 1.7's two edits gives `FAILED (failures=1)` with
`AssertionError: Lists differ: [1, 2, 3] != [1]`; deleting only Step 1.6's registry line gives
the same failure. Re-run those three checks if you change the case.

**Step 1.1.** Add `import unittest.mock` after `import unittest`, and the constant beside
`_MAX_REQUEST_BODY`:

```python
RED_HAT_EXTENSION = "RedHat"
```

**Step 1.2.** Add the fixture reader before `shape_bug_response`:

```python
def _saved_search_fixture():
    """Return (name, ids, sharer) for the saved-search fixture, or None when unusable.

    A malformed value disables the fixture instead of being coerced: filtering to
    an empty set on a typo would turn a harness mistake into a plausible-looking
    result, which is the exact failure this fixture exists to make visible.
    """
    name = os.environ.get("BZR_FUNC_SAVED_SEARCH_NAME", "")
    raw_ids = os.environ.get("BZR_FUNC_SAVED_SEARCH_IDS", "")
    sharer = os.environ.get("BZR_FUNC_SAVED_SEARCH_SHARER", "")
    if not name or not raw_ids:
        return None
    fields = raw_ids.split(",")
    if not all(field.isdigit() and int(field) > 0 for field in fields):
        return None
    if sharer and not (sharer.isdigit() and int(sharer) > 0):
        return None
    return name, {int(field) for field in fields}, sharer
```

**Step 1.3.** Add the extension transformer after it:

```python
def shape_saved_search_extensions(path, data):
    """Return JSON bytes advertising the Red Hat extension at /rest/extensions."""
    if urllib.parse.urlsplit(path).path != "/rest/extensions":
        return data, {}
    value = json.loads(data)
    extensions = value.get("extensions") if isinstance(value, dict) else None
    if not isinstance(extensions, dict) or RED_HAT_EXTENSION in extensions:
        return data, {}
    extensions[RED_HAT_EXTENSION] = {"version": "1.0"}
    return json.dumps(value, separators=(",", ":")).encode(), {"extensions": 1}
```

**Step 1.4.** Add the search transformer after it:

```python
def shape_saved_search_response(path, data, fixture):
    """Return JSON bytes with `bugs` resolved as a Red Hat saved search would.

    Filtering only when the parameter is present is what makes a client that
    stops sending it fail.
    """
    parsed = urllib.parse.urlsplit(path)
    if parsed.path != "/rest/bug":
        return data, {}
    query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True)
    requested = query.get("savedsearch")
    if not requested:
        return data, {}

    name, ids, sharer = fixture
    sharer_values = query.get("sharer_id") or []
    resolved = requested == [name] and (
        not sharer_values or (bool(sharer) and sharer_values == [sharer])
    )

    value = json.loads(data)
    bugs = value.get("bugs") if isinstance(value, dict) else None
    if not isinstance(bugs, list):
        return data, {}
    kept = [
        bug for bug in bugs
        if resolved and isinstance(bug, dict) and bug.get("id") in ids
    ]
    value["bugs"] = kept
    return json.dumps(value, separators=(",", ":")).encode(), {"bug-search": len(kept)}
```

**Step 1.5.** Add the hook runner beside the other `_run_*_hook` functions:

```python
def _run_saved_search_hook(_method, path, body):
    body, evidence = shape_saved_search_extensions(path, body)
    fixture = _saved_search_fixture()
    if fixture is not None:
        body, search_evidence = shape_saved_search_response(path, body, fixture)
        evidence = {**evidence, **search_evidence}
    return body, [
        ("saved-search", route, count) for route, count in evidence.items()
    ]
```

**Step 1.6.** Register it in `REWRITE_HOOKS`, on the line after the `_run_server_hook` entry:

```python
    (_hook_matches(mode="saved-search"), _run_saved_search_hook),
```

**Step 1.7.** In `make_handler`, add the mode flag after `bearer_auth_mode`:

```python
    saved_search_mode = os.environ.get("BZR_FUNC_REDHAT_MODE") == "saved-search"
```

and its entry in the enabled-modes set built in `_forward`, after `("cc-objects", …)`:

```python
                            ("saved-search", saved_search_mode),
```

**Step 1.8.** Add two helpers and the seven cases to `ShapeTests`, immediately before the
existing `_start_server` staticmethod:

```python
    _SAVED_SEARCH_BODY = json.dumps(
        {"bugs": [{"id": 1}, {"id": 2}, {"id": 3}]}, separators=(",", ":")
    ).encode()

    def _resolve(self, path, **environment):
        base = {
            "BZR_FUNC_SAVED_SEARCH_NAME": "owned-search",
            "BZR_FUNC_SAVED_SEARCH_IDS": "1",
            "BZR_FUNC_SAVED_SEARCH_SHARER": "7",
        }
        base.update(environment)
        with unittest.mock.patch.dict(os.environ, base, clear=False):
            body, _ = _run_saved_search_hook("GET", path, self._SAVED_SEARCH_BODY)
        return [bug["id"] for bug in json.loads(body)["bugs"]]

    def _saved_search_round_trip(self, path, mode="saved-search"):
        """Drive one request through a real handler, so make_handler's wiring counts."""
        body = self._SAVED_SEARCH_BODY

        class BackendHandler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        backend = http.server.ThreadingHTTPServer(("127.0.0.1", 0), BackendHandler)
        backend_thread = threading.Thread(target=backend.serve_forever, daemon=True)
        backend_thread.start()
        environment = {
            "BZR_FUNC_SAVED_SEARCH_NAME": "owned-search",
            "BZR_FUNC_SAVED_SEARCH_IDS": "1",
        }
        if mode is not None:
            environment["BZR_FUNC_REDHAT_MODE"] = mode
        try:
            # The whole round trip stays inside the patched environment. The mode is
            # read once by make_handler at construction, but the fixture variables are
            # read per request by _saved_search_fixture, so restoring the environment
            # before the request would silently disable the filter and the case would
            # pass for the wrong reason.
            with unittest.mock.patch.dict(os.environ, environment, clear=False):
                if mode is None:
                    os.environ.pop("BZR_FUNC_REDHAT_MODE", None)
                proxy, proxy_thread = self._start_server(backend.server_port)
                try:
                    with contextlib.redirect_stderr(io.StringIO()):
                        connection = http.client.HTTPConnection(
                            "127.0.0.1", proxy.server_port, timeout=2
                        )
                        connection.request("GET", path)
                        payload = json.loads(connection.getresponse().read())
                        connection.close()
                finally:
                    proxy.shutdown()
                    proxy.server_close()
                    proxy_thread.join(timeout=2)
        finally:
            backend.shutdown()
            backend.server_close()
            backend_thread.join(timeout=2)
        return [bug["id"] for bug in payload["bugs"]]

    def test_saved_search_filters_to_fixture_ids(self):
        self.assertEqual(self._resolve("/rest/bug?savedsearch=owned-search"), [1])

    def test_unknown_saved_search_resolves_empty(self):
        self.assertEqual(self._resolve("/rest/bug?savedsearch=other"), [])

    def test_sharer_id_qualifies_resolution(self):
        self.assertEqual(
            self._resolve("/rest/bug?savedsearch=owned-search&sharer_id=7"), [1]
        )
        self.assertEqual(
            self._resolve("/rest/bug?savedsearch=owned-search&sharer_id=8"), []
        )

    def test_absent_saved_search_is_untouched(self):
        self.assertEqual(self._resolve("/rest/bug?summary=x"), [1, 2, 3])

    def test_malformed_fixture_ids_disable_filtering(self):
        self.assertEqual(
            self._resolve(
                "/rest/bug?savedsearch=owned-search", BZR_FUNC_SAVED_SEARCH_IDS="1,x"
            ),
            [1, 2, 3],
        )

    def test_advertises_red_hat_extension(self):
        body, _ = _run_saved_search_hook(
            "GET", "/rest/extensions", b'{"extensions":{}}'
        )
        self.assertEqual(
            json.loads(body)["extensions"]["RedHat"], {"version": "1.0"}
        )

    def test_mode_gates_the_saved_search_hook(self):
        path = "/rest/bug?savedsearch=owned-search"
        self.assertEqual(self._saved_search_round_trip(path), [1])
        self.assertEqual(self._saved_search_round_trip(path, mode=None), [1, 2, 3])
```

**Step 1.9.** Run bare:

```bash
python3 tests/functional/redhat-shape-proxy.py --self-test
```

Expect `Ran 51 tests` and `OK` — 44 existing plus 7.

**Acceptance criteria.** 51 tests pass. Deleting either Step 1.6 or Step 1.7 reddens
`test_mode_gates_the_saved_search_hook`.

## Task 2 — seed a strict subset

Modifies `tests/functional/run-compare.sh`. The seeded query must select fewer bugs than an
unfiltered search returns, or the row's two sets cannot stand in a superset relation.

**Interfaces produced.** `seed_server_saved_search LOGIN NAME BUG_ID [BUG_ID...]`.

### Verification

- Contract: the seeder accepts one or more ids, rejects zero ids and a non-decimal id, and
  builds the `bug_id=` list from all of them. Mode: focused-test. Test:
  `run_saved_search_seed_fixture` (Task 4, Step 4.0), which extracts and sources the **real**
  function and drives it with stubbed container helpers. Expected red: with the old two-id
  signature, the one-id call returns 2 and the fixture reports
  `expected seed accepts a single ID to be 0, got 2`. Green:
  `bash tests/functional/pybz/container-tests.sh`.

Nothing compares the real function to the lifecycle fixture's hand-written stub today —
`rg -n 'seed_server_saved_search' tests/` returns only the definition (`run-compare.sh:34`),
that stub (`container-tests.sh:337`) and the single call site (`01-bug-lifecycle.sh:475`).

**Step 2.1.** Replace the argument guard and query construction:

```bash
seed_server_saved_search() {
    if [[ $# -lt 3 ]]; then
        printf 'seed_server_saved_search: expected LOGIN NAME BUG_ID [BUG_ID...]\n' >&2
        return 2
    fi

    local login="$1" name="$2"
    shift 2
    local runtime container helper query id ids=""
    if [[ -z $login || -z $name || ${#name} -gt 64 || $name == *$'\n'* ]]; then
        printf 'seed_server_saved_search: invalid login or name\n' >&2
        return 2
    fi
    for id in "$@"; do
        if [[ ! $id =~ ^[1-9][0-9]*$ ]]; then
            printf 'seed_server_saved_search: invalid bug ID\n' >&2
            return 2
        fi
        ids="${ids:+$ids,}$id"
    done
    runtime=$(container_runtime) || return 1
    container=$(bugzilla_container_name) || return 1
    helper="$SCRIPT_DIR/compare/seed-saved-search.pl"
    query="bug_id=${ids}&bug_id_type=anyexact"
    if [[ ! -r $helper ]]; then
        printf 'seed_server_saved_search: helper is unreadable\n' >&2
        return 1
    fi
    "$runtime" exec -i --workdir /var/www/html/bugzilla "$container" perl -I. - \
        "$login" "$name" "$query" <"$helper"
}
```

`seed-saved-search.pl` is unchanged: it already takes `LOGIN NAME QUERY`.

**Acceptance criteria.** `bash -n tests/functional/run-compare.sh` clean; the function builds
`bug_id=41&bug_id_type=anyexact` for one id and `bug_id=41,42&bug_id_type=anyexact` for two.

## Task 3 — the discriminating row

Modifies `tests/functional/compare/01-bug-lifecycle.sh`.

**Interfaces produced.** `lifecycle_ids_contain <file> <json_id_array>`;
`lifecycle_saved_search_probe <name> <args...>`; the `LIFECYCLE_BZR_URL` override read by
`lifecycle_bzr_probe`. Task 4 stubs `redhat_shape_start` / `redhat_shape_stop` and keys its
controls on the capture names `saved-search-control` and `saved-search-filtered`.

### Verification

One control per assertion — each must bite through *itself*, not through a neighbour that
short-circuits ahead of it in the `&&` chain. All three are `Mode: focused-test`, live in
Task 4, and are green under `bash tests/functional/pybz/container-tests.sh`.

| Contract | Control | Expected red before this task |
|---|---|---|
| The filtered call equals the seeded subset, so an ignoring server fails | `LIFECYCLE_SAVED_SEARCH_UNFILTERED` | flag has no effect; `saved-search control … unexpectedly passed` |
| The control contains both bugs, so the comparison is not vacuous | `LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW` | same message; a control equal to the subset passes, which is the defect being fixed |
| python-bugzilla's result contains an id the seeded query excludes | `LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED` | same message |

**Step 3.1.** Let a probe target another URL. In `lifecycle_bzr_probe`, replace the
`--server-url "$BZ_URL"` argument:

```bash
    RUST_LOG=bzr=debug run_bzr --server-url "${LIFECYCLE_BZR_URL:-$BZ_URL}" \
        --server-api-key-env BZR_COMPARE_API_KEY --server-email "$COMPARE_ADMIN_EMAIL" \
        --api "$api" "$@"
```

Nothing else sets `LIFECYCLE_BZR_URL`, so every existing caller is unaffected.

**Step 3.2.** Add the id helper immediately after `lifecycle_ids_are`:

```bash
# `lifecycle_ids_are` is exact-equality, which the saved-search row cannot use for
# either set it does not fully control: the unfiltered control is every bug whose
# summary carries the run stem, and python-bugzilla's unfiltered result is the whole
# database. Both need containment instead.
lifecycle_ids_contain() {
    local source="$1" expected="$2"

    if ! jq -e 'type == "array" and all(.[]; .id | type == "number")' \
        "$source" >/dev/null; then
        test_fail "ID evidence had an invalid structure"
        return 1
    fi
    jq -e --argjson expected "$expected" \
        '[.[].id] as $ids | all($expected[]; . as $id | $ids | index($id) != null)' \
        "$source" >/dev/null
}
```

**Step 3.3.** Add the proxied-probe wrapper after `lifecycle_bzr_refusal_gap`:

```bash
# Run one bzr probe against the Red-Hat-shaped proxy rather than the container, so
# ADR-0052's capability gate passes and `savedsearch` actually resolves. Only bzr is
# routed here; ADR-0061 records why the python-bugzilla arm stays on the container.
lifecycle_saved_search_probe() {
    local name="$1"
    shift
    local status=0

    BZR_FUNC_REDHAT_MODE=saved-search \
        BZR_FUNC_SAVED_SEARCH_NAME="$LIFECYCLE_SAVED_SEARCH" \
        BZR_FUNC_SAVED_SEARCH_IDS="$LIFECYCLE_BZR_ID" \
        redhat_shape_start "$BZ_PORT" || return 1
    LIFECYCLE_BZR_URL="http://127.0.0.1:${REDHAT_SHAPE_PORT}"
    lifecycle_bzr "$name" "$@" || status=1
    LIFECYCLE_BZR_URL=""
    redhat_shape_stop || status=1
    return "$status"
}
```

`redhat_shape_start` inherits its environment from this shell, which is how
`BZR_FUNC_REDHAT_MODE` already reaches it from `06-auth-config-tls.sh`.
`BZR_FUNC_SAVED_SEARCH_SHARER` is deliberately unset: the row passes no `--sharer`, and the
fixture treats an absent `sharer_id` as unqualified. `sharer_id` is proven in Task 1 instead,
where both legs run without seeding a second Bugzilla account.

**Step 3.4.** Replace the whole `test_begin "saved-search"` block with:

```bash
test_begin "saved-search" "server saved search"
if [[ -n $LIFECYCLE_BZR_ID && -n $LIFECYCLE_PYBZ_ID ]] &&
    seed_server_saved_search "$COMPARE_ADMIN_EMAIL" "$LIFECYCLE_SAVED_SEARCH" \
        "$LIFECYCLE_BZR_ID" &&
    lifecycle_saved_search_probe saved-search-control bug list --summary "$LIFECYCLE_STEM" &&
    lifecycle_ids_contain "$COMPARE_EXCHANGE_DIR/saved-search-control.bzr.stdout.json" \
        "[$LIFECYCLE_BZR_ID,$LIFECYCLE_PYBZ_ID]" &&
    lifecycle_saved_search_probe saved-search-filtered \
        bug search --saved-search "$LIFECYCLE_SAVED_SEARCH" &&
    lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/saved-search-filtered.bzr.stdout.json" \
        "[$LIFECYCLE_BZR_ID]" &&
    lifecycle_pybz saved-search saved_search "$(jq -cn --arg name "$LIFECYCLE_SAVED_SEARCH" \
        '{name:$name}')" &&
    lifecycle_transport_is saved-search pybz XMLRPC &&
    lifecycle_ids_contain "$COMPARE_EXCHANGE_DIR/saved-search.pybz.result.json" \
        "[$LIFECYCLE_PYBZ_ID]"; then
    if lifecycle_bzr_refusal_gap saved-search '"type":"unsupported_server_capability"' 15 \
        bug search --saved-search "$LIFECYCLE_SAVED_SEARCH" &&
        lifecycle_ids_are "$COMPARE_EXCHANGE_DIR/saved-search.bzr.stdout.json" \
            "[$LIFECYCLE_BZR_ID]"; then
        test_pass
    elif [[ $LAST_TEST_RESULT != FAIL ]]; then
        test_fail "bzr saved-search result differed"
    fi
    lifecycle_expect_gap 670
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "saved-search precondition failed"
fi
```

Four properties of this shape must not be rearranged:

- Every new assertion sits in the **precondition** chain, so a failure produces an outright
  FAIL that never reaches `lifecycle_expect_gap 670` — the shape the eight existing
  `run_gap_ineligible_control` entries already assert.
- The **stock refusal is the last bzr probe**, so the gap eligibility `lifecycle_expect_gap`
  reads is the one the refusal set; every `lifecycle_bzr_probe` begins with
  `lifecycle_gap_reset`.
- The inner `lifecycle_ids_are` expects `[$LIFECYCLE_BZR_ID]`, the seeded subset, because
  that is what a bzr which stopped refusing would return for the gap to be genuinely stale.
- **Control uses containment, filtered uses equality, and they are not interchangeable.** The
  control set is every stem-bearing bug in the run and is not fixed by construction —
  `01-bug-lifecycle.sh` already creates `"$LIFECYCLE_STEM generic bzr"` and
  `"$LIFECYCLE_STEM generic pybz"` in the `arbitrary-fields` row, and only that row's
  position *after* this one keeps the count at two. Do **not** add a third assertion that
  the two sets differ; ADR 0061 decision part 2 records why it could never fail.

**Acceptance criteria.** `bash -n` clean. Against a live container the row reports
`GAP (#670)`; `saved-search-filtered.bzr.stdout.json` holds exactly the bzr id and
`saved-search-control.bzr.stdout.json` holds at least the bzr and pybz ids.

## Task 4 — the self-test keeps step and gains three controls

Modifies `tests/functional/pybz/container-tests.sh`.

**Interfaces produced.** `run_saved_search_seed_fixture`; controls
`LIFECYCLE_SAVED_SEARCH_UNFILTERED`, `LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW`,
`LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED`; stubs `redhat_shape_start` / `redhat_shape_stop`.

### Verification

The three controls are the Task 3 table above. One further contract belongs here:

- Contract: the default scenario's PASS/FAIL/GAP counts and the stale-gap scenario are
  unchanged by the new assertions. Mode: focused-test. Test: the existing
  `assert_equals 7 "$PASS_COUNT"` / `0 "$FAIL_COUNT"` / `3 "$GAP_COUNT"` and
  `assert_equals 3 "$FAIL_COUNT" "stale gap fail count"`. Expected red: a count assertion
  reports the new value. Green: `bash tests/functional/pybz/container-tests.sh`.

**Step 4.0.** Add a fixture exercising the **real** seeder, beside the other `run_*_fixture`
definitions, and add `run_saved_search_seed_fixture` to the call list at the bottom of the
file immediately before `run_lifecycle_phase_fixture`:

```bash
run_saved_search_seed_fixture() (
    # Source the real function out of run-compare.sh rather than restating it: a
    # second hand-written copy is what let the arity change go untested before.
    local source_file="$PYBZ_DIR/../run-compare.sh" extracted status
    extracted=$(mktemp)
    trap 'rm -f "$extracted"' EXIT
    awk '/^seed_server_saved_search\(\) \{$/,/^\}$/' "$source_file" >"$extracted"
    if [[ ! -s $extracted ]]; then
        printf 'could not extract seed_server_saved_search from run-compare.sh\n' >&2
        return 1
    fi
    # shellcheck disable=SC1090 # the extracted function text is generated above.
    source "$extracted"

    SCRIPT_DIR=$(cd "$PYBZ_DIR/.." && pwd)
    container_runtime() { printf 'echo\n'; }
    bugzilla_container_name() { printf 'fixture-container\n'; }

    set +e
    seed_server_saved_search admin@test.bzr name >/dev/null 2>&1
    status=$?
    set -e
    assert_equals 2 "$status" "seed rejects a missing bug ID"

    set +e
    seed_server_saved_search admin@test.bzr name 1 x >/dev/null 2>&1
    status=$?
    set -e
    assert_equals 2 "$status" "seed rejects a non-decimal bug ID"

    assert_equals 'bug_id=41&bug_id_type=anyexact' \
        "$(seed_server_saved_search admin@test.bzr fixture-name 41 | awk '{print $NF}')" \
        "seed builds a single-ID query"
    assert_equals 'bug_id=41,42&bug_id_type=anyexact' \
        "$(seed_server_saved_search admin@test.bzr fixture-name 41 42 | awk '{print $NF}')" \
        "seed builds a multi-ID query"
)
```

`container_runtime` returns `echo`, so the function's last line prints its own argv and the
query is the final whitespace-separated field; `perl` never runs and no container is touched.
`SCRIPT_DIR` is set because the `<"$helper"` redirect still needs
`tests/functional/compare/seed-saved-search.pl` to be readable. The body is wrapped in
`( … )` so the overrides do not leak into later fixtures.

**Step 4.1.** Update the `seed_server_saved_search` stub for the new arity. It stays a stub
because the lifecycle fixture must not touch a container; Step 4.0 is what holds it honest:

```bash
    seed_server_saved_search() {
        [[ $1 == admin@test.bzr && -n $2 && ${#2} -le 64 && $# -ge 3 ]] || return 1
        local id
        shift 2
        for id in "$@"; do
            [[ $id =~ ^[1-9][0-9]*$ ]] || return 1
        done
    }
```

**Step 4.2.** Add proxy-lifecycle stubs beside it, and `BZ_PORT=1` to the fixture's variable
block beside `BZ_URL=http://127.0.0.1` so the wrapper's argument is non-empty:

```bash
    redhat_shape_start() {
        [[ $# -eq 1 && ${BZR_FUNC_REDHAT_MODE:-} == saved-search &&
            -n ${BZR_FUNC_SAVED_SEARCH_NAME:-} && -n ${BZR_FUNC_SAVED_SEARCH_IDS:-} ]] ||
            return 1
        REDHAT_SHAPE_PORT=59999
    }
    redhat_shape_stop() { REDHAT_SHAPE_PORT=""; }
```

**Step 4.3.** Teach the stubbed `run_bzr` about the two proxied calls. Insert immediately
before the existing `--saved-search` refusal branch:

```bash
        # The proxied control and filtered calls (ADR-0061).
        # LIFECYCLE_SAVED_SEARCH_UNFILTERED models a proxy or client that stopped
        # applying the filter; LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW models a control
        # that no longer exceeds the seeded subset, making the comparison vacuous.
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search-control ]]; then
            if [[ ${LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW:-0} -eq 1 ]]; then
                printf '[{"id":41}]\n' >"$BZR_STDOUT"
            else
                printf '[{"id":41},{"id":42}]\n' >"$BZR_STDOUT"
            fi
            fixture_finish_bzr 0
            return 0
        fi
        if [[ ${LIFECYCLE_BZR_CALL_NAME:-} == saved-search-filtered ]]; then
            if [[ ${LIFECYCLE_SAVED_SEARCH_UNFILTERED:-0} -eq 1 ]]; then
                printf '[{"id":41},{"id":42}]\n' >"$BZR_STDOUT"
            else
                printf '[{"id":41}]\n' >"$BZR_STDOUT"
            fi
            fixture_finish_bzr 0
            return 0
        fi
```

**Step 4.4.** Make the pybz stub model an ignoring server. Replace the `saved_search)` arm in
`fake_lifecycle_runtime`:

```bash
        saved_search)
            # An ignoring server returns the unfiltered set, which includes the
            # pybz bug the seeded query excludes.
            result='[{"id":41},{"id":42}]'
            [[ ${LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED:-0} -eq 0 ]] || result='[{"id":41}]'
            ;;
```

**Step 4.5.** Update the stale-gap arm so a bzr that stopped refusing returns the seeded
subset — otherwise that scenario fails on the id assertion instead of reporting the gap
stale. Replace the `--saved-search` case:

```bash
            *" --saved-search "*) printf '[{"id":41}]\n' >"$BZR_STDOUT" ;;
```

**Step 4.6.** Add the three controls beside the other `run_lifecycle_failure_control` calls.
All three fail in the precondition chain, so the outcome is an outright FAIL —
`run_lifecycle_failure_control`, not `run_gap_ineligible_control`:

```bash
    for control in LIFECYCLE_SAVED_SEARCH_UNFILTERED \
        LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW \
        LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED; do
        if ! run_lifecycle_failure_control "$control" saved-search 'server saved search'; then
            control_failures=$((control_failures + 1))
        fi
    done
```

**Step 4.7.** Run bare:

```bash
bash tests/functional/pybz/container-tests.sh
```

This is exactly what `make functional-compare-all` runs first (`Makefile:231`). Later
fixtures need a container runtime, so run it where Docker or podman is available. Expect the
counts above to hold, three new `controlled red: saved-search …` lines, and Step 4.0's four
assertions.

**Step 4.8.** If a count assertion reports a different value, do not adjust the expected
number. Read the fixture output and establish which assertion changed the row's outcome; a
changed count means the row's shape changed, which this plan does not intend.

**Acceptance criteria.** Three `controlled red` lines; `run_saved_search_seed_fixture`
passes; counts unchanged at 7/0/3; the stale-gap scenario still reports `#670 appears
resolved` with `stale gap fail count` 3.

## Task 5 — the parity record and its fixture copy

Modifies `docs/dev/python-bugzilla-parity.md` and `tests/functional/pybz/container-tests.sh`.
`run_parity_report_fixture` greps the document for each fixture literal with
`grep -Fxc "$row"` requiring exactly one whole-line match (`container-tests.sh:1037`), so a
fixture row missing from the document fails; the reverse is not caught, so both edits are in
one task.

### Verification

- Contract: the document row and the fixture row are byte-identical. Mode: focused-test.
  Test: `run_parity_report_fixture`. Expected red: change one of the two and the fixture
  reports `missing or duplicate parity report row`. Green:
  `bash tests/functional/pybz/container-tests.sh`.

**Step 5.1.** Replace the `Server saved search` row in `docs/dev/python-bugzilla-parity.md`:

```
| Server saved search | `bzr bug search --saved-search` | stock: bzr errors, python-bugzilla returns unfiltered results (#670); Red-Hat-shaped proxy: bzr filters | `compare/01-bug-lifecycle/saved-search` |
```

**Step 5.2.** Replace the identical literal in `run_parity_report_fixture`, keeping the
surrounding single quotes:

```bash
        '| Server saved search | `bzr bug search --saved-search` | stock: bzr errors, python-bugzilla returns unfiltered results (#670); Red-Hat-shaped proxy: bzr filters | `compare/01-bug-lifecycle/saved-search` |'
```

**Acceptance criteria.** `run_parity_report_fixture` passes — that is the check, and it runs
first in `make functional-compare-all`. Do not add a separate `diff`; it would duplicate it.

## Task 6 — full-tier verification

The only gate that reaches this change, and an acceptance criterion of the issue.

### Verification

- Contract: the row is green on all three supported images with real containers. Mode:
  focused-test. Test: `make functional-compare-all`. Expected red before Tasks 1–5: the row
  cannot reach its filtered assertion at all. Green: the command below.

**Step 6.1.** `cargo build --release` — the tier runs the release binary.

**Step 6.2.** Run `make functional-compare-all` bare, in the background (it exceeds the
foreground timeout). Expect `FAILED: 0` for `bz50`, `bz52` and `bz53`, and
`[compare/01-bug-lifecycle/saved-search] server saved search ... GAP (#670)` in each.

**Step 6.3.** In the completion report, record that the row was green **and** that
`LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW` reddened it in the self-test. That pair rules out the
vacuous oracle; a green row alone does not say the control exceeded the filtered result. The
exchange directory is removed on exit, so the in-run assertions are the evidence.

**Step 6.4.** Run `make lint` and `make test` bare. Neither reaches this change; they run
because the branch must not regress them.

**Acceptance criteria.** All three green.

## Deferrals

None. Both design-review cycles dispositioned every finding `accepted-fixed`; no deferral
record or tracker issue is owed. One suppression stands: that the row could be proven on a
stock image if bzr warned instead of refusing, suppressed against ADR 0052, which settled
that question and which this charter also excludes.

## Resume facts

- Branch `feat/prove-saved-search-filter-710`; `BASE_BRANCH=main` (absorbed by merge, so the
  eventual merge method is `--merge`); worktree `../bzr-worktrees/quest-710`.
- Guardrails: `make lint`, `make test`, `make functional-compare-all`.
- Scope charter: issue #710 `WORK:SCOPE`, token `q710-23cc718c`.
- Design set: this plan, `../specs/2026-09-06-prove-server-saved-search-filter-design.md`,
  `../../adr/0061-prove-vendor-extension-behaviour-against-a-shaped-proxy.md`.
- ADR index row for 0061: **pending**, owned by the campaign orchestrator.
- Design review: 2 cycles, 3 rounds total. Cycle 1 stopped blocked at its budget; the
  orchestrator authorized one bounded confirming cycle. All findings `accepted-fixed`.
