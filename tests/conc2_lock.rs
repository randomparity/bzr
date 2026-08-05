#![cfg(feature = "test-helpers")]
//! CONC-2 mutual-exclusion: a second process holding config.lock must block
//! this process's try-lock, which succeeds once the holder releases.
#![expect(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used
)]
use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

/// Serializes the tests in this binary. One test mutates the process
/// environment (`set_var`) while the other spawns a child that inherits
/// (reads) the environment — running them concurrently would be a data race
/// on `environ`. Both tests hold this for their whole body.
static SERIAL: Mutex<()> = Mutex::new(());

fn wait_for(path: &Path) {
    for _ in 0..500 {
        if path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

fn wait_for_or_stop(path: &Path, child: &mut std::process::Child) {
    for _ in 0..500 {
        if path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("timed out waiting for {}", path.display());
}

#[test]
fn second_process_holding_the_lock_blocks_try_lock() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

/// Drive `Config::update_locked_at` itself through the contention branch of
/// `acquire_exclusive_lock`: while a second process holds `config.lock`,
/// `update_locked_at` must block (not error or return early), then acquire the
/// lock and complete its write once the holder releases.
#[test]
fn update_locked_waits_for_a_held_lock_then_completes() {
    use bzr::config::Config;
    use std::sync::mpsc;
    use std::time::Duration;

    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    // update_locked_at on a background thread: try_lock is contended, so it must
    // fall back to a blocking lock and stay blocked while the helper holds it.
    let config_path = cfg_dir.join("config.toml");
    let (tx, rx) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        let result = Config::update_locked_at(Some(&config_path), |c| {
            c.default_server = Some("sentinel".to_string());
            Ok(())
        });
        tx.send(result.is_ok()).unwrap();
    });

    // Still blocked while the holder is alive.
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "update_locked_at must block while another process holds config.lock"
    );

    // Release the holder; update_locked_at now acquires the lock and completes.
    File::create(&release).unwrap();
    child.wait().expect("child exits");
    let ok = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("update_locked_at must complete after the lock is released");
    assert!(ok, "update_locked_at must succeed after acquiring the lock");
    writer.join().unwrap();

    // The write actually landed on disk.
    let reloaded = Config::load_at(Some(&cfg_dir.join("config.toml"))).unwrap();
    assert_eq!(reloaded.default_server.as_deref(), Some("sentinel"));
}

#[test]
fn skills_mode_requires_exactly_destination_ready_and_release_arguments() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let helper = env!("CARGO_BIN_EXE_bzr_lock_helper");
    let status = Command::new(helper)
        .args(["skills", "/tmp/destination", "/tmp/ready"])
        .status()
        .unwrap();

    assert!(!status.success());
}

#[test]
fn real_skills_install_refuses_a_helper_held_destination_lock() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let project = tempfile::TempDir::new().unwrap();
    let destination = project.path().join(".agents/skills");
    std::fs::create_dir_all(&destination).unwrap();
    let ready = project.path().join("skills-ready");
    let release = project.path().join("skills-release");
    let helper = env!("CARGO_BIN_EXE_bzr_lock_helper");
    let mut child = Command::new(helper)
        .arg("skills")
        .arg(&destination)
        .arg(&ready)
        .arg(&release)
        .spawn()
        .expect("spawn skills lock helper");
    wait_for_or_stop(&ready, &mut child);

    let output = Command::new(env!("CARGO_BIN_EXE_bzr"))
        .args(["skills", "install", "--agent", "standard", "--project"])
        .arg(project.path())
        .output()
        .expect("run real bzr binary");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("locked"));
    assert!(
        child.try_wait().unwrap().is_none(),
        "helper must retain ownership"
    );
    assert!(destination.join(".bzr-skill.lock").is_dir());

    File::create(&release).unwrap();
    assert!(child.wait().unwrap().success());
    assert!(!destination.join(".bzr-skill.lock").exists());
}

#[test]
fn skills_mode_timeout_exits_nonzero_and_removes_its_lock() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let project = tempfile::TempDir::new().unwrap();
    let destination = project.path().join("skills");
    std::fs::create_dir(&destination).unwrap();
    let ready = project.path().join("timeout-ready");
    let release = project.path().join("never-released");
    let helper = env!("CARGO_BIN_EXE_bzr_lock_helper");
    let mut child = Command::new(helper)
        .arg("skills")
        .arg(&destination)
        .arg(&ready)
        .arg(&release)
        .spawn()
        .expect("spawn timeout lock helper");
    wait_for_or_stop(&ready, &mut child);

    let status = child.wait().expect("timeout helper exits boundedly");

    assert!(!status.success());
    assert!(!destination.join(".bzr-skill.lock").exists());
}
