# RHBZ ExternalBugs mutation implementation plan (#801)

1. Add the nested bug CLI action and direct validation for one link target.
2. Add an `ExternalBugs` capability allowlist entry and gate before mutation.
3. Encode add/update/remove through the existing XML-RPC client boundary.
4. Return the established mutation output envelope and document the flags.
5. Replace the three controlled RHBZ gaps with bzr mutation/readback proofs;
   retain the component-update gap for #802.
6. Run focused tests, lint, the normal suite, and the live RHBZ functional arm.

## Contracts

| Contract | Evidence |
| --- | --- |
| Add emits `ExternalBugs.add_external_bug` with `bug_ids` and one `external_bugs` record. | XML-RPC test and RHBZ readback |
| Update/remove target `(bug, tracker, external ID)` through their named methods. | XML-RPC test and RHBZ readback |
| A stock server is refused before a mutation call. | capability tests |
| Login token use is refused before XML-RPC dispatch. | command test |

## Verification

- `make test-one T=external_bug`
- `make lint`
- `make test`
- `make functional-test` with RHBZ configured
