# bzr Audit Report

## 1. Executive Summary

This audit produced **41 confirmed findings** plus **34 proposed complex functional sequences**.

**By kind:**
- Likely bugs (behavior wrong, uncaught by tests): **13**
- Coverage gaps (behavior correct, regression-undetectable): **28**

**By severity:**

| Severity | Count | Notable |
|----------|-------|---------|
| Medium | 14 | HTTP-200 error swallowing on PUT, path traversal on download, mask_api_key panic, empty/no-op update PUT, deadline unvalidated, clone non-atomicity |
| Low | 27 | exit-code mapping assertions, version/auth fallback branches, leap-year boundaries, integer/UTF-8 parse edges |

**Biggest themes:**

1. **Mutation paths skip the 200-error and validation guards that read paths have.** `put_json` never inspects the body, so a Bugzilla HTTP-200 `{"error":true}` envelope on `bug update` / `attachment update` / `create_user` is reported as success. Empty-update and unvalidated-`--deadline` PUTs reach the server when the codebase enforces equivalent guards elsewhere (`query`, `template`).
2. **Cross-transport (REST / XML-RPC / hybrid) parity is almost entirely untested** outside comment/attachment private-data fallback. XML-RPC parsing has real defects (self-closing `Event::Empty` elements cause EOF errors; byte-slice panics on UTF-8 and oversized error bodies).
3. **The error-code contract (18 variants → 13 exit codes) is asserted only per-variant in isolation** — no exhaustiveness/collision guard, and four variants (`Http`, `XmlRpc`, `TomlSerialize`, `BatchPartialFailure`) have no exit_code/error_type assertion at all. A stale `1..=12` comment in `main.rs` contradicts `EXIT_CODE_TLS = 13`.
4. **Unicode/boundary footguns in shared output helpers** — `mask_api_key` and `truncate` slice by byte index and panic on multi-byte input or small bounds.
5. **Security-relevant config/TLS branches (`--tls-pin-clear`, TOFU issuer change, keyring error mapping, unset-keyring permission hardening) are untested**, several silently diverging from the module's own invariants.

---

## 2. Likely Bugs (uncaught by tests)

