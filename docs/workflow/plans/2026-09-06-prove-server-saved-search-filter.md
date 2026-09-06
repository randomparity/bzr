# Plan: prove the comparison harness's server-side saved search filters

**Goal.** Make `compare/01-bug-lifecycle/saved-search` able to fail for the right reason,
by giving the shaped proxy a Red-Hat saved-search fixture and restating the row's
assertions as a filtered set against an unfiltered control.

**Architecture.** `tests/functional/redhat-shape-proxy.py` is a host-side HTTP forwarding
proxy in front of the Bugzilla container, with response-rewrite hooks registered in a
`REWRITE_HOOKS` tuple of `(matcher, transformer)` pairs and selected by
`BZR_FUNC_REDHAT_MODE`. A new `saved-search` mode advertises the `RedHat` extension at
`GET /rest/extensions` and resolves `savedsearch`/`sharer_id` on `GET /rest/bug`.
`tests/functional/compare/01-bug-lifecycle.sh` runs two bzr calls through that proxy —
one with `--saved-search`, one without — and asserts their id sets differ.
`tests/functional/pybz/container-tests.sh` sources the real phase script against stubs and
is where the row's controls are proven without a container.

**Tech stack.** Bash 3.2-compatible shell (BSD userland host), Python 3 standard library
only (`http.server`, `urllib.parse`, `json`, `unittest`), `jq`, Docker or podman.

Expected implementation size: 330–410 changed lines (M) — derived from the file map below:
~195 in the proxy including its seven `ShapeTests` cases, ~45 in the lifecycle phase, ~15
in `run-compare.sh`, ~100 in the harness self-test (three controls, the proxy stubs, and
the seed fixture), and 2 record lines.

## Global Constraints

- **Bash 3.2.** The host is macOS/BSD userland. No `declare -A`, no `${var^^}`, no
  `mapfile`. Phase and harness scripts use **4-space indent** and are **not** CI-linted;
  bare `shfmt` will disagree with the house style — match the surrounding file.
- **Python standard library only.** The proxy is executed as a bare script inside a
  container and on the host; it may not import a third-party package.
- **No `src/` change.** ADR 0052's client behaviour ships separately (#670). This plan
  touches only `tests/functional/**` and two record lines.
- **Do not touch `docs/adr/README.md`.** The ADR index row for 0061 is reported as
  pending; the orchestrator appends it.
