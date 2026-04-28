//! Shared helpers for bug subcommand handlers.
//!
//! Currently empty: the per-handler logic in this directory does not share
//! mechanically-extractable code. The duplications `SonarCloud` flagged in the
//! pre-split file lived almost entirely inside test fixtures (e.g. repeated
//! `BugAction::List { … }` constructors, repeated mock JSON payloads), and
//! those naturally split into each handler's own `mod tests` block.
//!
//! Add helpers here when a cross-handler pattern emerges that uses identical
//! types and control flow (per the spec: extract only when generics need ≤2
//! type parameters and no non-trivial trait bounds).
