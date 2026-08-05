//! Build script: embed the current git short SHA and the canonical skill
//! payload into the binary.

use std::{
    env,
    fmt::Write as _,
    fs, io,
    path::{Component, Path, PathBuf},
    process::Command,
};

struct PayloadFile {
    relative_path: String,
    source_path: PathBuf,
}

fn main() -> io::Result<()> {
    generate_embedded_skills()?;

    let sha = Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BZR_GIT_SHA={sha}");

    if let Some(manifest_dir) = env::var_os("CARGO_MANIFEST_DIR") {
        for path in git_rerun_paths(Path::new(&manifest_dir)) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    Ok(())
}

fn git_rerun_paths(manifest_dir: &Path) -> Vec<PathBuf> {
    let Some(git_dir) = resolve_git_dir(manifest_dir) else {
        return Vec::new();
    };
    let head = git_dir.join("HEAD");
    let mut paths = vec![head.clone()];
    let Some(reference) = read_symbolic_ref(&head) else {
        return paths;
    };
    let Some(common_dir) = resolve_common_git_dir(&git_dir) else {
        return paths;
    };
    paths.push(common_dir.join(reference));
    paths
}

fn resolve_git_dir(manifest_dir: &Path) -> Option<PathBuf> {
    let dot_git = manifest_dir.join(".git");
    let metadata = fs::metadata(&dot_git).ok()?;
    if metadata.is_dir() {
        return fs::canonicalize(dot_git).ok();
    }
    if !metadata.is_file() {
        return None;
    }
    let contents = fs::read_to_string(dot_git).ok()?;
    let path = contents.strip_prefix("gitdir: ")?.trim_end();
    if path.is_empty() || path.contains(['\n', '\r']) {
        return None;
    }
    let path = Path::new(path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    };
    fs::canonicalize(path).ok()
}

fn read_symbolic_ref(head: &Path) -> Option<PathBuf> {
    let contents = fs::read_to_string(head).ok()?;
    let reference = contents.strip_prefix("ref: ")?.trim_end();
    let path = Path::new(reference);
    if reference.contains(['\n', '\r'])
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        || path.components().next() != Some(Component::Normal("refs".as_ref()))
    {
        return None;
    }
    Some(path.to_path_buf())
}

fn resolve_common_git_dir(git_dir: &Path) -> Option<PathBuf> {
    let commondir = git_dir.join("commondir");
    let Ok(contents) = fs::read_to_string(commondir) else {
        return Some(git_dir.to_path_buf());
    };
    let path = contents.trim_end();
    if path.is_empty() || path.contains(['\n', '\r']) {
        return None;
    }
    let path = Path::new(path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        git_dir.join(path)
    };
    fs::canonicalize(path).ok()
}

fn generate_embedded_skills() -> io::Result<()> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
            io::Error::other("CARGO_MANIFEST_DIR is not set for the build script")
        })?);
    let skills_dir = manifest_dir.join("content/skills");
    let files = collect_payload_files(&skills_dir)?;
    let manifest = render_manifest(&files)?;
    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR")
            .ok_or_else(|| io::Error::other("OUT_DIR is not set for the build script"))?,
    );

    fs::write(out_dir.join("embedded_skills.rs"), manifest)
}

fn collect_payload_files(skills_dir: &Path) -> io::Result<Vec<PayloadFile>> {
    println!("cargo:rerun-if-changed={}", skills_dir.display());

    let metadata = fs::symlink_metadata(skills_dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_payload(format!(
            "canonical skill payload must be a directory: {}",
            skills_dir.display()
        )));
    }

    let mut files = Vec::new();
    collect_regular_files(skills_dir, skills_dir, &mut files)?;
    if files.is_empty() {
        return Err(invalid_payload("canonical skill payload is empty"));
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    validate_skill_entrypoints(&files)?;

    Ok(files)
}

fn collect_regular_files(
    skills_dir: &Path,
    directory: &Path,
    files: &mut Vec<PayloadFile>,
) -> io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let source_path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(invalid_payload(format!(
                "symbolic links are not allowed: {}",
                source_path.display()
            )));
        }
        if file_type.is_dir() {
            println!("cargo:rerun-if-changed={}", source_path.display());
            collect_regular_files(skills_dir, &source_path, files)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(invalid_payload(format!(
                "only regular files are allowed: {}",
                source_path.display()
            )));
        }

        let relative_path = normalized_relative_path(skills_dir, &source_path)?;
        println!("cargo:rerun-if-changed={}", source_path.display());
        files.push(PayloadFile {
            relative_path,
            source_path,
        });
    }

    Ok(())
}

fn normalized_relative_path(skills_dir: &Path, source_path: &Path) -> io::Result<String> {
    let relative_path = source_path.strip_prefix(skills_dir).map_err(|_| {
        invalid_payload(format!(
            "payload file escaped the canonical tree: {}",
            source_path.display()
        ))
    })?;
    let mut parts = Vec::new();
    for component in relative_path.components() {
        let Component::Normal(part) = component else {
            return Err(invalid_payload(format!(
                "payload path is not normalized: {}",
                relative_path.display()
            )));
        };
        let part = part.to_str().ok_or_else(|| {
            invalid_payload(format!(
                "payload path is not valid UTF-8: {}",
                relative_path.display()
            ))
        })?;
        parts.push(part);
    }
    if parts.len() < 2 {
        return Err(invalid_payload(format!(
            "payload files must be under a skill directory: {}",
            relative_path.display()
        )));
    }

    Ok(parts.join("/"))
}

fn validate_skill_entrypoints(files: &[PayloadFile]) -> io::Result<()> {
    let mut skills = Vec::new();
    for file in files {
        let Some((skill, _)) = file.relative_path.split_once('/') else {
            return Err(invalid_payload(format!(
                "payload file has no skill directory: {}",
                file.relative_path
            )));
        };
        if skills.last().is_none_or(|previous| *previous != skill) {
            skills.push(skill);
        }
    }

    for skill in skills {
        validate_skill_name(skill)?;
        let entrypoint = format!("{skill}/SKILL.md");
        if !files.iter().any(|file| file.relative_path == entrypoint) {
            return Err(invalid_payload(format!(
                "skill directory is missing SKILL.md: {skill}"
            )));
        }
    }

    Ok(())
}

fn validate_skill_name(skill: &str) -> io::Result<()> {
    let valid = !skill.is_empty()
        && skill.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        });
    if valid {
        return Ok(());
    }
    Err(invalid_payload(format!(
        "skill directory name must contain lowercase ASCII alphanumeric segments separated by \
         single hyphens: {skill:?}"
    )))
}

fn render_manifest(files: &[PayloadFile]) -> io::Result<String> {
    let mut manifest = String::from("static EMBEDDED_FILES: &[EmbeddedFile] = &[\n");
    for file in files {
        let source_path = file.source_path.to_str().ok_or_else(|| {
            invalid_payload(format!(
                "payload path is not valid UTF-8: {}",
                file.source_path.display()
            ))
        })?;
        writeln!(
            manifest,
            "    EmbeddedFile {{ relative_path: {:?}, bytes: include_bytes!({source_path:?}) }},",
            file.relative_path
        )
        .map_err(|_| io::Error::other("could not write embedded skill manifest"))?;
    }
    manifest.push_str("];\n");
    Ok(manifest)
}

fn invalid_payload(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
