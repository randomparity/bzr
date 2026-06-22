# 02-server-auth
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 2: Server & Auth
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 2: Server & Auth ──────────────────────────────────"

test_begin "7. server info"
run_bzr server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "8. whoami"
run_bzr whoami
if assert_success && assert_json_exists '.id'; then test_pass; fi

test_begin "8a. --server auto whoami"
run_bzr_raw --json --server auto whoami
if assert_success && assert_json_exists '.id'; then test_pass; fi

test_begin "8b. fixture flag types exist"
_FLAG_SQL=$(mktemp /tmp/bzr-func-flags.XXXXXX.sql)
cat >"$_FLAG_SQL" <<'SQL'
INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'review', 'Functional test review flag for bugs', 'b', 1, 1, 1, 1, 10
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'review' AND target_type = 'b'
);

INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'review', 'Functional test review flag for attachments', 'a', 1, 1, 1, 1, 10
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'review' AND target_type = 'a'
);

INSERT INTO flaginclusions (type_id, product_id, component_id)
SELECT id, NULL, NULL
FROM flagtypes
WHERE name = 'review'
  AND target_type IN ('b', 'a')
  AND NOT EXISTS (
      SELECT 1 FROM flaginclusions WHERE flaginclusions.type_id = flagtypes.id
  );
SQL
if run_bugzilla_sql_file "$_FLAG_SQL"; then
    test_pass
else
    test_fail "could not seed functional flag types"
fi
rm -f "$_FLAG_SQL"
unset _FLAG_SQL

echo ""
