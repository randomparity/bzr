use super::validate_skill_name;

#[test]
fn accepts_canonical_skill_names() {
    for name in ["bzr-setup", "bzr-file-bug", "skill1", "a1-b2-c3"] {
        assert!(validate_skill_name(name).is_ok(), "{name}");
    }
}

#[test]
fn rejects_non_ascii_and_control_characters() {
    for name in ["bzr-café", "bzr-file\nbug", "bzr-file\rbug"] {
        assert!(validate_skill_name(name).is_err(), "{name:?}");
    }
}

#[test]
fn rejects_empty_and_malformed_hyphen_separators() {
    for name in ["", "-bzr", "bzr-", "bzr--file", "Bzr-file", "bzr_file"] {
        assert!(validate_skill_name(name).is_err(), "{name:?}");
    }
}
