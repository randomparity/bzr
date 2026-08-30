//! Output formatting modules.
//!
//! Command modules import resource writers from their owning leaf module so
//! unused facade exports do not accumulate as output formats change.

/// Version of the `--json` output contract: the `{schema_version, data}`
/// envelope plus the payload shapes inside `data`. Bumped manually per the JSON
/// Output Stability policy in `docs/bzr-cli.md`, independent of the crate
/// version. Present in `--json` output only (never `--output ndjson`).
pub const SCHEMA_VERSION: &str = "0.6.2";

mod formatting;
pub mod progress;
pub(crate) mod resources;
pub(crate) mod result_types;
pub mod writers;
