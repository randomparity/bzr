# Python-bugzilla parity

This report tracks bzr against python-bugzilla 3.3.0 using stable comparison test IDs.

| Capability | bzr equivalent | Status | Evidence test ID |
| --- | --- | --- | --- |
| Product listing | `bzr product list` | parity | `compare/00-products/list-products` |
| Bug create and first description | `bzr bug create`, `bzr comment list`, `bzr bug view` | parity | `compare/01-bug-lifecycle/create` |
| Bug query | `bzr bug list` | parity | `compare/01-bug-lifecycle/query` |
| Bug update | `bzr bug update` | parity | `compare/01-bug-lifecycle/update` |
| Bug view | `bzr bug view` | parity | `compare/01-bug-lifecycle/view` |
| Bug history | `bzr bug history` | parity | `compare/01-bug-lifecycle/history` |
| Server saved search | `bzr bug search --saved-search` | expected gap (#670) | `compare/01-bug-lifecycle/saved-search` |
| Generic arbitrary fields | `bzr bug create/update --field` | expected gap (#671) | `compare/01-bug-lifecycle/arbitrary-fields` |
| Comment tags and minor update | `bzr bug update --comment-tag --minor-update` | comment tags: parity; minor update — bz50/bz52: warns (no core support, mail sent anyway); bz53: parity | `compare/01-bug-lifecycle/update-options` |
| Whiteboard match types | `bzr bug list --status-whiteboard-type` | expected gap (#679) | `compare/01-bug-lifecycle/query-match-types` |
| Personal bug tags | `bzr bug tag`, `bzr bug list --tag` | expected gap (#680) | `compare/01-bug-lifecycle/bug-tags` |
| Public comments | `bzr comment add`, `bzr comment list` | parity | `compare/02-comments/public-comments` |
| Private comments over REST | `bzr comment add --private`, `bzr comment list` | parity | `compare/02-comments/private-comments-rest` |
| Private comments over XML-RPC | `bzr comment add --private`, `bzr comment list` | parity | `compare/02-comments/private-comments-xmlrpc` |
| Attachment upload metadata and comment | `bzr attachment upload`, `bzr attachment list`, `bzr comment list` | parity | `compare/03-attachments/upload-metadata-comment` |
| Attachment download content | `bzr attachment download` | parity | `compare/03-attachments/download-content` |
| Attachment flags | `bzr attachment update --flag` | parity | `compare/03-attachments/attachment-flags` |
| Private attachments over REST | `bzr attachment list/view/download` | parity | `compare/03-attachments/private-attachments-rest` |
| Private attachments over XML-RPC | `bzr attachment list/view/download` | parity | `compare/03-attachments/private-attachments-xmlrpc` |
| Multi-bug attachment upload | `bzr attachment upload` | expected gap (#674) | `compare/03-attachments/multi-bug-upload` |
| Ignore obsolete attachments | `bzr attachment download --bug --ignore-obsolete` | expected gap (#674) | `compare/03-attachments/ignore-obsolete` |
| User create, get, and search | `bzr user create`, `bzr user search` | parity | `compare/04-users-groups/user-create-get-search` |
| Group get and list | `bzr group view` | parity | `compare/04-users-groups/group-get-and-list` |
| Membership add and remove | `bzr group add-user/remove-user`, `bzr user search` | parity | `compare/04-users-groups/membership-add-remove` |
| Product catalogues | `bzr product list --type` | parity | `compare/05-products-components/product-catalogues` |
| Component create | `bzr component create`, `bzr component view` | parity | `compare/05-products-components/component-create` |
| Red Hat component update | `bzr component update` | expected gap (#675) | `compare/05-products-components/component-update-redhat` |
| API-key placement by server version | `bzr whoami` | bz50/bz52: both query; bz53: bzr header, python-bugzilla query | `compare/06-auth-config-tls/api-key-placement` |
| Restricted password login | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/restricted-login` |
| Cached login token reuse | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/cached-token` |
| Logout token invalidation | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/logout` |
| bugzillarc three-file precedence | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/bugzillarc-precedence` |
| bugzillarc default URL | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/bugzillarc-default-url` |
| bugzillarc URL-substring section | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/bugzillarc-substring-section` |
| Disable TLS verification | `--server-tls-insecure` | parity | `compare/06-auth-config-tls/nosslverify` |
| Login-token request transport | no equivalent | expected gap (#676) | `compare/06-auth-config-tls/token-transport-gap` |
| Login and logout commands | no equivalent | expected gap (#681) | `compare/06-auth-config-tls/login-command-gap` |
| bugzillarc import | no equivalent | expected gap (#682) | `compare/06-auth-config-tls/bugzillarc-import-gap` |
| Client certificate configuration | no equivalent | surface gap (#677) | `compare/06-auth-config-tls/client-certificate-surface-gap` |
| Red Hat Bearer API-key transport | no equivalent | expected gap (#678) | `compare/06-auth-config-tls/bearer-gap` |
