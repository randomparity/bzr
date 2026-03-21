pub mod attachment;
pub mod bug;
pub mod classification;
pub mod comment;
pub mod component;
pub mod config;
pub mod field;
mod flags;
pub mod group;
pub mod product;
pub mod server;
mod shared;
pub mod user;
pub mod whoami;

// ── Test infrastructure ──────────────────────────────────────────────

#[cfg(test)]
pub(super) mod test_helpers;
