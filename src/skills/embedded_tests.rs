use super::embedded;

#[test]
fn embeds_all_current_skills_in_lexical_order() {
    let names = embedded::skill_names();

    assert_eq!(
        names,
        vec![
            "bzr-bulk-triage",
            "bzr-dependency-analysis",
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

#[test]
fn embeds_complete_dependency_analysis_payload() {
    let paths: Vec<_> = embedded::files()
        .iter()
        .filter_map(|file| file.relative_path.strip_prefix("bzr-dependency-analysis/"))
        .collect();

    assert_eq!(
        paths,
        vec![
            "SKILL.md",
            "scripts/analyze.py",
            "scripts/collect.py",
            "scripts/render.py",
            "tests/fixtures/alias-collapse.expected.json",
            "tests/fixtures/alias-collapse.policy.json",
            "tests/fixtures/branch.analysis.json",
            "tests/fixtures/branch.collection.json",
            "tests/fixtures/cross-server.analysis.json",
            "tests/fixtures/cross-server.collection.json",
            "tests/fixtures/cycle.analysis.json",
            "tests/fixtures/cycle.collection.json",
            "tests/fixtures/diamond.analysis.json",
            "tests/fixtures/diamond.collection.json",
            "tests/fixtures/empty-partial.analysis.json",
            "tests/fixtures/empty-partial.collection.json",
            "tests/fixtures/hostile.analysis.json",
            "tests/fixtures/hostile.expected.md",
            "tests/fixtures/hostile.expected.mmd",
            "tests/fixtures/inaccessible.analysis.json",
            "tests/fixtures/inaccessible.collection.json",
            "tests/fixtures/missing.analysis.json",
            "tests/fixtures/missing.collection.json",
            "tests/fixtures/recording_runner.py",
            "tests/fixtures/resolved.analysis.json",
            "tests/fixtures/resolved.collection.json",
            "tests/fixtures/stale.analysis.json",
            "tests/fixtures/stale.collection.json",
            "tests/skill-contract.sh",
            "tests/test_analyze.py",
            "tests/test_collect.py",
            "tests/test_render.py",
        ]
    );
}
