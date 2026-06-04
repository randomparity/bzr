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

/// Drive `Config::update_locked` itself through the contention branch of
/// `acquire_exclusive_lock`: while a second process holds `config.lock`,
/// `update_locked` must block (not error or return early), then acquire the
/// lock and complete its write once the holder releases.
#[test]
fn update_locked_waits_for_a_held_lock_then_completes() {
    use bzr::config::Config;
    use std::sync::mpsc;
    use std::time::Duration;

    let dir = tempfile::TempDir::new().unwrap();
    // Point Config at this dir; pre-create the `bzr` subdir so the helper can
    // create `config.lock` there before any `update_locked` call resolves it.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };
    let cfg_dir = dir.path().join("bzr");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let lock_path = cfg_dir.join("config.lock");
    let ready = dir.path().join("ready");
    let release = dir.path().join("release");

    // A second process grabs config.lock and holds it until released.
    let helper = env!("CARGO_BIN_EXE_bzr_lock_helper");
    let mut child = Command::new(helper)
        .arg(&lock_path)
        .arg(&ready)
        .arg(&release)
        .spawn()
        .expect("spawn lock helper");
    wait_for(&ready);

    // update_locked on a background thread: try_lock is contended, so it must
    // fall back to a blocking lock and stay blocked while the helper holds it.
    let (tx, rx) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        let result = Config::update_locked(|c| {
            c.default_server = Some("sentinel".to_string());
            Ok(())
        });
        tx.send(result.is_ok()).unwrap();
    });

    // Still blocked while the holder is alive.
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "update_locked must block while another process holds config.lock"
    );

    // Release the holder; update_locked now acquires the lock and completes.
    File::create(&release).unwrap();
    child.wait().expect("child exits");
    let ok = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("update_locked must complete after the lock is released");
    assert!(ok, "update_locked must succeed after acquiring the lock");
    writer.join().unwrap();

    // The write actually landed on disk.
    let reloaded = Config::load().unwrap();
    assert_eq!(reloaded.default_server.as_deref(), Some("sentinel"));
}
