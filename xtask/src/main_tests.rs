#![expect(clippy::panic, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempOutputDir {
    path: PathBuf,
}

impl TempOutputDir {
    fn new() -> Self {
        for attempt in 0..100 {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "bzr-xtask-man-test-{}-{stamp}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => panic!("failed to create {}: {e}", path.display()),
            }
        }
        panic!("failed to create a unique xtask manpage test directory");
    }
}

impl Drop for TempOutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn generate_man_writes_root_and_subcommand_pages() {
    let out = TempOutputDir::new();

    super::generate_man(&out.path).unwrap();

    assert_non_empty_manpage(&out.path.join("bzr.1"));
    assert_non_empty_manpage(&out.path.join("bzr-bug.1"));
}

fn assert_non_empty_manpage(path: &Path) {
    assert!(path.is_file(), "expected generated file {}", path.display());
    let body = fs::read_to_string(path).unwrap();
    assert!(
        !body.trim().is_empty(),
        "expected generated manpage content in {}",
        path.display()
    );
    assert!(
        body.contains(".TH ") && body.contains(".SH NAME"),
        "expected roff manpage sections in {}",
        path.display()
    );
}