| id | location | severity | what's wrong | suggested test |
|----|----------|----------|--------------|----------------|
| `client-bug-update-200-error-swallowed` | `src/client/mod.rs:239-243`, `src/client/bug.rs:424-426` | medium | `put_json` never reads the body; a Bugzilla HTTP-200 `{"error":true,...}` on `bug update` is reported as success (`check_bugzilla_200_error` is only on GET/parse paths). Batch updates can report all-succeeded while all were rejected. | wiremock PUT `/rest/bug/42` → 200 `{"error":true,"code":115}`; assert `update_bug` returns `Err(BzrError::Api{code:115})`. |
| `client-comment-attachment-download-empty-data-string` | `src/client/attachment.rs:130-139` | medium | Guard only fires on `data == None`; `Some("")` decodes to a 0-byte Vec, silently writing an empty file and reporting success for a filtered private attachment. | `write_one_attachment` with `data=Some("")`, `size>0`; assert error (or documented 0-byte behavior). |
| `commands-misc-attachment-download-path-traversal` | `src/commands/attachment.rs:255-257` | medium | `dest = out.unwrap_or(&filename)` uses raw server-supplied `file_name` with no sanitization; `../../etc/...` or absolute paths write outside the target dir. Batch `join("{id}.{file_name}")` still traverses via embedded `/`. | mock `file_name="../escape.txt"` (and absolute); assert write stays within target / is rejected. |
| `output-mask-api-key-non-ascii-panic` | `src/output/formatting.rs:117-123` | medium | `key.len() > 8` then `&key[..8]` byte-slices; a multi-byte char straddling offset 8 panics (`config show` on a non-ASCII inline key). Reproduced with `"1234567é9"`. | `mask_api_key("1234567é9abc")` asserts no panic + sensible prefix; `from_config` with non-ASCII key. |
| `commands-bug-clone-comment-failure-after-create` | `src/commands/bug/clone.rs:87-101` | medium | Bug is created (line 87), then the "Cloned from" comment POST `?`-propagates; a comment failure returns `Err` and never prints the created bug id. User sees failure, re-clones, duplicates. | mock POST `/rest/bug/200/comment` → 500 after create succeeds; assert created id 200 is surfaced. |
| `commands-bug-clone-missing-comment-zero` | `src/commands/bug/clone.rs:40-45` | medium | `find(|c| c.count == 0)` → `None` when source has no comment #0; clone created with `description: None`, no warning. Silent description loss. | mock comments with only `count>=1`; assert POST body omits `description` or a warning is emitted. |
| `validation-parsing-deadline-unvalidated` | `src/commands/bug/update.rs:146` | medium | `deadline: deadline.clone()` ships raw; no `parse_optional_date`, unlike every other date flag (which fails fast exit 7). `--deadline garbage` reaches the server. | `build_update_params` with `deadline=Some("tomorrow")` asserts `Err(InputValidation)` exit 7 after wiring validation. |
| `cli-parsing-bug-update-no-fields-empty-put` | `src/commands/bug/update.rs:99-174,192` | medium | `bug update 42` with no change flags builds an all-empty `UpdateBugParams` and PUTs it, printing "Updated bug #42". No "at least one field" guard (which `query`/`template` have). | unit test: all-empty action asserts `Err(InputValidation)` after adding guard. |
| `cli-parsing-query-save-search-filter-no-conflict` | `src/cli/query.rs:62-63`, `src/commands/query.rs:100-130` | medium | `--search` has no `conflicts_with_all` (unlike `--from-url`), yet docs claim mutual exclusivity. `query save q --search foo --product Firefox` is accepted; both quicksearch AND structured filters are sent at run time. | clap negative test asserting `ArgumentConflict` after adding `conflicts_with_all` to `search`. |
| `error-exitcodes-stale-range-comment` | `src/main.rs:73,75` | low | Comment says "exit codes are in the range 1..=12" but `EXIT_CODE_TLS = 13` (PinMismatch/IssuerChanged). Stale; masks a latent `unwrap_or(1)` truncation footgun if codes exceed 255. | update comments to `1..=13`; `main_tests.rs` asserts `exit_code(&PinMismatch{..})` renders ExitCode 13. |
| `commands-misc-flags-name-contains-status-char` | `src/commands/flags.rs:24-37` | low | `s.find(['+','-','?','X'])` takes the FIRST match; a flag name containing `X` mis-splits (`Xfeature+` → empty name; `feaXture+` → wrong split). Status char should be trailing. | `parse_flags(["Xfeature+"])` / `["feaXture?"]` assert trailing-status interpretation. |
| `output-truncate-small-bound-underflow` | `src/output/formatting.rs:91-98` | low | `max_chars - 3` underflows for `max_chars < 3` with over-length input: debug panics, release wraps. Internal helper; all current callers pass constants ≥ 60. | `truncate("abcdefg", 2)` (and 0/1) assert defined value; fix with `saturating_sub(3)`. |
| `xmlrpc-empty-element-events-ignored` | `src/xmlrpc/parsing.rs:108-169` | medium | Every parse loop matches only `Start`/`End`/`Text`; quick-xml emits `Event::Empty` for self-closing tags. `<value/>` → "unexpected EOF"; `<struct/>`/`<array/>` → wrong type (empty string). Bugzilla emits these for empty/null fields. | parse `<value/>`, `<value><struct/></value>`, `<value><array/></value>`; assert empty string/struct/array, not EOF. |
| `xmlrpc-http-error-body-slice-panic` | `src/xmlrpc/client.rs:77` | low | `&body[..body.len().min(512)]` byte-slices a String; a >512-byte error body with a multi-byte char at offset 512 panics. Only reachable with debug logging (`-vv`) enabled. | wiremock 500 with 600+ byte body crossing a char boundary at 512; assert `HttpStatus` returned, no panic. |
| `config-unset-keyring-skips-permission-hardening` | `src/commands/config.rs:410-425` | low | `save_config_without_validation` omits `0o600`/`0o700` hardening and the insecure-permissions warning that `Config::save()` applies. Only materializes if dir/file was recreated between credentialing and unset. | Unix: force the unset writer to create the file; assert `mode() & 0o077 == 0`. |
| `tls-keyring-issuer-der-extraction-no-tag-check` | `src/tls/verifier.rs:264-269` | low | `extract_issuer_der` doesn't check the element is a SEQUENCE (`0x30`) before `skip_der_element`, unlike `parse_issuer_from_tbs`. DER and string issuer paths can disagree on a malformed cert (affects which error is emitted, not accept/reject). | craft DER valid up to issuer but with a non-SEQUENCE there; assert `extract_issuer_der` returns `None` after adding tag check. |

---

## 3. Coverage Gaps by Subsystem

