//! Test-only helper: acquire an exclusive lock on argv[1], create the ready
//! file argv[2], then poll for the release file argv[3] before exiting.
//! Used by the CONC-2 two-process mutual-exclusion test. Not shipped (gated
//! behind the `test-helpers` feature).
#![expect(clippy::expect_used)]
use std::fs::{File, OpenOptions};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lock_path = &args[1];
    let ready_path = &args[2];
    let release_path = &args[3];

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .expect("open lock file");
    file.lock().expect("acquire exclusive lock");

    File::create(ready_path).expect("write ready file");

    while !Path::new(release_path).exists() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Lock released on drop / process exit.
}
