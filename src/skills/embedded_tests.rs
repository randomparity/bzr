use super::embedded;

#[test]
fn embeds_all_current_skills_in_lexical_order() {
    let names = embedded::skill_names();

    assert_eq!(
        names,
        vec![
            "bzr-bulk-triage",
            "bzr-dry-run-confirm",
            "bzr-file-bug",
            "bzr-reference",
            "bzr-release-tracking",
            "bzr-search-report",
            "bzr-setup",
            "bzr-triage-bug",
            "bzr-weekly-status",
        ]
    );
}

#[test]
fn embeds_normalized_payload_paths_and_each_skill_entrypoint() {
    let files = embedded::files();
    let paths: Vec<_> = files.iter().map(|file| file.relative_path).collect();

    assert!(paths.contains(&"bzr-reference/reference/commands.md"));
    assert!(paths
        .iter()
        .all(|path| !path.starts_with('/') && !path.contains('\\')));
    assert!(paths
        .iter()
        .all(|path| !path.split('/').any(|part| part == "..")));

    for skill in embedded::skill_names() {
        let entrypoints = paths
            .iter()
            .filter(|path| **path == format!("{skill}/SKILL.md"))
            .count();
        assert_eq!(entrypoints, 1, "{skill} must have one SKILL.md");
    }
}
