#!/bin/bash
set -euo pipefail

BZ_DIR=/var/www/html/bugzilla
API_KEY="FuncTest0123456789abcdef0123456789abcdef"

echo "==> Starting MariaDB..."
/usr/libexec/mysqld --user=mysql --datadir=/var/lib/mysql &
MYSQL_PID=$!

# Wait for MariaDB socket (up to 30s)
for i in $(seq 1 30); do
    if mysqladmin ping --silent 2>/dev/null; then
        echo "==> MariaDB ready after ${i}s"
        break
    fi
    sleep 1
done

if ! mysqladmin ping --silent 2>/dev/null; then
    echo "FATAL: MariaDB did not start within 30 seconds"
    exit 1
fi

# ── Create DB and user ──────────────────────────────────────────────
echo "==> Creating database and user..."
mysql -u root <<'SQL'
CREATE DATABASE IF NOT EXISTS bugs CHARACTER SET utf8;
GRANT ALL ON bugs.* TO 'bugs'@'localhost' IDENTIFIED BY 'bugzilla';
FLUSH PRIVILEGES;
SQL

# ── Run checksetup.pl ────────────────────────────────────────────────
# Bugzilla master (5.3+) checksetup needs multiple passes:
#   1) Generate/update localconfig fields (exits early with "Please edit")
#   2) May still need localconfig updates — run again to accept
#   3) Create schema + admin user
#   4) Finalize
cd "$BZ_DIR"

# Bugzilla needs data/ to exist before checksetup can proceed
mkdir -p data

echo "==> Running checksetup.pl (pass 1 — generate localconfig)..."
perl checksetup.pl answers.txt 2>&1 | tail -5 || true

echo "==> Running checksetup.pl (pass 2 — accept localconfig)..."
perl checksetup.pl answers.txt 2>&1 | tail -5 || true

echo "==> Running checksetup.pl (pass 3 — schema)..."
perl checksetup.pl answers.txt 2>&1 | tail -5

echo "==> Running checksetup.pl (pass 4 — finalize)..."
perl checksetup.pl answers.txt 2>&1 | tail -5

# ── Enable use_email_as_login (required for user create API on 5.3+) ──
# Bugzilla stores params in data/params (Perl hash) or data/params.json
echo "==> Enabling use_email_as_login..."
cd "$BZ_DIR"
if [[ -f data/params.json ]]; then
    perl -pi -e 's/"use_email_as_login"\s*:\s*0/"use_email_as_login":1/g' data/params.json 2>/dev/null || true
fi
if [[ -f data/params ]]; then
    perl -pi -e "s/'use_email_as_login' => '0'/'use_email_as_login' => '1'/g" data/params 2>/dev/null || true
    perl -pi -e "s/'use_email_as_login' => 0/'use_email_as_login' => 1/g" data/params 2>/dev/null || true
fi

# ── Insert API key for admin user ────────────────────────────────────
echo "==> Inserting API key..."
mysql -u root bugs <<SQL
INSERT IGNORE INTO user_api_keys (user_id, api_key, description, revoked)
SELECT userid, '${API_KEY}', 'functional-test', 0
FROM profiles
WHERE login_name = 'admin@test.bzr'
LIMIT 1;
SQL

# ── Fix permissions ──────────────────────────────────────────────────
chown -R apache:apache "$BZ_DIR/data" "$BZ_DIR/lib" 2>/dev/null || true

# ── Start Apache ──────────────────────────────────────────────────────
# Start directly in foreground — the external health check in
# setup-bugzilla.sh verifies the REST API is working.
echo "==> Starting Apache in foreground..."
exec httpd -D FOREGROUND
