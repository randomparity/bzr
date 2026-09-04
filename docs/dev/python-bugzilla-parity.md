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
| Comment tags and minor update | `bzr bug update --comment-tag --minor-update` | expected gap (#672) | `compare/01-bug-lifecycle/update-options` |
| Whiteboard match types | `bzr bug list --status-whiteboard-type` | expected gap (#679) | `compare/01-bug-lifecycle/query-match-types` |
| Personal bug tags | `bzr bug tag`, `bzr bug list --tag` | expected gap (#680) | `compare/01-bug-lifecycle/bug-tags` |
