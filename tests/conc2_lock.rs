#![cfg(feature = "test-helpers")]
//! CONC-2 mutual-exclusion: a second process holding config.lock must block
//! this process's try-lock, which succeeds once the holder releases.
#![expect(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::process::Command;

fn wait_for(path: &Path) {
    for _ in 0..500 {
        if path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

#[test]
fn second_process_holding_the_lock_blocks_try_lock() {
    let dir = tempfile::TempDir::new().unwrap();
    let lock_path = dir.path().join("config.lock");
    let ready = dir.path().join("ready");
    let release = dir.path().join("release");

    let helper = env!("CARGO_BIN_EXE_bzr_lock_helper");
    let mut child = Command::new(helper)
        .arg(&lock_path)
        .arg(&ready)
        .arg(&release)
        .spawn()
        .expect("spawn lock helper");

    wait_for(&ready);

    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    match f.try_lock() {
        Err(TryLockError::WouldBlock) => {}
        other => panic!("expected WouldBlock while child holds lock, got {other:?}"),
    }

    File::create(&release).unwrap();
    child.wait().expect("child exits");
    f.try_lock()
        .expect("try_lock must succeed after the holder releases");
}