### client-bug
- **Hybrid search id-injection on XML-RPC fallback leg** — `src/client/bug.rs:204-232,247-273`: only tested with default fields; add a hybrid test with `include_fields="summary,status"` + empty-REST fallback asserting the XML-RPC request carries injected `id` and the bug deserializes.
- **`get_bug_rest` 100500 → empty-search → NotFound** — `src/client/bug.rs:374-416`: mock `/rest/bug/99` → 100500 and `/rest/bug?id=99` → `{"bugs":[]}`; assert `NotFound`, and (hybrid) assert XML-RPC mock gets zero requests.
- **`search_bugs` `limit=None` / `limit=Some(0)`** — `src/client/bug.rs:141-144,296-298`: assert `query_param_is_missing("limit")` when None, and `limit=0` is sent verbatim (Bugzilla's "no limit" semantic).

### client-comment-attachment
- **`update_attachment` all-None/empty params** — `src/client/attachment.rs:155-158`, `src/types/attachment.rs:79-96`: assert `UpdateAttachmentParams::default()` serializes to `{}` and command-layer either rejects or has defined behavior.

### client-auth-version
- **`whoami` 200-with-garbage-body (`UnparseableResponse`)** — `src/client/auth/whoami.rs:75-77`: header probe returns 200 `"<html>"`, query probe returns 200 `{"id":5}`; assert fall-through to QueryParam.
- **`valid_login` `NetworkError` → Header fallback** — `src/client/auth/mod.rs:116-122`: whoami 404 + valid_login send/read error; assert `Ok(AuthMethod::Header)`.
- **version-probe `apply_auth` failure → unauthenticated GET** — `src/client/version.rs:22-29`: call `detect_version_and_mode` with `api_key="bad\nkey"`, Header; assert it still returns `(Some(ver), Rest)`.
- **`version_to_api_mode` non-numeric / single-token** — `src/client/version.rs:81-87`: assert `""`, `"unknown"`, `"5.beta"`, `"v5.1"` → Hybrid (pins the catch-all and `(Some(5),None)` arms).

### client-misc
- **`whoami` < 5.1 fallback (32614 / 404)** — `src/client/user.rs:12-27`: three tests (32614+email, 404+email, no-email synthetic error) plus the empty user-lookup `NotFound`.
- **`create_user` Hybrid fallback + XmlRpc routing** — `src/client/user.rs:55-67`: Hybrid REST 200-error-envelope → XML-RPC id; XmlRpc mode expects 0 REST hits; document Hybrid-falls-back-on-403 behavior.
- **`list_products_by_type` multi-chunk (>50 ids)** — `src/client/product.rs:30-39`: 51+ accessible ids, `/rest/product` `.expect(2)`, assert ordered accumulation; second test errors a later chunk.
- **`get_field_values` empty-values vs NotFound** — `src/client/field.rs:24-36`: `{"fields":[{"values":[]}]}` asserts `Ok(vec![])`, not NotFound.
- **`server_info` extensions-call failure after version succeeds** — `src/client/server.rs:7-14`: `/rest/version` 200, `/rest/extensions` 500; assert `Err`.

### commands-bug
- **`bug my --created --cc` (both, no `--all`)** — `src/commands/bug/my.rs:46-60`: assert the assigned-to search is skipped (`expect(0)` on `assigned_to=<me>`), creator+cc merged.
- **Batch update all-failed + comment suffix** — `src/commands/bug/update.rs:217-239`: two ids both PUT→500, comment Some, Table format; assert `BatchPartialFailure{0,2}`, no "Updated bugs:" line, both failures on stderr. (Behavior is correct; only the test is missing.)

### commands-misc
- **`config set-server` `--api-key`/`--api-key-env` both/neither** — `src/commands/config.rs:158`: assert `InputValidation` exit 7, message contains "exactly one of".
- **`config set-server --tls-pin-clear`** — `src/commands/config.rs:142-156`: success path (seed pin, clear, assert all three fields None) and not-found path ("nothing to clear"). Branch also ignores `format` (no JSON output).
- **`comment add` stdin/editor body resolution** — `src/commands/comment.rs:37-104`: non-terminal stdin returning only comment lines → `InputValidation` "empty comment" after filter.

### config
- **`ApiMode::XmlRpc` serde round-trip** — `src/types/common.rs:41`, `src/config.rs:37-38`: save a ServerConfig with `XmlRpc`/`QueryParam`, assert TOML contains `api_mode = "xmlrpc"`, reload to same variants (the `#[serde(rename="xmlrpc")]` is load-bearing).
- **`Config::load` on corrupt TOML** — `src/config.rs:246`: invalid TOML → assert `BzrError::TomlParse`.
- **`Config::path()` relative-XDG fallback** — `src/config.rs:233-238`: relative `XDG_CONFIG_HOME` → assert it falls back to `dirs::config_dir`.
- **`resolve_api_key` env unset/empty** — `src/config.rs:175-187`: assert "is not set" and "is empty" errors.

### error-exitcodes
- **No exit_code/error_type assertion for `Http`, `XmlRpc`, `TomlSerialize`, `BatchPartialFailure`** — `src/error.rs:169-200`: add direct `assert_eq!` for each (5/"http", 4/"api", 3/"config", 11/"batch_partial_failure"), plus the Display string for batch.
- **No exhaustiveness/collision guard** — `src/error.rs:167,187`: table-driven test enumerating one instance of all 18 variants, asserting the code multiset matches the documented constants.

### output
- **`write_server_info` table branches** — `src/output/resources/server.rs:26-44`: no-extensions line, `unwrap_or("unknown")` version fallback, version header — call into a buffer with Table format (all tests use JSON).
- **`--fields` + `--exclude-fields` combined** — `src/output/resources/bug.rs:328-340,404-416,276-304`: include `id,status,priority` + exclude `status` across JSON projection, table columns, detail rows, and validators (logic correct; regression guard missing).
- **`colorize_status` lowercase actually colorizes** — `src/output/formatting.rs:108-115`: `set_override(true)` + `colorize_status("new")` asserts `\x1b[` present (current test only checks substring).

### validation-parsing
- **`limit=` URL param invalid/zero** — `src/url_parser.rs:162-166`: `limit=abc`/overflow silently dropped (no warn), `limit=0` accepted; decide and pin behavior.
- **Date year/leap boundaries** — `src/validation/datetime.rs:50-69`: `1900-02-29` is_err (century non-leap), `2000-02-29` is_ok (400-divisible), `0000-01-01` decision (the century-rule branches are mutation-surviving).

### tls-keyring
- **Keyring `map_error` non-NoEntry variants** — `src/credentials/keyring.rs:86-115`: direct `map_error` calls for PlatformFailure/NoStorageAccess/TooLong/Ambiguous/BadEncoding/Invalid/other asserting guidance substrings.
- **PIN_MISMATCH multi-RDN issuer** — `src/tls/pin_failure.rs:40-51`: chain with `issuer CN=foo, O=Acme, C=US`; assert full DN captured.
- **`probe_server_cert` "no certificate captured"** — `src/tls/tofu.rs:98-101`: http:// endpoint responds 200 to HEAD; assert `Err` "no certificate captured".
- **`keyring_stub` never compiled in CI** — `src/credentials/keyring_stub.rs:16-29`: add a `cargo test --no-default-features` CI job to execute the stub's Err/UNSUPPORTED bodies.

### xmlrpc
- **`add_field_lists` empty/blank tokens** — `src/xmlrpc/client.rs:268-279`: `Some("")` / `Some("id,,summary")` should filter empty-string elements.
- **Malformed-shape errors** — `src/xmlrpc/client.rs:419-433,362-384,166-184`: struct-where-array → assert "expected attachments array" / "expected comments array" / "expected groups array".
- **Integer/double/base64 parse errors** — `src/xmlrpc/parsing.rs:116-143`: `<int>99999999999999999999</int>`, `<double>not-a-number</double>`, `<base64>@@@</base64>` each assert `Err(XmlRpc)`.

---

## 4. Recommended Complex Functional Sequences

Numbered continuing from existing case 102.

### Case 103 — Full bug lifecycle with state verification at each transition
**API modes:** rest. **Catches:** `client-bug-update-200-error-swallowed`; component default-assignee → `bug.assigned_to`; REOPENED resolution-clearing.
**Steps:** create product `XRefProd` (assert 0/idempotent) → create component `Core --default-assignee admin@test.bzr` → create bug, capture `LB` → view: assert `.assigned_to == admin@test.bzr` → update `--status CONFIRMED` → view: `.status==CONFIRMED` → update `--status RESOLVED --resolution FIXED --comment 'fixed...'` → view: `.status==RESOLVED && .resolution==FIXED` → comment list: last `.text` contains the comment → update `--status REOPENED --resolution ''` → view: `.status==REOPENED && .resolution==''`.

### Case 104 — Group ↔ bug group-permission roundtrip
**API modes:** rest. **Catches:** group-membership ↔ bug-group cross-resource flow; `--groups-add/--groups-remove` roundtrip.
**Steps:** create group `xref-grp` → add-user `testuser@test.bzr` → list-users: assert email present → create bug `GB` → update `--groups-add xref-grp` (requires group exists) → view: `.groups` contains `xref-grp` → remove-user → view: `.groups` STILL contains `xref-grp` (user removal ≠ group removal) → update `--groups-remove xref-grp` → view: `.groups` excludes it.

### Case 105 — Clone preserves description and dependency edges
**API modes:** rest. **Catches:** `commands-bug-clone-missing-comment-zero`, `commands-bug-clone-comment-failure-after-create`, `--add-depends-on` roundtrip.
**Steps:** create source `SRC` with `--description 'UNIQUE-CLONE-BODY-MARKER-7731'` → clone `$SRC --add-depends-on`, capture `CL` → view CL: summary/priority copied → comment list CL: comment `count==0` text contains the marker → view CL: `.depends_on` contains `SRC` → view SRC: `.blocks` contains `CL`.

### Case 106 — Dupe-of transition + relationship persistence
**API modes:** rest. **Catches:** dupe-of reflected in cross-resource views; relationship survival across status transition.
**Steps:** create `TGT`, `SRC` → update SRC `--depends-on-add $TGT` → view SRC: `.depends_on` has TGT → update SRC `--dupe-of $TGT` → view SRC: `.status==RESOLVED && .resolution==DUPLICATE && .dupe_of==TGT` → view TGT: `.blocks` still contains SRC → comment list TGT: length ≥ 1.

### Case 107 — Empty no-op update PUT and `--deadline` garbage validation
**API modes:** rest. **Catches:** `cli-parsing-bug-update-no-fields-empty-put`, `validation-parsing-deadline-unvalidated`.
**Steps:** create `NB`, capture `.last_change_time` T0 → `bug update $NB` (no flags): record exit (desired 7, current 0) → view: `.last_change_time == T0` → update `--deadline 2026-12-31` → view: `.deadline == 2026-12-31` → update `--deadline not-a-date`: assert exit 7 desired, document actual → view: `.deadline == 2026-12-31` (garbage didn't overwrite).

### Case 108 — `query save` mixing `--search` with filters, then run
**API modes:** rest. **Catches:** `cli-parsing-query-save-search-filter-no-conflict`.
**Steps:** `query save mixed-q --search 'Bug one' --product FuncTestProd --status NEW --limit 5` (desired: conflict error; current: 0) → `query show mixed-q --json`: `.kind==search`, note whether `.product`/`.status` persisted → `query run mixed-q`: assert results not constrained by product/status → delete → show fails.

### Case 109 — Template → bug create field inheritance + default-assignee
**API modes:** rest. **Catches:** template→bug inheritance + component default-assignee resolution; CLI-flag-over-template precedence.
**Steps:** create component `TmplComp --default-assignee admin@test.bzr` → `template save chain-tmpl --product FuncTestProd --component TmplComp --priority Normal` → show: component/product set → create `--template chain-tmpl`, capture `TB1` → view: `.component==TmplComp && .priority==Normal && .assigned_to==admin@test.bzr` → create `--template chain-tmpl --priority Highest`, `TB2` → view: `.priority==Highest && .component==TmplComp` → delete template.

### Case 110 — Attachment obsolete transition + atomic comment association
**API modes:** rest. **Catches:** `--obsolete` reflected in later list (re-query gap); atomic attachment+comment by `attachment_id`.
**Steps:** create bug `AB` → upload with `--comment 'ATTACH-COMMENT-MARKER'`, capture `AID` → comment list: comment with `.attachment_id==AID` has the marker → attachment list: `AID.is_obsolete==false` → update `$AID --obsolete true` → attachment list: `AID.is_obsolete==true` → update `--summary renamed` → list: summary changed AND `is_obsolete` still true.

### Case 111 — `bug my --created --cc` skips assigned search
**API modes:** rest. **Catches:** `commands-bug-my-created-and-cc-combo`.
**Steps:** create `MB` (created by admin) → update `--assignee admin@test.bzr` → `bug my`: contains MB → `bug my --created`: contains MB → `bug my --created --cc`: document result (assigned search skipped) → `bug my --all`: contains MB.

### Case 112 — Cross-transport read parity for one mutated bug
**API modes:** rest, hybrid, xmlrpc. **Catches:** `client-bug-hybrid-search-id-fallback-not-tested`; REST/XMLRPC/hybrid read consistency.
**Steps:** create `XB --priority High` → update `--whiteboard multimode-marker --status CONFIRMED` → `--api rest bug view`: whiteboard/status/priority, capture summary → `--api hybrid bug view`: agrees → `--api hybrid bug search --query multimode-marker --fields summary,status`: result has `.id==XB` (id injected on fallback) → `--api xmlrpc comment list`: comment #0 readable.

### Case 113 — Cross-mode field parity (rest/xmlrpc/hybrid)
**API modes:** rest, xmlrpc, hybrid. **Catches:** hybrid id-injection, `xmlrpc-empty-element-events-ignored`, `xmlrpc-int-overflow-non-i64`, `client-bug-get-bug-hybrid-empty-search-fallback`.
**Steps:** create `MBUG --priority High` → `--api rest bug view --fields id,summary,status,priority`: capture → `--api xmlrpc bug view ...`: `.id==MBUG` (id survives despite XML-RPC ignoring `--fields`), summary/priority match → `--api hybrid bug view ...`: summary/priority equal REST values.

### Case 114 — Private comment count parity (rest under-reports, xmlrpc/hybrid full)
**API modes:** rest, hybrid, xmlrpc. **Catches:** private-data fallback differential; `xmlrpc-extract-attachments-wrong-shape-error`, `xmlrpc-empty-element-events-ignored`.
**Steps:** create `PBUG` → comment add public → comment add `--private` → `--api rest comment list`: capture `REST_N`, private count → `--api hybrid`: `HYB_N`, ≥1 private, `HYB_N >= REST_N` → `--api xmlrpc`: `XR_N == HYB_N`, and if `REST_PRIV==0` then `HYB_N > REST_N`.

### Case 115 — Private attachment download empty-data truncation across modes
**API modes:** hybrid, xmlrpc, rest. **Catches:** `client-comment-attachment-download-empty-data-string`, `commands-misc-attachment-download-path-traversal`.
**Steps:** create `ABUG` → write `/tmp/...` known content (record SRC_SIZE) → upload `--private`, capture `PAID` → `--api hybrid download --out /tmp/m-hybrid.txt`: size==SRC_SIZE, content matches → `--api xmlrpc download --out ...`: size==SRC_SIZE, `cmp` equal → `--api rest download --out /tmp/m-rest.txt`: assert exit≠0 OR (exists AND size==SRC_SIZE) — a 0-byte success fails.

### Case 116 — `server info` parity + partial-failure surface
**API modes:** rest, xmlrpc, hybrid. **Catches:** `client-misc-server-info-partial-failure`, `client-auth-version-nonnumeric-version`.
**Steps:** `--api rest server info`: capture `RVER`, extension count `REXT_N` → `--api xmlrpc server info`: `XVER==RVER` → `--api hybrid server info`: `.version==RVER` → `--api rest server info` again: extension count == REXT_N.

### Case 117 — Auto-detect persistence drives follow-on operations
**API modes:** auto, rest, hybrid. **Catches:** `client-auth-version-whoami-unparseable-200`, `client-auth-version-validlogin-network-error`, `client-auth-version-apply-auth-fallback`, `client-misc-whoami-fallback-uncovered`.
**Steps:** `config set-server auto --url --api-key --email` (no auth-method, forces detection) → `--server auto whoami`: `.id` exists → create `AUBUG` → `--server auto bug view`: correct → comment add `--private` → `--server auto comment list` vs `--api hybrid comment list`: `AUTO_N <= HYB_N` and `AUTO_N >= public count`.

### Case 118 — Bug update no-op + 200-error swallowing via read-back (rest+hybrid)
**API modes:** rest, hybrid. **Catches:** `cli-parsing-bug-update-no-fields-empty-put`, `client-bug-update-200-error-swallowed`.
**Steps:** create `UBUG --priority Normal` → view: capture `PRE_PRIO`, `PRE_WB` → `bug update $UBUG` (no flags): exit 7 OR no-op → view: priority/whiteboard unchanged → update `--status RESOLVED --resolution FIXED --comment` → `--api hybrid bug view`: status/resolution committed (read-back catches success-message vs unchanged-state divergence).

### Case 119 — Clone non-atomicity + `--no-comment` description carry
**API modes:** rest, hybrid. **Catches:** `commands-bug-clone-comment-failure-after-create`, `commands-bug-clone-missing-comment-zero`.
**Steps:** create `CSRC --description 'ORIGINAL-DESC-MARKER'` → clone, `CDST` → view: summary/priority copied → `--api hybrid comment list CDST`: comment #0 text contains marker → comment list CDST: a "Cloned from $CSRC" comment exists.

### Case 120 — `create_user` mode routing convergence
**API modes:** rest, xmlrpc, hybrid. **Catches:** `client-misc-create-user-hybrid-xmlrpc-uncovered`.
**Steps:** `--api rest user create matrixuser@test.bzr ...`: 0 or "already" → `--api xmlrpc user create` (same): non-zero "already" → `--api hybrid user create` (same): non-zero "already" (no spurious success) → `user search matrixuser`: exactly one result.

### Case 121 — Saved search-kind query silently drops filters (save→run→compare)
**API modes:** rest. **Catches:** `cli-parsing-query-save-search-filter-no-conflict`, `client-bug-search-no-limit-no-param`.
**Steps:** `query save mixed-q --search Matrix --product NonexistentProductXYZ --status NEW --limit 50` (reject desired / accept current) → show: `.kind==search`, note `.product` → `query save pure-q --search Matrix --limit 50` → run mixed-q: capture sorted ids → run pure-q: capture sorted ids → assert `MIXED_IDS == PURE_IDS` (product silently ignored) → delete both.

### Case 122 — `--limit 0` unbounded semantics via saved query
**API modes:** rest. **Catches:** `client-bug-search-no-limit-no-param`, `validation-parsing-url-limit-silently-dropped`.
**Steps:** `query save limit-q --product FuncTestProd --status NEW,CONFIRMED,RESOLVED --limit 1` → run: length==1 → run `--limit 2`: capture `TWO_N` → run `--limit 0`: capture `ZERO_N`, assert `ZERO_N >= TWO_N` (0 = unbounded, not empty) → delete.

### Case 123 — Deadline validation parity with read-back across modes
**API modes:** rest, hybrid. **Catches:** `validation-parsing-deadline-unvalidated`.
**Steps:** create `DBUG` → update `--deadline not-a-date`: assert exit 7 desired / document → view: `.deadline` not 'not-a-date' → update `--deadline 2027-03-15` → `--api rest bug view`: `.deadline==2027-03-15` → `--api hybrid bug view`: matches.

### Case 124 — Attachment boolean-field parity across modes after update
**API modes:** rest, xmlrpc, hybrid. **Catches:** `xmlrpc-empty-element-events-ignored`, `client-comment-attachment-update-noop-params`.
**Steps:** create `MABUG` → upload, `MAID` → update `--is-patch true --obsolete true` → `--api rest attachment list`: capture `is_patch/is_obsolete/is_private` → `--api xmlrpc list`: booleans match → `--api hybrid list`: booleans match.

### Case 125 — Group membership parity across modes after mutation
**API modes:** rest, hybrid, xmlrpc. **Catches:** `xmlrpc-extract-attachments-wrong-shape-error`, `client-misc-create-user-hybrid-xmlrpc-uncovered`.
**Steps:** create group `matrix-grp` → create user `grpmatrix@test.bzr` → add-user → `--api rest group view`: membership contains user → `--api hybrid group view`: contains user → remove-user → `--api rest group view`: excludes user.

### Case 126 — Config xmlrpc/query_param persistence survives reload and drives a call
**API modes:** xmlrpc, auto. **Catches:** `config-apimode-xmlrpc-serde-roundtrip`, `error-exitcodes-xmlrpc-mapping-untested`.
**Steps:** `config set-server xrserver --auth-method query_param ...` → `config show`: reflects query_param → create `XRBUG` → comment add `--private` → `--server xrserver --api xmlrpc comment list`: ≥1 private (persisted config authenticates, xmlrpc routes) → `--server xrserver whoami`: `.id` exists.

### Case 127 — Create idempotency invariance (product + group)
**API modes:** rest. **Catches:** create idempotency non-mutation (product, group); second create must not overwrite.
**Steps:** product create `IdemProd --description 'first desc'`, capture `PROD_ID` → product create `IdemProd --description 'SECOND desc'`: "already exists" or 0, no new product → product view: `.id==PROD_ID`, `.description=='first desc'` → group create `idem-grp --is-active true` → group create `idem-grp --is-active false`: duplicate handling → group view: `is_active` reflects FIRST create.

### Case 128 — Group member remove/re-add cycle with no intervening disable
**API modes:** rest, hybrid. **Catches:** add/remove true-inverse + idempotency; no duplicate membership; non-member remove exit stability.
**Steps:** create user `cycle@test.bzr` + group `cycle-grp` → add-user → list-users `--details`: present → add-user again: idempotent → list: appears exactly once → remove-user → list: absent → remove-user again: stable exit, no crash → add-user → list: restored.

### Case 129 — Saved query delete→recreate state isolation
**API modes:** rest. **Catches:** delete leaves no residue (re-save = "saved" not "updated"); show/run exit-code consistency.
**Steps:** `query save lifecycle-q --product FuncTestProd --status NEW --limit 7` → show: limit 7, product set → delete: `.action==deleted` → show: assert_failure, record exit → run: same failure exit → `query save lifecycle-q --search 'Clone source bug' --limit 3`: `.action==saved` (fresh, not "updated") → show: `.kind==search`, `.limit==3`, `.product` absent.

### Case 130 — Batch update partial failure (exit 11) commits valid ids only
**API modes:** rest. **Catches:** `error-exitcodes-batchpartial-errortype-untested`, `commands-bug-update-batch-empty-comment-suffix-on-failure`.
**Steps:** create `BVALID` → `bug update $BVALID 999999 --whiteboard partial-batch`: exit 11, `.succeeded` has BVALID, `.failed` has 999999, `.type=='batch_partial_failure'` → view BVALID: `.whiteboard=='partial-batch'` (valid leg committed) → `bug update 999998 999999 --whiteboard all-fail`: empty succeeded, 2 failed → `bug update $BVALID 999999 --whiteboard wb --comment 'batch comment'`: exit 11, view: whiteboard updated AND comment landed.

### Case 131 — Dangling default-server resolution after underlying server removed
**API modes:** rest, auto. **Catches:** dangling-default resolution; no silent fallback; recovery.
**Steps:** `config set-server temp-default ...` → `config set-default temp-default` → `config show`: default==temp-default → `whoami`: works → remove `[servers.temp-default]` from config.toml in place (keep `default="temp-default"`) → `config show`: defined behavior (exit 3 or list remaining), record → `whoami`: exit 3 (must NOT fall through to another server) → `config set-default test` → `whoami`: recovered.

### Case 132 — TOFU pin → issuer change (exit 13) → clear recovery
**API modes:** rest. **Catches:** PinMismatch/IssuerChanged exit 13 end-to-end; `config-set-server-tls-pin-clear` success and not-found branches; `tls-keyring-issuer-with-comma-pin-mismatch`.
**Steps (gate on https endpoint with swappable cert, else skip):** `config set-server tofu --url $HTTPS_URL --tls-pin-sha256 <A>` → `--server tofu server info`: ok → swap cert to different issuer → `--server tofu server info`: exit 13, stderr names issuer change/pin mismatch → `config set-server tofu --tls-pin-clear`: stderr "Certificate pin cleared" → `--server tofu server info`: documented behavior (accept or re-pin), no crash → `config set-server doesnotexist --tls-pin-clear`: config error "not found — nothing to clear".

### Case 133 — Alias global-uniqueness collision and rejected-update atomicity
**API modes:** rest. **Catches:** alias collision across two bugs; atomicity of rejected alias update (co-fields not partially applied); same-alias-same-bug idempotency.
**Steps:** create `BA`, `BB` → update BA `--alias uniq-alias-001` → view BA: alias set → update BA `--alias uniq-alias-001 --whiteboard wb1`: idempotent, succeeds → view BA: `.whiteboard==wb1` → update BB `--alias uniq-alias-001 --whiteboard wb-should-not-apply`: assert_failure → view BB: alias NOT set AND whiteboard NOT 'wb-should-not-apply' (rejected update was atomic).

---

## 5. Prioritized Next Actions (top 10, impact-to-effort)

1. **Route `put_json` through 200-error detection** (`client-bug-update-200-error-swallowed`). Highest-impact correctness fix: silently-reported failed mutations affect `bug update`, `attachment update`, batch updates. Add the wiremock 200-error-envelope test. *Low effort, high impact.*

2. **Add the empty-update guard** (`cli-parsing-bug-update-no-fields-empty-put`). Mirror the existing `query`/`template` "at least one field" pattern; one guard + one test. Eliminates a silent no-op PUT with a false success message. *Low effort.*

3. **Sanitize attachment download filenames** (`commands-misc-attachment-download-path-traversal`). Take basename only (reject embedded separators / absolute / `..`) on both single and batch paths. Security-relevant write-outside-cwd. *Low effort.*

4. **Fix `mask_api_key` and `truncate` Unicode/bound handling** (`output-mask-api-key-non-ascii-panic`, `output-truncate-small-bound-underflow`). `chars().take(8)` and `saturating_sub(3)`; `config show` can panic today. *Trivial effort.*

5. **Handle `Event::Empty` in XML-RPC parsing** (`xmlrpc-empty-element-events-ignored`). Either set `expand_empty_elements(true)` on the quick-xml reader (smallest change, fixes all loops at once) or add `Event::Empty` arms. Real EOF/wrong-type failures against servers emitting `<value/>`. *Low effort, broad fix.*

6. **Wire `parse_optional_date` into `--deadline`** (`validation-parsing-deadline-unvalidated`). One call + functional test mirroring case 45b; restores consistency with every other date flag. *Low effort.*

7. **Add `conflicts_with_all` to `query save --search`** (`cli-parsing-query-save-search-filter-no-conflict`). Match `--from-url`; aligns clap with the docs and removes contradictory stored state. *Low effort.*

8. **Add the error-code contract test** (`error-exitcodes-no-distinctness-guard` + the 4 unasserted variants + `error-exitcodes-stale-range-comment`). One table-driven test covers all 18 variants, exit-code multiset, and the missing `Http`/`XmlRpc`/`TomlSerialize`/`BatchPartialFailure` assertions; fix the stale `1..=12` comment. *Low effort, locks the whole subsystem.*

9. **Cover the `--tls-pin-clear` and config-validation branches** (`config-set-server-tls-pin-clear`, `commands-misc-config-mutual-exclusion-uncovered`, `config-apimode-xmlrpc-serde-roundtrip`). Security/persistence-critical config paths with zero coverage; pure test additions (plus deciding whether `--tls-pin-clear` should honor `--output json`). *Medium effort.*

10. **Land the cross-transport matrix functional sequences** (cases 112–116, 124, 125). The single largest coverage theme; one new functional helper that reads the same mutated resource through rest/xmlrpc/hybrid catches the bulk of XML-RPC parsing and hybrid-fallback risk. *Medium effort, high impact.*