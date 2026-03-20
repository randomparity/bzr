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

# ── Run checksetup.pl (creates schema + admin user) ─────────────────
echo "==> Running checksetup.pl (first pass — schema)..."
cd "$BZ_DIR"
perl checksetup.pl answers.txt 2>&1 | tail -5

echo "==> Running checksetup.pl (second pass — finalize)..."
perl checksetup.pl answers.txt 2>&1 | tail -5

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

# ── Verify REST API works ────────────────────────────────────────────
echo "==> Starting Apache..."
httpd -k start 2>&1 || true
sleep 1

# Quick self-test
if curl -sf http://localhost/rest/version >/dev/null 2>&1; then
    echo "==> REST API self-test passed"
else
    echo "WARN: REST API self-test failed (may still work from outside)"
fi

# Stop the background Apache and restart in foreground
httpd -k stop 2>/dev/null || true
sleep 1

echo "==> Starting Apache in foreground..."
exec httpd -D FOREGROUND
