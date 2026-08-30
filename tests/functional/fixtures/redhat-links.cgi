#!/usr/bin/perl
use strict;
use warnings;

print "Content-Type: application/json\r\n\r\n";

my $path = $ENV{PATH_INFO} // '';
if ($path eq '/rest/version') {
    print '{"version":"5.0.0"}';
    exit 0;
}

if ($path ne '/rest/bug') {
    print '{"error":true,"message":"unknown fixture endpoint"}';
    exit 0;
}

my %requested = map { $_ => 1 }
    (($ENV{QUERY_STRING} // '') =~ /(?:^|&)id=(\d+)/g);
my %bugs = (
    998 => '{"id":998,"summary":"Red Hat duplicate root","status":"CLOSED",'
        . '"duplicates":[{"bug_id":1117050,"summary":"vendor object"}]}',
    1117050 => '{"id":1117050,"summary":"Red Hat duplicate child","status":"CLOSED",'
        . '"duplicates":[{"bug_id":1200000}]}',
    1200000 => '{"id":1200000,"summary":"Red Hat duplicate leaf","status":"CLOSED",'
        . '"duplicates":[]}',
);
my @matches = map { $bugs{$_} } grep { $requested{$_} } sort { $a <=> $b } keys %bugs;
print '{"bugs":[' . join(',', @matches) . ']}';
