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

# ── Seed functional-test keywords ───────────────────────────────────
echo "==> Seeding functional-test keywords..."
mysql -u root bugs <<'SQL'
INSERT IGNORE INTO keyworddefs (name, description)
VALUES ('fix-needed', 'Functional test keyword');
SQL

# ── Configure insidergroup ──────────────────────────────────────────
# Bugzilla's default `insidergroup` is empty, which forbids anyone
# (including admins) from marking comments private. Real deployments
# that exercise issue #125 have this configured (otherwise private
# comments wouldn't exist there to be hidden). Set it to `admin` so
# the test admin user can post and read private comments.
echo "==> Configuring insidergroup..."
cd "$BZ_DIR"
if [[ -f data/params.json ]]; then
    perl -pi -e 's/"insidergroup"\s*:\s*""/"insidergroup":"admin"/g' data/params.json 2>/dev/null || true
fi
if [[ -f data/params ]]; then
    perl -pi -e "s/'insidergroup' => ''/'insidergroup' => 'admin'/g" data/params 2>/dev/null || true
fi

# ── Disable outbound mail for functional tests ──────────────────────
# Comment and attachment mutations trigger bugmail. The functional
# containers do not run an MTA, so use Bugzilla's built-in no-op mailer.
echo "==> Disabling outbound mail..."
if [[ -f data/params.json ]]; then
    perl -pi -e 's/"mail_delivery_method"\s*:\s*"[^"]*"/"mail_delivery_method":"None"/g' data/params.json 2>/dev/null || true
fi
if [[ -f data/params ]]; then
    perl -pi -e "s/'mail_delivery_method' => '[^']*'/'mail_delivery_method' => 'None'/g" data/params 2>/dev/null || true
fi

# ── Fix permissions ──────────────────────────────────────────────────
chown -R apache:apache "$BZ_DIR/data" "$BZ_DIR/lib" 2>/dev/null || true

# ── Start Apache ──────────────────────────────────────────────────────
# Start directly in foreground — the external health check in
# setup-bugzilla.sh verifies the REST API is working.
echo "==> Starting Apache in foreground..."
exec httpd -D FOREGROUND
