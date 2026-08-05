/// A file from the canonical skill payload compiled into this binary.
pub struct EmbeddedFile {
    pub relative_path: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/embedded_skills.rs"));

pub fn files() -> &'static [EmbeddedFile] {
    EMBEDDED_FILES
}

pub fn skill_names() -> Vec<&'static str> {
    let mut names = Vec::new();
    for file in files() {
        let Some((skill, _)) = file.relative_path.split_once('/') else {
            continue;
        };
        if names.last().copied() != Some(skill) {
            names.push(skill);
        }
    }
    names
}
