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
rm localconfig
perl checksetup.pl
printf '%s\n' "\$db_driver = \"mysql\";" "\$db_host = \"localhost\";" "\$db_name = \"bugs\";" \
    "\$db_user = \"bugs\";" "\$db_pass = \"bugzilla\";" "\$webservergroup = \"apache\";" >> localconfig
printf '%s\n' "\$answer{'ADMIN_EMAIL'} = 'admin@test.bzr';" "\$answer{'ADMIN_PASSWORD'} = 'FuncTest1!';" "\$answer{'ADMIN_REALNAME'} = 'Admin User';" "\$answer{'ext_logins'} = '';" "\$answer{'NO_PAUSE'} = 1;" > answers.txt
# RedHat reads core tables while its extension schema loads. Build the core schema first.
mkdir -p /var/www/html/bugzilla/data
touch /var/www/html/bugzilla/data/bz.log /var/www/html/bugzilla/extensions/RedHat/info.log
if ! perl checksetup.pl --update-db answers.txt; then
    echo '==> RHBZ schema migration requires a second pass'
    mysql -u root bugs -e 'ALTER TABLE report_groups MODIFY report_id MEDIUMINT NOT NULL, MODIFY group_id MEDIUMINT NOT NULL; ALTER TABLE products ADD COLUMN rule_group MEDIUMINT NULL, ADD COLUMN report_group MEDIUMINT NULL;'
fi
perl checksetup.pl --update-db answers.txt
perl checksetup.pl --update-db answers.txt
perl checksetup.pl answers.txt
exec httpd -D FOREGROUND