- **Sibling-owned files are off limits:** `src/client/auth/*`, `src/client/version.rs`,
  `src/client/resources/{comment,attachment,bug}.rs`, `src/client/mod.rs`,
  `src/cli/mod.rs`, `docs/bzr-cli.md`, and phase scripts `02`, `08`, `08e`, `09c`, `15b`,
  `16b` (issues #713, #714, #719). Report anything adjacent; do not fix it.
- **Guardrails.** `make lint`; `make test` (never bare `cargo test`; use
  `make test-one T=<substring>` / `make test-fast` while iterating);
  `make functional-compare-all` is the only gate that reaches this change.
- **Branch** `feat/prove-saved-search-filter-710`, base `main`
  (`BASE_BRANCH=main`), worktree at `../bzr-worktrees/quest-710`.
- **Run gates bare** — no `| tail`, no `>/dev/null`, no `|| true`.
- The proxy's fixture environment variables are named exactly
  `BZR_FUNC_SAVED_SEARCH_NAME`, `BZR_FUNC_SAVED_SEARCH_IDS`,
  `BZR_FUNC_SAVED_SEARCH_SHARER`.
- The advertised extension name is exactly `RedHat`, matching
  `RED_HAT_EXTENSION` in `src/commands/runtime/shared/capability.rs`. The advertised value
  is `{"version": "1.0"}`, matching `ExtensionInfo { version: Option<String> }` in
  `src/types/server_info.rs`.

## File map

| File | Change | Answerable for |
|---|---|---|
| `tests/functional/redhat-shape-proxy.py` | modify | the `saved-search` mode: extension advertisement, `savedsearch`/`sharer_id` resolution, and its `ShapeTests` |
| `tests/functional/run-compare.sh` | modify | `seed_server_saved_search` accepting a variable number of bug ids |
| `tests/functional/compare/01-bug-lifecycle.sh` | modify | the `saved-search` row's assertions, the two new id helpers, and the proxied-call wrapper |
| `tests/functional/pybz/container-tests.sh` | modify | stubs for the proxy lifecycle and the proxied call; three new failure controls, one per new assertion; a fixture exercising the real seeder; count and fixture-row updates |
| `docs/dev/python-bugzilla-parity.md` | modify | the `Server saved search` row text |

Existing names this plan borrows, each confirmed to exist with the assumed signature:

- `_hook_matches(prefixes=(), *, method=None, mode=None)` — `redhat-shape-proxy.py:377`
- `REWRITE_HOOKS` tuple of `(matcher, transformer)` — `redhat-shape-proxy.py:460`
- `apply_rewrite_hooks(method, path, body, enabled_modes)` — `redhat-shape-proxy.py:478`
- `emit_rewrite_evidence(evidence)` over `(marker, route, count)` — `redhat-shape-proxy.py:489`
- `make_handler(backend_port)` and its `BZR_FUNC_REDHAT_MODE` reads — `redhat-shape-proxy.py:540`
- `redhat_shape_start <backend_port>` / `redhat_shape_stop`, setting `REDHAT_SHAPE_PORT` — `lib.sh:1438`, `lib.sh:1460`
- `lifecycle_bzr_probe`, `lifecycle_bzr`, `lifecycle_bzr_refusal_gap`, `lifecycle_ids_are`,
  `lifecycle_pybz`, `lifecycle_transport_is`, `lifecycle_expect_gap` —
  `compare/01-bug-lifecycle.sh:57,98,113,208,171,228,162`
- `expect_gap` converting FAIL→GAP and PASS→FAIL — `lib.sh:209`
- `run_gap_ineligible_control <flag> <capability> <label>` — `container-tests.sh:406`
- `run_lifecycle_failure_control <flag> <capability> <label> [value]` — `container-tests.sh:371`

## Task 1 — the proxy's `saved-search` mode

Creates nothing; modifies `tests/functional/redhat-shape-proxy.py`. This is the fixture
every later task depends on.

**Interfaces produced** (later tasks rely on these names):

- module constant `RED_HAT_EXTENSION = "RedHat"`
- `_saved_search_fixture() -> tuple[str, set[int], str] | None`
- `shape_saved_search_extensions(path, data) -> (bytes, dict)`
- `shape_saved_search_response(path, data, fixture) -> (bytes, dict)`
- `_run_saved_search_hook(method, path, body) -> (bytes, list)`
- mode string `saved-search` for `BZR_FUNC_REDHAT_MODE`

### Verification

- Contract: the proxy advertises `RedHat` at `/rest/extensions` when the mode is on.
  Mode: focused-test. Test: `ShapeTests.test_advertises_red_hat_extension` in
  `redhat-shape-proxy.py`. Expected red before the transformer exists: `AttributeError`
  / `NameError` on `shape_saved_search_extensions`. Green:
  `python3 tests/functional/redhat-shape-proxy.py --self-test`.
- Contract: `savedsearch` matching the fixture filters `bugs` to the fixture ids.
  Mode: focused-test. Test: `ShapeTests.test_saved_search_filters_to_fixture_ids`.
  Expected red: the unfiltered three-bug body comes back unchanged, so
  `assertEqual([1], ids)` fails with `[1, 2, 3] != [1]`. Green: same command.
- Contract: a `savedsearch` naming an unknown query resolves to no bugs.
  Mode: focused-test. Test: `ShapeTests.test_unknown_saved_search_resolves_empty`.
  Expected red: `[1, 2, 3] != []`. Green: same command.
- Contract: a mismatched `sharer_id` resolves to no bugs; a matching one filters.
  Mode: focused-test. Test: `ShapeTests.test_sharer_id_qualifies_resolution`.
  Expected red: `[1, 2, 3] != []` on the mismatch leg. Green: same command.
- Contract: with no `savedsearch` parameter the response is forwarded unchanged.
  Mode: focused-test. Test: `ShapeTests.test_absent_saved_search_is_untouched`.
  Expected red: the body is filtered anyway, `[] != [1, 2, 3]`. Green: same command.
- Contract: a malformed `BZR_FUNC_SAVED_SEARCH_IDS` disables the fixture rather than
  filtering to nothing. Mode: focused-test. Test:
  `ShapeTests.test_malformed_fixture_ids_disable_filtering`. Expected red:
  `[] != [1, 2, 3]`. Green: same command.
- Contract: the hook is reached only when the mode is enabled — i.e. Step 1.6's registry
  entry and Step 1.7's enabled-modes wiring are both present. Mode: focused-test. Test:
  `ShapeTests.test_mode_gates_the_saved_search_hook`, which goes through
  `apply_rewrite_hooks` rather than calling the transformer directly. Expected red: with
  the registry entry missing the enabled case comes back unfiltered, `[1, 2, 3] != [1]`.
  Green: same command. This case exists because the other six bypass the matcher entirely,
  and `make_handler`'s enabled-modes set is a place this codebase has already left a mode
  out — `bearer_auth_mode` is deliberately absent from it, so an omission there looks
  ordinary. Skipping Step 1.7 would leave the proxy never advertising `RedHat`, and the row
  would then fail on an ADR-0052 capability error that reads like a bzr regression rather
  than a harness wiring bug.

Confirmed: `main()` in `redhat-shape-proxy.py` dispatches on exactly `--self-test`, loading
`ShapeTests` through `unittest.defaultTestLoader` and returning 1 on failure. That is the
command every step below uses.

Also confirmed: the file does `import unittest` at line 21 but **not** `import
unittest.mock`, and `import unittest` alone does not bind the `mock` submodule. Add
`import unittest.mock` to the import block in Step 1.1, or the new cases fail with
`AttributeError: module 'unittest' has no attribute 'mock'`.

**Step 1.1.** Add `import unittest.mock` to the import block (after `import unittest`), and
add the module constant beside `_MAX_REQUEST_BODY`:

```python
RED_HAT_EXTENSION = "RedHat"
```

**Step 1.2.** Add the fixture reader after `is_termless_bug_search`:

```python
def _saved_search_fixture():
    """Return (name, ids, sharer) for the saved-search fixture, or None when unusable.

    A malformed value disables the fixture instead of being coerced: filtering to an
    empty set on a typo would turn a harness mistake into a plausible-looking result,
    which is the exact failure this fixture exists to make visible.
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

**Step 1.3.** Add the extension transformer immediately after it:

```python
def shape_saved_search_extensions(path, data):
    """Return JSON bytes advertising the Red Hat extension at /rest/extensions.

    All three functional images answer this route with `{"extensions":{}}`, so the
    advertisement is a rewrite of a real response rather than a synthesized route.
    """
    if urllib.parse.urlsplit(path).path != "/rest/extensions":
        return data, {}
    value = json.loads(data)
    extensions = value.get("extensions") if isinstance(value, dict) else None
    if not isinstance(extensions, dict) or RED_HAT_EXTENSION in extensions:
        return data, {}
    extensions[RED_HAT_EXTENSION] = {"version": "1.0"}
    return json.dumps(value, separators=(",", ":")).encode(), {"extensions": 1}
```

**Step 1.4.** Add the search transformer immediately after it:

```python
def shape_saved_search_response(path, data, fixture):
    """Return JSON bytes with `bugs` resolved as a Red Hat saved search would.

    Red Hat Bugzilla resolves `savedsearch`, optionally qualified by `sharer_id`,
    against stored named queries. Upstream accepts both and silently discards them,
    so the comparison tier cannot otherwise tell a honoured parameter from an ignored
    one. Filtering only when the parameter is present is what makes a client that
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

**Step 1.6.** Register it in `REWRITE_HOOKS`, on the line immediately after the
`_run_server_hook` entry so the mode-gated hooks stay together:

```python
    (_hook_matches(mode="saved-search"), _run_saved_search_hook),
```

**Step 1.7.** In `make_handler`, add the mode flag beside the existing four:

```python
    saved_search_mode = os.environ.get("BZR_FUNC_REDHAT_MODE") == "saved-search"
```

and add its entry to the enabled-modes set built in `_forward`, after the `cc-objects`
entry:

```python
                            ("saved-search", saved_search_mode),
```

**Step 1.8.** Add the six `ShapeTests` cases named in the Verification inventory. Each
sets the fixture environment with `unittest.mock.patch.dict(os.environ, ...)` — import
`unittest.mock` if the file does not already — calls the transformer directly with a
three-bug body, and asserts on the resulting ids. Body used by every case:

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
```

The cases:

- `test_saved_search_filters_to_fixture_ids`:
  `self.assertEqual(self._resolve("/rest/bug?savedsearch=owned-search"), [1])`
- `test_unknown_saved_search_resolves_empty`:
  `self.assertEqual(self._resolve("/rest/bug?savedsearch=other"), [])`
- `test_sharer_id_qualifies_resolution`:
  `self.assertEqual(self._resolve("/rest/bug?savedsearch=owned-search&sharer_id=7"), [1])`
  and
  `self.assertEqual(self._resolve("/rest/bug?savedsearch=owned-search&sharer_id=8"), [])`
- `test_absent_saved_search_is_untouched`:
  `self.assertEqual(self._resolve("/rest/bug?summary=x"), [1, 2, 3])`
- `test_malformed_fixture_ids_disable_filtering`:
  `self.assertEqual(self._resolve("/rest/bug?savedsearch=owned-search", BZR_FUNC_SAVED_SEARCH_IDS="1,x"), [1, 2, 3])`
- `test_advertises_red_hat_extension` calls
  `_run_saved_search_hook("GET", "/rest/extensions", b'{"extensions":{}}')` and asserts
  `json.loads(body)["extensions"]["RedHat"] == {"version": "1.0"}`.
- `test_mode_gates_the_saved_search_hook` goes through the registry instead of the
  transformer, so it covers Step 1.6 and Step 1.7:

  ```python
      def test_mode_gates_the_saved_search_hook(self):
          environment = {
              "BZR_FUNC_SAVED_SEARCH_NAME": "owned-search",
              "BZR_FUNC_SAVED_SEARCH_IDS": "1",
          }
          path = "/rest/bug?savedsearch=owned-search"
          with unittest.mock.patch.dict(os.environ, environment, clear=False):
              enabled, _ = apply_rewrite_hooks(
                  "GET", path, self._SAVED_SEARCH_BODY, {"saved-search"}
              )
              disabled, _ = apply_rewrite_hooks(
                  "GET", path, self._SAVED_SEARCH_BODY, set()
              )
          self.assertEqual([bug["id"] for bug in json.loads(enabled)["bugs"]], [1])
          self.assertEqual(json.loads(disabled)["bugs"], [{"id": 1}, {"id": 2}, {"id": 3}])
  ```

  The disabled leg asserts on the parsed body rather than raw bytes because
  `_run_bug_hook` matches `/rest/bug` unconditionally and re-serializes it, so the bytes
  are not identical even when nothing filtered.

**Step 1.9.** Run the self-tests bare and read the count:

```bash
python3 tests/functional/redhat-shape-proxy.py --self-test
```

Expect `OK` and a test count seven higher than the 44 the file runs today.

**Acceptance criteria.** All seven cases pass; the existing 44 `ShapeTests` still pass;
`test_mode_gates_the_saved_search_hook` demonstrates that with the mode off no `/rest/bug`
response is filtered.

## Task 2 — seed a strict subset

Modifies `tests/functional/run-compare.sh`. The seeded named query must select fewer bugs
than an unfiltered search returns, or the row's two id sets cannot differ.

**Interfaces produced.** `seed_server_saved_search LOGIN NAME BUG_ID [BUG_ID...]` — one or
more ids, replacing the fixed pair. Consumed by Task 3 and stubbed by Task 4.

### Verification

- Contract: the seeder accepts one or more ids, rejects zero ids and a non-decimal id, and
  builds the `bug_id=` list from all of them. Mode: focused-test. Test:
  `run_saved_search_seed_fixture` in `container-tests.sh` (Task 4, Step 4.0), which
  extracts and sources the **real** function from `run-compare.sh` and drives it with
  stubbed container helpers. Expected red: with the old two-id signature in place, the
  one-id call returns 2 and the fixture reports `seed_server_saved_search rejected a
  single id`. Green: `bash tests/functional/pybz/container-tests.sh`.

  Nothing compares the real function to the lifecycle fixture's hand-written stub today —
  `rg -n 'seed_server_saved_search' tests/` returns only the definition
  (`run-compare.sh:34`), that stub (`container-tests.sh:337`) and the single call site
  (`01-bug-lifecycle.sh:475`). The new fixture is what makes the guards testable at all;
  it runs the real text, so it cannot drift from the definition the way a second
  hand-written stub would.

**Step 2.1.** Replace the argument guard and query construction in
`seed_server_saved_search`:

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

`seed-saved-search.pl` is unchanged: it already takes `LOGIN NAME QUERY` and the query
string is built here.

**Acceptance criteria.** `bash -n tests/functional/run-compare.sh` is clean; the function
builds `bug_id=41&bug_id_type=anyexact` for a single id and
`bug_id=41,42&bug_id_type=anyexact` for two.

## Task 3 — the discriminating row

Modifies `tests/functional/compare/01-bug-lifecycle.sh`.

**Interfaces produced.** `lifecycle_ids_contain <file> <json_id_array>`,
`lifecycle_saved_search_probe <name> <args...>`, and the `LIFECYCLE_BZR_URL` override read
by `lifecycle_bzr_probe`. Task 4 stubs `redhat_shape_start` / `redhat_shape_stop` and keys
its controls on the capture-file names `saved-search-control` and `saved-search-filtered`.

### Verification

One control per assertion. Each assertion must be shown to bite through *itself*, not
through a neighbour that short-circuits ahead of it in the `&&` chain.

- Contract: the filtered call equals the seeded subset, so a server that ignores the
  parameter fails. Mode: focused-test. Test: `run_lifecycle_failure_control` with
  `LIFECYCLE_SAVED_SEARCH_UNFILTERED` in Task 4 — the proxied filtered call returns the
  control set and the row must go red on this assertion. Expected red before this task: the
  control does not exist, so the flag has no effect and the row still passes. Green:
  `bash tests/functional/pybz/container-tests.sh`.
- Contract: the unfiltered control contains both lifecycle bugs, so the filtered result is
  a strict subset of something and the comparison is not vacuous. Mode: focused-test.
  Test: `run_lifecycle_failure_control` with `LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW` in
  Task 4 — the control returns only the bzr bug and the row must go red. Expected red
  before this task: the assertion does not exist, and a control equal to the seeded subset
  passes, which is the defect this issue exists to fix. Green: same.
- Contract: python-bugzilla's result contains an id the seeded query excludes.
  Mode: focused-test. Test: `run_lifecycle_failure_control` with
  `LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED` in Task 4. Expected red before this task: the
  assertion does not exist and the row passes with a filtered pybz result. Green: same.
- Contract: the row remains an expected gap (#670) in the default scenario and reports the
  gap stale when bzr stops refusing. Mode: focused-test. Test: the existing default-scenario
  count assertions and the `LIFECYCLE_STALE_GAPS` scenario in `container-tests.sh`.
  Expected red: `lifecycle gap count` assertion reports a changed count. Green: same.

**Step 3.1.** Let a probe target a URL other than `$BZ_URL`. In `lifecycle_bzr_probe`,
replace the `--server-url "$BZ_URL"` argument with the override:

```bash
    RUST_LOG=bzr=debug run_bzr --server-url "${LIFECYCLE_BZR_URL:-$BZ_URL}" \
        --server-api-key-env BZR_COMPARE_API_KEY --server-email "$COMPARE_ADMIN_EMAIL" \
        --api "$api" "$@"
```

Nothing else sets `LIFECYCLE_BZR_URL`, so every existing caller is unaffected.

**Step 3.2.** Add the one id helper immediately after `lifecycle_ids_are`:

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

**Step 3.3.** Add the proxied-probe wrapper immediately after `lifecycle_bzr_refusal_gap`:

```bash
# Run one bzr probe against the Red-Hat-shaped proxy rather than the container.
# The proxy advertises the `RedHat` extension so ADR-0052's capability gate passes,
# and resolves `savedsearch` against the same ids the named query was seeded with.
# Only bzr is routed here: the python-bugzilla arm stays on the stock container so
# the row still measures the documented stock-server difference (ADR-0061).
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
`BZR_FUNC_SAVED_SEARCH_SHARER` is deliberately unset: the row does not pass `--sharer`,
and the fixture treats an absent `sharer_id` as unqualified. `sharer_id` resolution is
proven by Task 1's `ShapeTests` instead, where both legs can be exercised without seeding
a second Bugzilla account.

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

Four things about this shape are deliberate and must not be rearranged:

- Every new assertion sits in the **precondition** chain, so a failure produces an outright
  FAIL that never reaches `lifecycle_expect_gap 670`. That is what the eight existing
  `run_gap_ineligible_control` entries for this row already assert.
- The **stock refusal is the last bzr probe**, so the gap eligibility
  `lifecycle_expect_gap` reads is the one the refusal set. Every `lifecycle_bzr_probe` call
  begins with `lifecycle_gap_reset`, so an earlier probe's eligibility would be discarded.
- The inner `lifecycle_ids_are` now expects `[$LIFECYCLE_BZR_ID]`, the seeded subset,
  because that is what a bzr which stopped refusing would have to return for the gap to be
  genuinely stale.
- The control uses **containment** and the filtered call uses **equality**, and neither is
  interchangeable with the other. The control set is every stem-bearing bug in the run and
  is not fixed by construction — `01-bug-lifecycle.sh` already creates
  `"$LIFECYCLE_STEM generic bzr"` and `"$LIFECYCLE_STEM generic pybz"` in the
  `arbitrary-fields` row, and only that row's position *after* this one keeps the count at
  two. Do **not** add a third assertion that the two sets differ: given a control
  containing both bugs and a filtered result equal to one of them, it is entailed and
  could never fail.

**Acceptance criteria.** `bash -n tests/functional/compare/01-bug-lifecycle.sh` is clean.
Against a live container the row reports `GAP (#670)`. The captured
`saved-search-filtered.bzr.stdout.json` holds exactly the bzr id, and
`saved-search-control.bzr.stdout.json` holds at least the bzr and pybz ids — so the
control genuinely exceeds the filtered result on real data.

## Task 4 — the harness self-test keeps step and gains two controls

Modifies `tests/functional/pybz/container-tests.sh`. This is where the row's controls are
proven without a container, and it is the only place a mismatch between the parity document
and its fixture copy is caught.

**Interfaces produced.** Controls `LIFECYCLE_SAVED_SEARCH_UNFILTERED`,
`LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW` and `LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED`; stubs
`redhat_shape_start` / `redhat_shape_stop`; fixture `run_saved_search_seed_fixture`.

### Verification

- Contract: an unfiltered proxied result turns the row red.
  Mode: focused-test. Test: `run_lifecycle_failure_control LIFECYCLE_SAVED_SEARCH_UNFILTERED
  saved-search 'server saved search'`. Expected red before Task 3: the flag changes nothing
  and the control reports `saved-search control LIFECYCLE_SAVED_SEARCH_UNFILTERED
  unexpectedly passed`. Green: the invocation in Step 4.6.
- Contract: a control that cannot exceed the seeded subset turns the row red.
  Mode: focused-test. Test: `run_lifecycle_failure_control
  LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW saved-search 'server saved search'`. Expected red
  before Task 3: `saved-search control LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW unexpectedly
  passed`. Green: same.
- Contract: a python-bugzilla result that appears filtered turns the row red.
  Mode: focused-test. Test: `run_lifecycle_failure_control
  LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED saved-search 'server saved search'`. Expected red
  before Task 3: `saved-search control LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED unexpectedly
  passed`. Green: same.
- Contract: the real `seed_server_saved_search` accepts one or more ids and rejects the
  rest. Mode: focused-test. Test: `run_saved_search_seed_fixture` (Step 4.0). Expected red
  before Task 2: the one-id call returns 2. Green: same.
- Contract: the default scenario's PASS/FAIL/GAP counts and the stale-gap scenario are
  unchanged by the new assertions. Mode: focused-test. Test: the existing
  `assert_equals 7 "$PASS_COUNT"` / `0 "$FAIL_COUNT"` / `3 "$GAP_COUNT"` and
  `assert_equals 3 "$FAIL_COUNT" "stale gap fail count"`. Expected red: a count assertion
  reports the new value. Green: same.

**Step 4.0.** Add a fixture that exercises the **real** seeder. Put it beside the other
`run_*_fixture` definitions and add `run_saved_search_seed_fixture` to the call list at the
bottom of the file, immediately before `run_lifecycle_phase_fixture`:

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

Three things make this work, and each is already true of the file:

- `container_runtime` returns `echo`, so the function's last line runs
  `echo exec -i --workdir … fixture-container perl -I. - admin@test.bzr fixture-name <query>`.
  The query is the final whitespace-separated field and contains no spaces, so
  `awk '{print $NF}'` reads it exactly. `perl` never runs and no container is touched.
- The `<"$helper"` redirect still needs `$SCRIPT_DIR/compare/seed-saved-search.pl` to be
  readable, which is why the fixture sets `SCRIPT_DIR` to `tests/functional`.
- `container-tests.sh` sources `lib.sh` at line 9, so `container_runtime` and
  `bugzilla_container_name` already exist; the fixture body is wrapped in `( … )` so the
  overrides and `SCRIPT_DIR` do not leak into later fixtures.

`assert_equals` takes `expected actual label` in that order (`container-tests.sh:11`), and
`PYBZ_DIR` is set at `container-tests.sh:7` to the `pybz` directory. The script runs under
`set -euo pipefail`, which is why the two rejection calls are bracketed by `set +e` /
`set -e`.

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

**Step 4.2.** Add proxy-lifecycle stubs beside it. The fixture has no proxy, so these
record the port the phase will use and succeed:

```bash
    redhat_shape_start() {
        [[ $# -eq 1 && ${BZR_FUNC_REDHAT_MODE:-} == saved-search &&
            -n ${BZR_FUNC_SAVED_SEARCH_NAME:-} && -n ${BZR_FUNC_SAVED_SEARCH_IDS:-} ]] ||
            return 1
        REDHAT_SHAPE_PORT=59999
    }
    redhat_shape_stop() { REDHAT_SHAPE_PORT=""; }
```

`BZ_PORT` is not otherwise set in the fixture; add `BZ_PORT=1` to the fixture's variable
block beside `BZ_URL=http://127.0.0.1` so the wrapper's argument is non-empty.

**Step 4.3.** Teach the stubbed `run_bzr` about the two proxied calls. Insert this
immediately before the existing `--saved-search` refusal branch (the block guarded by
`$args == *" --saved-search "*` at what is currently line 629):

```bash
        # The proxied control and filtered calls (ADR-0061). The control is an
        # ordinary summary search through the shaped proxy; the filtered call is
        # what the fixture resolves. LIFECYCLE_SAVED_SEARCH_UNFILTERED models a
        # proxy or client that stopped applying the filter, which must turn the
        # row red rather than passing on a set equal to its own control.
        # LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW models the other half: a control
        # that no longer exceeds the seeded subset, which would make the
        # comparison vacuous even with the filter working.
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

**Step 4.4.** Make the pybz stub model an ignoring server, and give it the control flag.
Replace the `saved_search)` arm in `fake_lifecycle_runtime`:

```bash
        saved_search)
            # An ignoring server returns the unfiltered set, which includes the
            # pybz bug the seeded query excludes. LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED
            # models python-bugzilla appearing to honour the parameter.
            result='[{"id":41},{"id":42}]'
            [[ ${LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED:-0} -eq 0 ]] || result='[{"id":41}]'
            ;;
```

**Step 4.5.** Update the stale-gap arm so a bzr that stopped refusing returns the seeded
subset — otherwise the stale-gap scenario fails on the id assertion rather than reporting
the gap stale. Replace the `--saved-search` case at what is currently line 674:

```bash
            *" --saved-search "*) printf '[{"id":41}]\n' >"$BZR_STDOUT" ;;
```

**Step 4.6.** Add the three controls — one per new assertion. All three fail in the
**precondition** chain, so the row never reaches `lifecycle_expect_gap` and the outcome is
an outright FAIL, which is `run_lifecycle_failure_control`, not
`run_gap_ineligible_control`. Add beside the other `run_lifecycle_failure_control` calls:

```bash
    for control in LIFECYCLE_SAVED_SEARCH_UNFILTERED \
        LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW \
        LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED; do
        if ! run_lifecycle_failure_control "$control" saved-search 'server saved search'; then
            control_failures=$((control_failures + 1))
        fi
    done
```

Each reddens through its own assertion rather than through a neighbour: the chain is
control-containment, then filtered-equality, then pybz-containment, and each control
corrupts exactly the input its own assertion reads.

**Step 4.7.** Run the self-test bare and read the counts:

```bash
bash tests/functional/pybz/container-tests.sh
```

Confirmed: this is exactly the invocation `make functional-compare-all` uses
(`Makefile:231`), and it is the first thing that target runs. The script executes every
`run_*_fixture` in sequence; the later ones (`run_container_fixture`,
`run_sidecar_wrapper_fixture`) need a working container runtime, so run it on a host with
Docker or podman available. `assert_equals` is defined at `container-tests.sh:11`.

Expect the lifecycle fixture's `assert_equals 7 "$PASS_COUNT"`, `0 "$FAIL_COUNT"`,
`3 "$GAP_COUNT"` to hold, three new `controlled red: saved-search ...` lines, and the four
`run_saved_search_seed_fixture` assertions from Step 4.0.

**Step 4.8.** If any of the three count assertions now reports a different value, do not
adjust the expected number to match. Read the fixture output it prints on failure and
establish which assertion changed the row's outcome; a changed count here means the row's
shape changed, which this plan does not intend.

**Acceptance criteria.** All three new controls report `controlled red`;
`run_saved_search_seed_fixture` passes; the default-scenario counts are unchanged at 7/0/3;
the stale-gap scenario still reports `#670 appears resolved` and
`assert_equals 3 "$FAIL_COUNT" "stale gap fail count"` holds.

## Task 5 — the parity record and its fixture copy

Modifies `docs/dev/python-bugzilla-parity.md` and
`tests/functional/pybz/container-tests.sh`. `run_parity_report_fixture` greps the document
for each fixture literal with `grep -Fxc "$row"`, requiring exactly one whole-line match
(`container-tests.sh:1037`), so a fixture row missing from the document does fail. The
reverse — a document row with no fixture entry — is not caught, so both edits stay in one
task.

### Verification

- Contract: the document row and the fixture row are byte-identical.
  Mode: focused-test. Test: `run_parity_report_fixture` in `container-tests.sh`, which
  greps the document for each literal row. Expected red: change one of the two and the
  fixture reports the row as missing. Green: `bash tests/functional/pybz/container-tests.sh`.

**Step 5.1.** Replace the `Server saved search` row in `docs/dev/python-bugzilla-parity.md`
(currently line 13) with:

```
| Server saved search | `bzr bug search --saved-search` | stock: bzr errors, python-bugzilla returns unfiltered results (#670); Red-Hat-shaped proxy: bzr filters | `compare/01-bug-lifecycle/saved-search` |
```

**Step 5.2.** Replace the identical literal in `run_parity_report_fixture` (currently line
1001) with the same string, keeping the surrounding single quotes:

```bash
        '| Server saved search | `bzr bug search --saved-search` | stock: bzr errors, python-bugzilla returns unfiltered results (#670); Red-Hat-shaped proxy: bzr filters | `compare/01-bug-lifecycle/saved-search` |'
```

**Acceptance criteria.** `run_parity_report_fixture` passes — that is the check, and it
already runs as the first step of `make functional-compare-all`. Do not add a separate
`diff` of the two files; it would duplicate a check the fixture performs.

## Task 6 — full-tier verification

Creates and modifies nothing. This is the only gate that reaches the change, and the issue
makes it an acceptance criterion.

### Verification

- Contract: the row is green on all three supported images with real containers.
  Mode: focused-test. Test: `make functional-compare-all`. Expected red before Tasks 1–5:
  the row cannot reach its filtered assertion at all. Green: the command below.

**Step 6.1.** Build the release binary the comparison tier runs:

```bash
cargo build --release
```

**Step 6.2.** Run the whole comparison tier bare, in the background, and read it on
completion — it exceeds the foreground command timeout:

```bash
make functional-compare-all
```

Expect `FAILED: 0` for each of `bz50`, `bz52`, `bz53`, and
`[compare/01-bug-lifecycle/saved-search] server saved search ... GAP (#670)` in each.

**Step 6.3.** Confirm the discrimination held on real data rather than assuming it. The
green row alone does not say the control exceeded the filtered result — that is what the
`lifecycle_ids_contain` control assertion establishes, and it is an assertion that can
fail, so a green row is evidence for it. Record in the completion report that the row was
green **and** that `LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW` reddened it in the self-test,
because that pair is what rules out the vacuous-oracle failure this issue exists to fix.

The exchange directory is removed on exit, so do not plan to inspect the captured id files
after the run; the assertions inside the run are the evidence.

**Step 6.4.** Run the repository guardrails bare:

```bash
make lint
make test
```

Expect both green. Neither reaches this change; they are run because the branch must not
regress them.

**Acceptance criteria.** `make functional-compare-all` green on all three images;
`make lint` and `make test` green.

## Deferrals

None. The design review (`$gauntlet`, iteration 1) raised 8 findings and every one was
dispositioned `accepted-fixed` in this set — no deferral record and no tracker issue is
owed. One suppression was recorded and stands: the reviewer noted that the row could be
proven directly on a stock image if bzr sent `savedsearch` with a warning instead of
refusing before dispatch, suppressed against ADR 0052, which settled that question and
which this charter also excludes.

Any deferral from the branch review is appended here with its owning record path or tracker
issue.

## Design review status — BLOCKED at the iteration budget

`$trial-loop` (`$gauntlet`, 2 iterations, budget 2) stopped as blocked with a residual
blocking figure of 2. Iteration 1 raised 8 findings, all `accepted-fixed`; iteration 2
raised 6, of which two are blocking and **unresolved**. Neither may be treated as settled;
both need a decision this run did not have the budget to take.

1. **`test_mode_gates_the_saved_search_hook` does not cover Step 1.7** (high). Task 1's
   Verification claims the case covers both Step 1.6's registry entry and Step 1.7's
   `make_handler` enabled-modes wiring. It does not: the case builds the enabled-modes set
   by hand and passes it to `apply_rewrite_hooks`, never reaching `make_handler`. The
   reviewer applied Steps 1.1–1.8 to a scratch copy and deleted only the Step 1.7 edits —
   `--self-test` still reported `Ran 51 tests ... OK`. Deleting Step 1.6 instead does redden
   it. So a missing Step 1.7 would pass `make lint`, `make test` and the proxy self-test,
   and first surface as `make functional-compare-all` failing the row with
   `unsupported_server_capability` — the misleading failure this case was written to
   prevent. Proposed remedy: drive a real request through `make_handler` against a stub
   backend, reusing the round-trip helper
   `test_termless_search_returns_production_shaped_code_1000` already uses, and verify the
   new case by deleting Step 1.7 and confirming it reds.
2. **Design-to-implementation ratio is 3.1x–3.8x** (high), above the 3x blocking line at
   every point of this plan's own 330–410 estimate. The reviewer costed the estimate
   independently at ~353 added lines and confirmed it is honest, so widening it is not
   available and would be a defeated control. The excess is argument repeated verbatim
   across the set — the entailment argument three times, the 182-bug `curl` block twice,
   the bzr-arm-only rationale four times, and Task 1's Verification narrating the seven
   `ShapeTests` cases Step 1.8 then writes in full. Proposed remedy: state each once and
   cross-reference, which the reviewer estimates removes 300–400 lines and lands the set
   near 2.4x.

Four non-blocking findings are also open and unfixed: the "one flag apart" framing in the
spec and ADR is wrong (the two calls differ by subcommand, term, and flag — the row still
discriminates, but the recorded reason is not what the code does); the ADR cites
`/rest/extensions` evidence as being "in Context" where Context carries only the
`savedsearch` probe; "mirroring the server" is false, since all three images answer a
termless search with 200 and the whole database while the proxy synthesizes `code 1000`
itself; and several headings still say "two controls"/"six ShapeTests cases" above lists of
three and seven — fossils of the revision that dropped the entailed assertion.

Full findings: the `$gauntlet` artifact for run `gauntlet-710-design-run1-4f9c2a71`.

## Resume facts

- Branch `feat/prove-saved-search-filter-710`; `BASE_BRANCH=main`; worktree
  `../bzr-worktrees/quest-710`.
- Guardrails: `make lint`, `make test`, `make functional-compare-all`.
- Scope charter: issue #710 `WORK:SCOPE`, token `q710-23cc718c`.
- Design set: this plan, `../specs/2026-09-06-prove-server-saved-search-filter-design.md`,
  `../../adr/0061-prove-vendor-extension-behaviour-against-a-shaped-proxy.md`.
- ADR index row for 0061: **pending**, owned by the campaign orchestrator.
