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
| Server saved search | `bzr bug search --saved-search` | stock: bzr errors, python-bugzilla returns unfiltered results (#670); Red-Hat-shaped proxy: bzr filters | `compare/01-bug-lifecycle/saved-search` |
| Generic arbitrary fields | `bzr bug create/update --field` | parity | `compare/01-bug-lifecycle/arbitrary-fields` |
| Comment tags and minor update | `bzr bug update --comment-tag --minor-update` | comment tags: parity; minor update — bz50/bz52: warns (no core support, mail sent anyway); bz53: parity | `compare/01-bug-lifecycle/update-options` |
| Whiteboard match types | `bzr bug list --status-whiteboard-type` | supported | `compare/01-bug-lifecycle/query-match-types` |
| Personal bug tags | `bzr bug tag`, `bzr bug list --tag` | parity | `compare/01-bug-lifecycle/bug-tags` |
| Public comments | `bzr comment add`, `bzr comment list` | parity | `compare/02-comments/public-comments` |
| Private comments over REST | `bzr comment add --private`, `bzr comment list` | parity | `compare/02-comments/private-comments-rest` |
| Private comments over XML-RPC | `bzr comment add --private`, `bzr comment list` | parity | `compare/02-comments/private-comments-xmlrpc` |
| Attachment upload metadata and comment | `bzr attachment upload`, `bzr attachment list`, `bzr comment list` | parity | `compare/03-attachments/upload-metadata-comment` |
| Attachment download content | `bzr attachment download` | parity | `compare/03-attachments/download-content` |
| Attachment flags | `bzr attachment update --flag` | parity | `compare/03-attachments/attachment-flags` |
| Private attachments over REST | `bzr attachment list/view/download` | parity | `compare/03-attachments/private-attachments-rest` |
| Private attachments over XML-RPC | `bzr attachment list/view/download` | parity | `compare/03-attachments/private-attachments-xmlrpc` |
| Multi-bug attachment upload | `bzr attachment upload` | parity | `compare/03-attachments/multi-bug-upload` |
| Ignore obsolete attachments | `bzr attachment download --bug --ignore-obsolete` | parity | `compare/03-attachments/ignore-obsolete` |
| User create, get, and search | `bzr user create`, `bzr user search` | parity | `compare/04-users-groups/user-create-get-search` |
| Group get and list | `bzr group view` | parity | `compare/04-users-groups/group-get-and-list` |
| Membership add and remove | `bzr group add-user/remove-user`, `bzr user search` | parity | `compare/04-users-groups/membership-add-remove` |
| Product catalogues | `bzr product list --type` | parity | `compare/05-products-components/product-catalogues` |
| Component create | `bzr component create`, `bzr component view` | parity | `compare/05-products-components/component-create` |
| RHBZ ExternalBugs add | no equivalent | expected gap (#774) | `compare/08-rhbz-externalbugs/add` |
| RHBZ ExternalBugs update | no equivalent | expected gap (#774) | `compare/08-rhbz-externalbugs/update` |
| RHBZ ExternalBugs remove | no equivalent | expected gap (#774) | `compare/08-rhbz-externalbugs/remove` |
| RHBZ component update | `bzr component update` | expected gap (#774) | `compare/08-rhbz-externalbugs/component-update` |
| RHBZ sub-components | `bzr bug update --field-json -` with `rh_sub_components` | parity | `compare/09-rhbz-fields/sub-components` |
| RHBZ target release | `bzr bug update --field target_release=...` | parity | `compare/09-rhbz-fields/target-release` |
| RHBZ fixed-in | `bzr bug update --field cf_fixed_in=...` | parity | `compare/09-rhbz-fields/fixed-in` |
| RHBZ whiteboards | `bzr bug update --field cf_*_whiteboard=...` | parity | `compare/09-rhbz-fields/whiteboards` |
| API-key placement by server version | `bzr whoami` | bz50/bz52: both query; bz53: bzr header, python-bugzilla query | `compare/06-auth-config-tls/api-key-placement` |
| Restricted password login | `bzr auth login --restrict-login` | parity | `compare/06-auth-config-tls/restricted-login` |
| Cached login token reuse | persisted `token` configuration | parity | `compare/06-auth-config-tls/cached-token` |
| Logout token invalidation | `bzr auth logout` | parity | `compare/06-auth-config-tls/logout` |
| bugzillarc three-file precedence | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/bugzillarc-precedence` |
| bugzillarc default URL | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/bugzillarc-default-url` |
| bugzillarc URL-substring section | no equivalent | python-bugzilla only | `compare/06-auth-config-tls/bugzillarc-substring-section` |
| Disable TLS verification | `--server-tls-insecure` | parity | `compare/06-auth-config-tls/nosslverify` |
| Login-token request transport | persisted `token` configuration | parity | `compare/06-auth-config-tls/token-transport-gap` |
| Login and logout commands | `bzr auth login`, `bzr auth logout` | parity | `compare/06-auth-config-tls/login-command-gap` |
| bugzillarc API-key import | `bzr config import-bugzillarc` | parity; username/password and client certificates are reported unsupported | `compare/06-auth-config-tls/bugzillarc-import` |
| Client certificate configuration | no equivalent | surface gap (#677) | `compare/06-auth-config-tls/client-certificate-surface-gap` |
| Red Hat Bearer API-key transport | automatic REST transport for `bugzilla.redhat.com` | parity | `compare/06-auth-config-tls/bearer-gap` |

The Red-Hat-shaped proxy arm is a harness fixture built from the vendor's documented
parameter names (ADR 0061). It proves bzr's behaviour, not that Red Hat Bugzilla resolves a
named query the same way.

## Python-bugzilla 3.3.0 reference-surface consolidation

The capability matrix above records real-server comparisons.  This matrix makes the
reference surface explicit: every public option declared by `bugzilla/_cli.py` and every
public `Bugzilla` method declared by `bugzilla/base.py` is classified below.  A grouped
cell means each spelling in that cell has the stated classification.  `compare/07-parity-
report/reference-surface` is a checked fixture that rejects a missing or duplicate group;
the cited phase ID is the operation-level evidence where the behavior is exercised.

| python-bugzilla 3.3.0 surface | Classification | bzr equivalent or reason | Evidence |
| --- | --- | --- | --- |
| `--bugzilla`, `--verbose`, `--debug`, `--version`, `--bztype` | non-goal | bzr uses `--server`, verbosity levels, its own version output, and automatic API selection; hidden python-bugzilla compatibility switches are not a bzr contract. | `compare/07-parity-report/reference-surface` |
| `--nosslverify`, `--cert` | parity / explicit gap | `--server-tls-insecure` is parity for `--nosslverify`; client certificate configuration is gap #677. | `compare/06-auth-config-tls/nosslverify`, `compare/06-auth-config-tls/client-certificate-surface-gap` |
| `--login`, `--username`, `--password`, `--restrict-login`, `--ensure-logged-in`, `--no-cache-credentials`, `--cookiefile`, `--tokenfile`, `--api-key`, `pos_username`, `pos_password` | parity / non-goals | Login, logout, restricted login, API-key, and token credential paths are parity. Cookie files, interactive password prompts, positional credentials, and python-bugzilla cache toggles are non-goals because bzr uses explicit credential sources and never imports browser cookies. | `compare/06-auth-config-tls/login-command-gap`, `compare/06-auth-config-tls/login-command-xmlrpc`, `compare/06-auth-config-tls/token-transport-gap`, `compare/06-auth-config-tls/restricted-login`, `compare/06-auth-config-tls/cached-token` |
| `--full`, `--ids`, `--extra`, `--oneline`, `--json`, `--includefield`, `--extrafield`, `--excludefield`, `--raw`, `--outputformat` | non-goal | bzr deliberately has its own table/JSON/NDJSON shapes and `--fields` projection; python-format strings and presentation aliases are out of parity scope. | `compare/01-bug-lifecycle/view` |
| `--field`, `--field-json` | parity | `bzr bug create/update --field/--field-json`. | `compare/01-bug-lifecycle/arbitrary-fields` |
| `--product`, `--version`, `--component`, `--summary`, `--short_desc`, `--comment`, `--long_desc`, `--sub-component`, `--os`, `--arch`, `--severity`, `--priority`, `--alias`, `--status`, `--bug_status`, `--url`, `--target_milestone`, `--target_release`, `--blocked`, `--dependson`, `--keywords`, `--groups`, `--cc`, `--assigned_to`, `--assignee`, `--qa_contact`, `--private`, `--id`, `--bug_id`, `--reporter` | parity / explicit gaps | Core create, query, update, and view fields are parity; `--sub-component` and `--target_release` are RHBZ gap #775. | `compare/01-bug-lifecycle/create`, `compare/01-bug-lifecycle/query`, `compare/01-bug-lifecycle/update`, `compare/09-rhbz-fields/sub-components`, `compare/09-rhbz-fields/target-release` |
| `--quicksearch`, `--savedsearch`, `--savedsearch-sharer-id`, `--from-url`, `--emailtype`, `--components_file`, `--url_type`, `--keywords_type`, `--status_whiteboard_type`, `--fixed_in_type` | parity / explicit gaps / non-goals | Quicksearch and URL import are parity; saved search is #670; URL/keywords/whiteboard match types are parity; `--emailtype`, `--components_file`, and `--fixed_in_type` have no bzr counterpart and are non-goals (the latter is RHBZ-only). | `compare/01-bug-lifecycle/query`, `compare/01-bug-lifecycle/saved-search`, `compare/01-bug-lifecycle/query-match-types`, `compare/09-rhbz-fields/fixed-in` |
| `--products`, `--components`, `--component_owners`, `--versions`, `--active-components` | parity / non-goal | Product and component listing are parity; python-bugzilla's aggregate catalogue-output convenience is a non-goal because bzr retains resource-specific commands. | `compare/00-products/list-products`, `compare/05-products-components/product-catalogues` |
| `--close`, `--dupeid`, `--reset-assignee`, `--reset-qa-contact`, `--file`, `--description`, `--type`, `--get`, `--getall`, `--get-all`, `--ignore-obsolete`, `--comment-tag`, `--minor-update`, `--flag`, `--tags`, `--whiteboard`, `--status_whiteboard`, `--devel_whiteboard`, `--internal_whiteboard`, `--qa_whiteboard`, `--fixed_in` | parity / explicit gaps / non-goals | Close/duplicate, attachment transfer, comments, flags, comment tags, minor update, tags, and obsolete filtering are parity. RHBZ whiteboards and fixed-in are #775; reset-assignee/reset-qa-contact are non-goal convenience aliases. | `compare/01-bug-lifecycle/update-options`, `compare/03-attachments/upload-metadata-comment`, `compare/03-attachments/ignore-obsolete`, `compare/01-bug-lifecycle/bug-tags`, `compare/09-rhbz-fields/whiteboards`, `compare/09-rhbz-fields/fixed-in` |
| `url_to_query`, `fix_url`, `get_rcfile_default_url`, `bz_ver_major`, `bz_ver_minor`, `readconfig`, `connect`, `is_xmlrpc`, `is_rest`, `get_requests_session`, `disconnect` | non-goal | Python library connection/configuration helpers have no public bzr library API. `bzr config import-bugzillarc` imports API keys and reports unsupported username/password and client-certificate entries. | `compare/06-auth-config-tls/bugzillarc-import` |
| `login`, `interactive_save_api_key`, `interactive_login`, `logout`, `logged_in` | parity / non-goals | `bzr auth login` and `bzr auth logout` are parity; interactive credential prompts and library-session inspection are non-goals for a CLI with explicit credential sources. | `compare/06-auth-config-tls/login-command-gap`, `compare/06-auth-config-tls/login-command-xmlrpc`, `compare/06-auth-config-tls/logout` |
| `getbugfields`, `product_get`, `refresh_products`, `getproducts`, `getcomponentsdetails`, `getcomponentdetails`, `getcomponents`, `addcomponent`, `editcomponent`, `getbug`, `getbugs`, `get_comments`, `build_query`, `query_return_extra`, `query`, `pre_translation`, `post_translation`, `bugs_history_raw` | parity / non-goals | Corresponding resource commands are parity; cache refresh, query construction, and translation hooks are Python-library internals with no bzr library surface. Component update is RHBZ gap #774. | `compare/00-products/list-products`, `compare/01-bug-lifecycle/query`, `compare/01-bug-lifecycle/history`, `compare/05-products-components/component-create`, `compare/08-rhbz-externalbugs/component-update` |
| `update_bugs`, `update_tags`, `update_flags`, `build_update`, `attachfile`, `openattachment_data`, `openattachment`, `updateattachmentflags`, `get_attachments`, `build_createbug`, `createbug` | parity / non-goals | Mutation, tags, and attachment commands are parity; Python object builders and file-object return values are non-goals. | `compare/01-bug-lifecycle/update`, `compare/01-bug-lifecycle/bug-tags`, `compare/03-attachments/upload-metadata-comment`, `compare/03-attachments/attachment-flags` |
| `getuser`, `getusers`, `searchusers`, `createuser`, `updateperms`, `getgroup`, `getgroups` | parity | `bzr user`, `bzr group`, and membership commands cover the server operations. | `compare/04-users-groups/user-create-get-search`, `compare/04-users-groups/group-get-and-list`, `compare/04-users-groups/membership-add-remove` |
| `add_external_tracker`, `update_external_tracker`, `remove_external_tracker` | explicit gap | RHBZ `ExternalBugs` methods are owned by #774. | `compare/08-rhbz-externalbugs/add`, `compare/08-rhbz-externalbugs/update`, `compare/08-rhbz-externalbugs/remove` |
