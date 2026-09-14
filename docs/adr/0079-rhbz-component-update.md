# ADR 0079: Restore component update for advertised RHBZ XML-RPC

## Status

Accepted

## Context

ADR 0037 correctly removed the former `PUT /component/{id}` command: stock
Bugzilla does not expose that endpoint. The real RHBZ comparison now proves a
different contract through `Component.update` over XML-RPC. The proof updates a
component by its product/name pair and independently reads the persisted state.

## Decision

Expose `bzr component update` only through the XML-RPC `Component.update`
request shape, after the server advertises the `RedHat` extension. It accepts a
product/component target and one or more supported changes: description, default
assignee, or active state. Login tokens are refused because XML-RPC requires an
API key.

Do not restore the removed stock REST PUT request. A stock server is refused at
the capability gate before an update request. ExternalBugs remains its separate
extension and command surface.

## Consequences

RHBZ administrators receive a tested update command while stock users retain a
clear unsupported-capability refusal. The command has no REST fallback and no
component-ID target because those would imply a stock contract that is not
proven. The RHBZ functional phase is the compatibility proof for the server
contract.

## Considered and rejected

- **Restore `PUT /component/{id}`.** ADR 0037's stock-server evidence remains
  valid, so this would recreate the unsupported public surface.
- **Offer the command without a capability gate.** Upstream can accept vendor
  fields without implementing their semantics; the gate prevents a false
  success.
