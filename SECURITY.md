# Security Policy

## Supported Versions

`bzr` routinely provides security fixes only on the latest stable minor release
line. Older minor lines are unsupported. If an older version is affected, its
range remains named in the advisory; users should normally upgrade to the first
fixed release rather than expect a backport. A support exception must be stated
explicitly for that advisory and does not change this rolling policy.

## Advisory publication

[GitHub Security Advisories](https://github.com/randomparity/bzr/security/advisories)
are the canonical public inventory for `bzr` vulnerabilities. A project runtime
vulnerability advisory records:

- affected `bzr` version ranges, including affected unsupported versions;
- the first fixed version;
- the runtime impact;
- a public identifier and advisory link; and
- upgrade guidance.

Dependency-only advisories are identified separately. They remain useful
security information, but they are not `bzr` runtime vulnerabilities and do not
satisfy project-vulnerability disclosure requirements.

## Reporting a Vulnerability

If you discover a security vulnerability in bzr, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Email [randomparity@gmail.com](mailto:randomparity@gmail.com) with:
   - Description of the vulnerability
   - Steps to reproduce
   - Affected version(s)
3. You will receive a response within 72 hours acknowledging your report
4. A fix will be developed privately and released as a patch version

## Scope

Security issues in bzr itself and its direct dependencies are in scope. Issues
in upstream Bugzilla servers are out of scope — report those to the Bugzilla
project directly.
