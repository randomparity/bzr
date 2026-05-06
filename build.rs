//! Build script: embed the current git short SHA into the binary so
//! `bzr --version` can disambiguate dev builds between releases.
//!
//! Falls back to `unknown` when the build runs outside a git checkout
//! (e.g. when the crate is built from a crates.io tarball). The
//! `rerun-if-changed=.git/HEAD` line ensures Cargo invalidates the
//! cached env var on commits and branch switches, but only when the
//! `.git/HEAD` file itself exists — packaged-tarball builds skip the
//! directive (the file path doesn't exist) and reuse the cached
//! `unknown` until a real input file changes.

use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BZR_GIT_SHA={sha}");

    if std::path::Path::new(".git/HEAD").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
    }
}
