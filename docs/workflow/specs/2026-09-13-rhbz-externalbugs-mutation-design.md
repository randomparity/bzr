# RHBZ ExternalBugs mutation design (#801)

## Problem

The real RHBZ comparison proves python-bugzilla can add, update, and remove
ExternalBugs links, while bzr has no corresponding command. ADR 0074 proves
the extension and its XML-RPC methods on the pinned server; this change turns
those three controlled gaps into bzr operations without emulating them on
stock Bugzilla.

## Scope

Add `bzr bug external-bug {add,update,remove}`. Each operation takes one bug,
configured tracker ID, and external ID; add/update also take non-empty status
and description. The command requires an API key, verifies the advertised
`ExternalBugs` capability, and dispatches only the proven XML-RPC operation.
It reports the normal mutation envelope and updates the RHBZ comparison phase
to read persisted server state after each bzr mutation.

No ownership transition is needed: the bug command layer owns command shape,
the client owns transport selection, and XML-RPC owns request encoding.
Excluded: stock-server emulation (ADR 0074) and component update (#802).

### Failure model

- Empty user strings fail locally with exit 7.
- Login tokens fail before XML-RPC dispatch with the API-key remedy.
- Absent or unreadable `ExternalBugs` capability fails with exit 15.
- Missing tracker, invalid link, and denied privilege preserve the server error;
  the live phase verifies successful persisted-state paths only.

## Success

- The three commands encode the adapter-proven methods and payload shapes.
- Stock servers do not receive an ExternalBugs mutation request.
- RHBZ add, update, and remove each produce an independent persisted-state PASS.
- CLI reference and parity documentation describe the supported surface.

## Validation

- focused-test: XML-RPC payload tests cover all three method names and fields.
- focused-test: command tests cover local validation and token rejection.
- focused-test: capability tests cover absent/advertised extension behavior.
- focused-test: the RHBZ fixture validates phase control flow.
- task-test-not-applicable: component update remains #802 and is unchanged.
- live-test: `make functional-test` with the RHBZ comparison arm.
