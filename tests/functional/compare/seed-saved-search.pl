use strict;
use warnings;

use Bugzilla;
use Bugzilla::User;

my ($login, $name, $query) = @ARGV;
die "expected LOGIN NAME QUERY\n" unless defined $query && @ARGV == 3;

my $user = Bugzilla::User->check({ name => $login });
my $dbh = Bugzilla->dbh;
my $user_id = $user->id;

$dbh->do(
    q{INSERT INTO namedqueries (userid, name, query) VALUES (?, ?, ?)
      ON DUPLICATE KEY UPDATE query = VALUES(query)},
    undef,
    $user_id,
    $name,
    $query,
);
my $stored = $dbh->selectrow_hashref(
    q{SELECT userid, name, query FROM namedqueries WHERE userid = ? AND name = ?},
    undef,
    $user_id,
    $name,
);
die "saved search readback failed\n"
    unless $stored
    && $stored->{userid} == $user_id
    && $stored->{name} eq $name
    && $stored->{query} eq $query;
