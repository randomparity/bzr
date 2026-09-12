#!/bin/bash
set -euo pipefail

/usr/libexec/mysqld --user=mysql --datadir=/var/lib/mysql &
for i in $(seq 1 30); do
    if mysqladmin ping --silent; then
        echo "==> MariaDB ready after ${i}s"
        break
    fi
    sleep 1
done
mysqladmin ping --silent
mysql -u root -e "CREATE DATABASE IF NOT EXISTS bugs; GRANT ALL ON bugs.* TO 'bugs'@'localhost' IDENTIFIED BY 'bugzilla'; FLUSH PRIVILEGES;"
cd /var/www/html/bugzilla
printf '%s\n' "\$answer{'ADMIN_EMAIL'} = 'admin@test.bzr';" "\$answer{'ADMIN_PASSWORD'} = 'FuncTest1!';" "\$answer{'ADMIN_REALNAME'} = 'Admin User';" "\$answer{'NO_PAUSE'} = 1;" > answers.txt
perl checksetup.pl answers.txt
exec httpd -D FOREGROUND
