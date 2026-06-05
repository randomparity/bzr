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

    // Bounded wait: if the parent test aborts before creating the release file
    // (e.g. a panic, or CI cancellation), self-terminate instead of spinning
    // forever holding the lock. The happy path releases in milliseconds.
    let step = std::time::Duration::from_millis(10);
    let mut remaining = std::time::Duration::from_secs(30);
    while !Path::new(release_path).exists() {
        if remaining.is_zero() {
            return;
        }
        std::thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
    // Lock released on drop / process exit.
}
