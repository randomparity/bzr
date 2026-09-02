# ADR 0036: Preserve bug-create group presence through structured input

## Status

Accepted

## Context

Bugzilla distinguishes an omitted `groups` member on `Bug.create` from an explicit empty array.
Omission applies the product's default groups; `"groups": []` opts the bug out of them. `bzr`
currently accepts and documents a `groups` array in `bug create --from-json`, but deserializes a
missing key and an empty array into the same empty `Vec`, then omits every empty `Vec` while
serializing `CreateBugParams`. The documented opt-out is therefore unreachable. Verified at
commit `fa230aec233a9d61609c11d8d0a3df6ac9b72e8b` by `src/commands/bug/create_json.rs`,
`src/types/bug/payload.rs`, and `schemas/bug-create-input.json`.

## Decision

Keep the existing `bug create --from-json` `groups` key as the user-facing opt-out surface and
preserve the public Rust shape of `CreateBugParams.groups: Vec<String>`. Add a crate-visible,
non-public presence marker to the existing non-exhaustive payload type and replace its derived
serializer with a serializer backed by a private wire view. The wire view computes exactly one
`groups` member: it omits an empty vector by default, emits `"groups": []` when structured input
explicitly supplied an empty array, and emits non-empty values regardless of the marker. A
crate-private setter records structured-input presence.

Structured input preserves the same distinction before it reaches the payload. A missing key maps
to `None`, an array maps to `Some(array)`, and JSON `null` remains rejected so the accepted input
schema does not broaden. A non-empty `--groups` flag continues to override the JSON member.

## Consequences

Existing Rust callers and flag, template, clone, and structured inputs that omit groups retain
their current field type and server behavior. Structured input can now express the Bugzilla
empty-array opt-out without a new flag or schema key. The published input schema and
`SCHEMA_VERSION` do not change. Existing in-crate full struct expressions initialize the private
marker to false; only the structured-input adapter sets it true. The wire view cannot emit a
duplicate `groups` key even if later in-crate code mutates the public vector.

## Considered & rejected

- **Add `--no-groups`.** judgment: the existing structured-input `groups` array already has the
  required empty representation, so a second public surface would add parsing, precedence, and
  documentation rules without enabling new behavior.
- **Let `--groups ''` mean empty.** verified: `src/cli/bug/mod.rs` at commit
  `fa230aec233a9d61609c11d8d0a3df6ac9b72e8b` defines `--groups` as a repeatable,
  comma-delimited list of group names; overloading an empty group name would make malformed list
  input carry presence semantics.
- **Accept JSON `null` as equivalent to omission.** verified:
  `schemas/bug-create-input.json` at commit `fa230aec233a9d61609c11d8d0a3df6ac9b72e8b`
  allows only arrays for `groups`; accepting null would change the parser contract without a
  schema change.
- **Always serialize an empty group array.** verified: Bugzilla's create contract and issue #623
  distinguish omission, which applies product defaults, from an explicit empty array; always
  sending empty would silently change existing creates on products with defaults.
