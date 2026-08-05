//! Cross-process lock holder used only by integration tests.
use std::fs::{File, OpenOptions};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "skills") {
        return run_skills_mode(&args).map_err(Into::into);
    }
    run_config_mode(&args).map_err(Into::into)
}

fn run_skills_mode(args: &[String]) -> bzr::error::Result<()> {
    let [mode, destination, ready, release] = args else {
        return Err(bzr::error::BzrError::input(
            "usage: bzr_lock_helper skills <destination> <ready-file> <release-file>".into(),
        ));
    };
    if mode != "skills" {
        return Err(bzr::error::BzrError::input(
            "skills lock-helper mode was not selected".into(),
        ));
    }
    bzr::skills_test_helpers::hold_destination_lock(
        Path::new(destination),
        Path::new(ready),
        Path::new(release),
    )
}

fn run_config_mode(args: &[String]) -> std::io::Result<()> {
    let [lock_path, ready_path, release_path] = args else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: bzr_lock_helper <lock-file> <ready-file> <release-file>",
        ));
    };

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    file.lock()?;
    File::create(ready_path)?;

    let step = std::time::Duration::from_millis(10);
    let mut remaining = std::time::Duration::from_secs(30);
    while !Path::new(release_path).exists() {
        if remaining.is_zero() {
            return Ok(());
        }
        std::thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
    Ok(())
}
