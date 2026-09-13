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
rm -f localconfig
perl checksetup.pl
printf '%s\n' "\$db_driver = \"mysql\";" "\$db_host = \"localhost\";" "\$db_name = \"bugs\";" \
    "\$db_user = \"bugs\";" "\$db_pass = \"bugzilla\";" "\$webservergroup = \"apache\";" >> localconfig
printf '%s\n' "\$answer{'ADMIN_EMAIL'} = 'admin@test.bzr';" "\$answer{'ADMIN_PASSWORD'} = 'FuncTest1!';" "\$answer{'ADMIN_REALNAME'} = 'Admin User';" "\$answer{'ext_logins'} = '';" "\$answer{'NO_PAUSE'} = 1;" > answers.txt
# RedHat reads core tables while its extension schema loads. Build the core schema first.
mkdir -p /var/www/html/bugzilla/data
touch /var/www/html/bugzilla/data/bz.log /var/www/html/bugzilla/extensions/RedHat/info.log
chown -R apache:apache /var/www/html/bugzilla/data
chown apache:apache /var/www/html/bugzilla/extensions/RedHat/info.log
if ! perl checksetup.pl --update-db answers.txt; then
    echo '==> RHBZ schema migration requires a second pass'
    mysql -u root bugs -e "ALTER TABLE report_groups MODIFY report_id MEDIUMINT NOT NULL, MODIFY group_id MEDIUMINT NOT NULL; ALTER TABLE products ADD COLUMN rule_group MEDIUMINT NULL, ADD COLUMN report_group MEDIUMINT NULL; CREATE TABLE IF NOT EXISTS workflow_groupreq (regex varchar(255)); INSERT INTO groups (name, description, isbuggroup, userregexp, isactive) VALUES ('admin','RHBZ bootstrap group',0,'',1),('all_partners','RHBZ bootstrap group',0,'',1),('devel','RHBZ bootstrap group',0,'',1),('ecs','RHBZ bootstrap group',0,'',1),('epm','RHBZ bootstrap group',0,'',1),('gss_manager','RHBZ bootstrap group',0,'',1),('ibm_storage','RHBZ bootstrap group',0,'',1),('ibm_storage_devel','RHBZ bootstrap group',0,'',1),('ibm_storage_product_mgt_staff','RHBZ bootstrap group',0,'',1),('ibm_storage_program_staff','RHBZ bootstrap group',0,'',1),('ibm_storage_qe','RHBZ bootstrap group',0,'',1),('ibm_storage_support_staff','RHBZ bootstrap group',0,'',1),('product_management','RHBZ bootstrap group',0,'',1),('program_management','RHBZ bootstrap group',0,'',1),('psirt_staff','RHBZ bootstrap group',0,'',1),('qa','RHBZ bootstrap group',0,'',1),('redhat','RHBZ bootstrap group',0,'',1),('rhn','RHBZ bootstrap group',0,'',1),('secalert_grant','RHBZ bootstrap group',0,'',1),('security','RHBZ bootstrap group',0,'',1) ON DUPLICATE KEY UPDATE name=VALUES(name);"
fi
perl checksetup.pl --update-db answers.txt
perl checksetup.pl --update-db answers.txt
perl checksetup.pl answers.txt
api_key=$(BZR_API_KEY='FuncTest0123456789abcdef0123456789abcdef' perl -I. -MBugzilla -MBugzilla::Constants=PASSWORD_DIGEST_ALGORITHM -MBugzilla::Util=bz_crypt -e 'print bz_crypt($ENV{BZR_API_KEY}, Bugzilla->localconfig->{site_wide_secret}, PASSWORD_DIGEST_ALGORITHM)')
mysql -u root bugs -e "INSERT IGNORE INTO user_api_keys (user_id, api_key, description, revoked) SELECT userid, '$api_key', 'functional-test', 0 FROM profiles WHERE login_name = 'admin@test.bzr';"
mysql -u root bugs -e "INSERT IGNORE INTO user_group_map (user_id, group_id, isbless, grant_type) SELECT p.userid, g.id, 0, 0 FROM profiles p JOIN groups g WHERE p.login_name = 'admin@test.bzr' AND g.name IN ('admin', 'devel', 'editbugs', 'editcomponents', 'qa', 'redhat');"
exec httpd -D FOREGROUND
