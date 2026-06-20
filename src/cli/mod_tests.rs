#![expect(clippy::unwrap_used, clippy::panic)]

use super::{
    AttachmentAction, BugAction, ClassificationAction, Cli, Commands, CommentAction,
    ComponentAction, ConfigAction, FieldAction, GroupAction, ProductAction, QueryAction,
    ServerAction, TemplateAction, UserAction,
};
use crate::types::{ApiMode, ProductListType};
use clap::{CommandFactory as _, Parser as _};

/// Doc-comment coverage for items already converted to multi-paragraph
/// `long_about` per docs/dev/cli-doc-style.md. Phase 1 + 2a of the
/// CLI doc-expansion plan; extend as phases 2b/2c land. When this list
/// covers every leaf subcommand, drop the explicit list and walk the
/// command tree instead.
const COVERED_PATHS: &[&[&str]] = &[
    // Phase 1: top-level + 14 group variants
    &[],
    &["bug"],
    &["comment"],
    &["attachment"],
    &["config"],
    &["product"],
    &["field"],
    &["user"],
    &["group"],
    &["whoami"],
    &["server"],
    &["classification"],
    &["component"],
    &["template"],
    &["query"],
    // Phase 2a: bug.rs actions
    &["bug", "list"],
    &["bug", "view"],
    &["bug", "search"],
    &["bug", "history"],
    &["bug", "create"],
    &["bug", "my"],
    &["bug", "clone"],
    &["bug", "update"],
    // Phase 2a: config.rs actions
    &["config", "set-server"],
    &["config", "set-default"],
    &["config", "show"],
    &["config", "set-keyring"],
    &["config", "unset-keyring"],
    &["config", "migrate-to-keyring"],
    // Phase 2a: query.rs actions
    &["query", "save"],
    &["query", "list"],
    &["query", "show"],
    &["query", "delete"],
    &["query", "run"],
    // Phase 2b: attachment.rs actions
    &["attachment", "list"],
    &["attachment", "download"],
    &["attachment", "upload"],
    &["attachment", "update"],
    // Phase 2b: comment.rs actions
    &["comment", "list"],
    &["comment", "add"],
    &["comment", "tag"],
    &["comment", "search-tags"],
    // Phase 2b: user.rs actions
    &["user", "search"],
    &["user", "create"],
    &["user", "update"],
    // Phase 2b: group.rs actions
    &["group", "add-user"],
    &["group", "remove-user"],
    &["group", "list-users"],
    &["group", "view"],
    &["group", "create"],
    &["group", "update"],
    // Phase 2b: product.rs actions
    &["product", "list"],
    &["product", "view"],
    &["product", "create"],
    &["product", "update"],
    // Phase 2b: template.rs actions
    &["template", "save"],
    &["template", "list"],
    &["template", "show"],
    &["template", "delete"],
    // Phase 2b: component.rs actions
    &["component", "create"],
    &["component", "update"],
    // Phase 2c: trivial files
    &["server", "info"],
    &["classification", "view"],
    &["field", "aliases"],
    &["field", "list"],
];

#[test]
fn cli_doc_long_about_coverage() {
    let cmd = Cli::command();
    for path in COVERED_PATHS {
        let mut current = &cmd;
        for &name in *path {
            let next = current.get_subcommands().find(|c| c.get_name() == name);
            current =
                next.unwrap_or_else(|| panic!("subcommand path {path:?} not found in clap tree"));
        }
        let about = current.get_about().map(ToString::to_string);
        let long_about = current.get_long_about().map(ToString::to_string);
        assert!(
            long_about.is_some(),
            "subcommand path {path:?} is missing long_about (multi-paragraph doc comment required)"
        );
        assert_ne!(
            about, long_about,
            "subcommand path {path:?} has long_about identical to about (single-paragraph doc -- expand to multi-paragraph per docs/dev/cli-doc-style.md)"
        );
    }
}

#[test]
fn parse_bug_list_minimal() {
    let cli = Cli::try_parse_from(["bzr", "bug", "list"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Bug {
            action: BugAction::List { .. }
        }
    ));
}

#[test]
fn parse_bug_view_by_id() {
    let cli = Cli::try_parse_from(["bzr", "bug", "view", "12345"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::View { ids, .. },
        } => assert_eq!(ids, vec!["12345".to_string()]),
        _ => panic!("expected Bug View"),
    }
}

#[test]
fn parse_global_json_flag() {
    let cli = Cli::try_parse_from(["bzr", "--json", "bug", "list"]).unwrap();
    assert!(cli.json);
}

#[test]
fn parse_global_server_flag() {
    let cli = Cli::try_parse_from(["bzr", "--server", "myserver", "bug", "list"]).unwrap();
    assert_eq!(cli.server.as_deref(), Some("myserver"));
}

#[test]
fn parse_update_expect_unchanged_since() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "42",
        "--status",
        "RESOLVED",
        "--expect-unchanged-since",
        "2026-06-19T12:00:00Z",
    ])
    .unwrap();
    let Commands::Bug {
        action: BugAction::Update {
            expect_unchanged_since,
            ..
        },
    } = cli.command
    else {
        panic!("expected bug update");
    };
    assert_eq!(
        expect_unchanged_since.as_deref(),
        Some("2026-06-19T12:00:00Z")
    );
}

#[test]
fn parse_inline_server_flags() {
    let cli = Cli::try_parse_from([
        "bzr",
        "--server-url",
        "https://bz.example.com",
        "--server-api-key-env",
        "BZR_KEY",
        "--server-email",
        "dev@example.com",
        "bug",
        "view",
        "42",
    ])
    .unwrap();
    assert_eq!(cli.server_url.as_deref(), Some("https://bz.example.com"));
    assert_eq!(cli.server_api_key_env.as_deref(), Some("BZR_KEY"));
    assert_eq!(cli.server_email.as_deref(), Some("dev@example.com"));
}

#[test]
fn inline_server_url_requires_api_key_env() {
    // --server-url alone is rejected: a credential source is mandatory.
    let result = Cli::try_parse_from([
        "bzr",
        "--server-url",
        "https://bz.example.com",
        "bug",
        "view",
        "42",
    ]);
    assert!(result.is_err(), "--server-url without env key must fail");
}

#[test]
fn inline_server_conflicts_with_named_server() {
    // A named (config) server and an inline one are mutually exclusive.
    let result = Cli::try_parse_from([
        "bzr",
        "--server",
        "prod",
        "--server-url",
        "https://bz.example.com",
        "--server-api-key-env",
        "BZR_KEY",
        "bug",
        "view",
        "42",
    ]);
    assert!(result.is_err(), "--server and --server-url must conflict");
}

#[test]
fn inline_server_email_requires_url() {
    // --server-email is meaningless without --server-url.
    let result = Cli::try_parse_from(["bzr", "--server-email", "dev@example.com", "bug", "list"]);
    assert!(
        result.is_err(),
        "--server-email without --server-url must fail"
    );
}

#[test]
fn parse_bug_create_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "bug", "create", "--from-json", "-"]).unwrap();
    let Commands::Bug {
        action: BugAction::Create { from_json, .. },
    } = cli.command
    else {
        panic!("expected bug create");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
}

#[test]
fn bug_create_from_json_conflicts_with_template() {
    let result = Cli::try_parse_from([
        "bzr",
        "bug",
        "create",
        "--from-json",
        "bugs.json",
        "--template",
        "sec",
    ]);
    assert!(
        result.is_err(),
        "--from-json and --template must be mutually exclusive"
    );
}

#[test]
fn parse_global_yes_flag_short_and_long() {
    let short = Cli::try_parse_from(["bzr", "-y", "bug", "update", "5", "--status", "X"]).unwrap();
    assert!(short.yes);
    let long =
        Cli::try_parse_from(["bzr", "--yes", "bug", "update", "5", "--status", "X"]).unwrap();
    assert!(long.yes);
    let absent = Cli::try_parse_from(["bzr", "bug", "list"]).unwrap();
    assert!(!absent.yes);
}

#[test]
fn parse_config_set_server() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz.example.com",
        "--api-key",
        "secret123",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Commands::Config {
            action: ConfigAction::SetServer { .. }
        }
    ));
}

#[test]
fn parse_config_set_server_with_env_var() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz.example.com",
        "--api-key-env",
        "BZR_API_KEY",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Commands::Config {
            action: ConfigAction::SetServer { .. }
        }
    ));
}

#[test]
fn parse_unknown_command_fails() {
    let result = Cli::try_parse_from(["bzr", "nonexistent"]);
    assert!(result.is_err());
}

#[test]
fn parse_whoami() {
    let cli = Cli::try_parse_from(["bzr", "whoami"]).unwrap();
    assert!(matches!(cli.command, Commands::Whoami));
    // The redundant `show` subcommand was removed (#323).
    assert!(Cli::try_parse_from(["bzr", "whoami", "show"]).is_err());
}

#[test]
fn parse_bug_search() {
    let cli = Cli::try_parse_from(["bzr", "bug", "search", "crash"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Search { query, limit, .. },
        } => {
            assert_eq!(query.as_deref(), Some("crash"));
            assert_eq!(limit, None);
        }
        _ => panic!("expected Bug Search"),
    }
}

#[test]
fn parse_bug_search_with_limit() {
    let cli = Cli::try_parse_from(["bzr", "bug", "search", "crash", "--limit", "10"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Search { limit, .. },
        } => assert_eq!(limit, Some(10)),
        _ => panic!("expected Bug Search"),
    }
}

#[test]
fn parse_bug_history() {
    let cli = Cli::try_parse_from(["bzr", "bug", "history", "42"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::History { id, since },
        } => {
            assert_eq!(id, 42);
            assert!(since.is_none());
        }
        _ => panic!("expected Bug History"),
    }
}

#[test]
fn parse_bug_create() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "create",
        "--product",
        "TestProduct",
        "--component",
        "General",
        "--summary",
        "Test bug",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action:
                BugAction::Create {
                    product,
                    component,
                    summary,
                    version,
                    ..
                },
        } => {
            assert_eq!(product.as_deref(), Some("TestProduct"));
            assert_eq!(component.as_deref(), Some("General"));
            assert_eq!(summary.as_deref(), Some("Test bug"));
            assert_eq!(version, None);
        }
        _ => panic!("expected Bug Create"),
    }
}

#[test]
fn parse_bug_create_with_description_file() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "create",
        "--product",
        "TestProduct",
        "--component",
        "General",
        "--summary",
        "Test bug",
        "--description-file",
        "/tmp/desc.txt",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Create {
                description_file, ..
            },
        } => {
            assert_eq!(
                description_file.as_deref(),
                Some(std::path::Path::new("/tmp/desc.txt"))
            );
        }
        _ => panic!("expected Bug Create"),
    }
}

#[test]
fn parse_bug_create_description_and_description_file_conflict() {
    let result = Cli::try_parse_from([
        "bzr",
        "bug",
        "create",
        "--product",
        "P",
        "--component",
        "C",
        "--summary",
        "S",
        "--description",
        "literal",
        "--description-file",
        "/tmp/desc.txt",
    ]);
    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_bug_update_with_flags() {
    let cli = Cli::try_parse_from([
        "bzr", "bug", "update", "42", "--status", "RESOLVED", "--flag", "review+",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Update {
                ids, status, flag, ..
            },
        } => {
            assert_eq!(ids, vec![42]);
            assert_eq!(status.as_deref(), Some("RESOLVED"));
            assert_eq!(flag, vec!["review+"]);
        }
        _ => panic!("expected Bug Update"),
    }
}

#[test]
fn parse_bug_update_with_dupe_of() {
    let cli = Cli::try_parse_from(["bzr", "bug", "update", "42", "--dupe-of", "99"]).unwrap();
    match cli.command {
        Commands::Bug {
            action:
                BugAction::Update {
                    ids,
                    dupe_of,
                    status,
                    resolution,
                    ..
                },
        } => {
            assert_eq!(ids, vec![42]);
            assert_eq!(dupe_of, Some(99));
            assert!(status.is_none());
            assert!(resolution.is_none());
        }
        _ => panic!("expected Bug Update"),
    }
}

#[test]
fn parse_bug_update_scalar_parity_flags() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "42",
        "--alias",
        "short-name",
        "--deadline",
        "2026-12-31",
        "--estimated-time",
        "3.5",
        "--remaining-time",
        "1.25",
        "--work-time",
        "0.5",
        "--reset-assigned-to",
        "--reset-qa-contact",
    ])
    .unwrap();

    let Commands::Bug { action } = cli.command else {
        panic!("expected bug command");
    };
    let BugAction::Update {
        alias,
        deadline,
        estimated_time,
        remaining_time,
        work_time,
        reset_assigned_to,
        reset_qa_contact,
        ..
    } = action
    else {
        panic!("expected bug update");
    };

    assert_eq!(alias.as_deref(), Some("short-name"));
    assert_eq!(deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(estimated_time, Some(3.5));
    assert_eq!(remaining_time, Some(1.25));
    assert_eq!(work_time, Some(0.5));
    assert!(reset_assigned_to);
    assert!(reset_qa_contact);
}

#[test]
fn parse_bug_update_rejects_dupe_of_with_status() {
    let result = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "42",
        "--dupe-of",
        "99",
        "--status",
        "RESOLVED",
    ]);

    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_bug_update_rejects_dupe_of_with_resolution() {
    let result = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "42",
        "--dupe-of",
        "99",
        "--resolution",
        "DUPLICATE",
    ]);

    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_bug_update_with_comment() {
    let cli = Cli::try_parse_from(["bzr", "bug", "update", "42", "--comment", "see #99"]).unwrap();
    match cli.command {
        Commands::Bug {
            action:
                BugAction::Update {
                    ids,
                    comment,
                    comment_file,
                    comment_private,
                    ..
                },
        } => {
            assert_eq!(ids, vec![42]);
            assert_eq!(comment.as_deref(), Some("see #99"));
            assert!(comment_file.is_none());
            assert!(!comment_private);
        }
        _ => panic!("expected Bug Update"),
    }
}

#[test]
fn parse_bug_update_with_comment_file_and_private() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "42",
        "--comment-file",
        "/tmp/body.txt",
        "--comment-private",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action:
                BugAction::Update {
                    comment,
                    comment_file,
                    comment_private,
                    ..
                },
        } => {
            assert!(comment.is_none());
            assert_eq!(
                comment_file.as_deref(),
                Some(std::path::Path::new("/tmp/body.txt"))
            );
            assert!(comment_private);
        }
        _ => panic!("expected Bug Update"),
    }
}

#[test]
fn parse_bug_update_rejects_comment_and_comment_file_together() {
    let result = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "42",
        "--comment",
        "x",
        "--comment-file",
        "/tmp/body.txt",
    ]);
    assert!(
        result.is_err(),
        "clap should reject mutually-exclusive flags"
    );
}

#[test]
fn parse_comment_list() {
    let cli = Cli::try_parse_from(["bzr", "comment", "list", "99"]).unwrap();
    match cli.command {
        Commands::Comment {
            action: CommentAction::List { bug_id, .. },
        } => assert_eq!(bug_id, 99),
        _ => panic!("expected Comment List"),
    }
}

#[test]
fn parse_comment_add_with_body() {
    let cli = Cli::try_parse_from(["bzr", "comment", "add", "42", "--body", "This is a comment"])
        .unwrap();
    match cli.command {
        Commands::Comment {
            action:
                CommentAction::Add {
                    bug_id,
                    body,
                    body_file,
                    private,
                },
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(body.as_deref(), Some("This is a comment"));
            assert!(body_file.is_none());
            assert!(!private);
        }
        _ => panic!("expected Comment Add"),
    }
}

#[test]
fn parse_comment_add_with_private() {
    let cli = Cli::try_parse_from([
        "bzr",
        "comment",
        "add",
        "42",
        "--body",
        "secret note",
        "--private",
    ])
    .unwrap();
    match cli.command {
        Commands::Comment {
            action:
                CommentAction::Add {
                    bug_id,
                    body,
                    body_file,
                    private,
                },
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(body.as_deref(), Some("secret note"));
            assert!(body_file.is_none());
            assert!(private);
        }
        _ => panic!("expected Comment Add"),
    }
}

#[test]
fn parse_attachment_list() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "list", "42"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::List { bug_id },
        } => assert_eq!(bug_id, 42),
        _ => panic!("expected Attachment List"),
    }
}

#[test]
fn parse_attachment_download_single_id_legacy() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "download", "100"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Download {
                    ids,
                    bug_ids,
                    out,
                    out_dir,
                },
        } => {
            assert_eq!(ids, vec![100]);
            assert!(bug_ids.is_empty());
            assert!(out.is_none());
            assert_eq!(out_dir, "./attachments");
        }
        _ => panic!("expected Attachment Download"),
    }
}

#[test]
fn parse_attachment_download_with_out() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "download",
        "100",
        "--out",
        "patch.diff",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Download { ids, out, .. },
        } => {
            assert_eq!(ids, vec![100]);
            assert_eq!(out.as_deref(), Some("patch.diff"));
        }
        _ => panic!("expected Attachment Download"),
    }
}

#[test]
fn parse_attachment_download_multiple_positional_ids() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "download", "100", "200", "300"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Download { ids, bug_ids, .. },
        } => {
            assert_eq!(ids, vec![100, 200, 300]);
            assert!(bug_ids.is_empty());
        }
        _ => panic!("expected Attachment Download"),
    }
}

#[test]
fn parse_attachment_download_bug_flag_repeatable() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "download",
        "--bug",
        "12345",
        "--bug",
        "67890",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Download { ids, bug_ids, .. },
        } => {
            assert!(ids.is_empty());
            assert_eq!(bug_ids, vec![12345, 67890]);
        }
        _ => panic!("expected Attachment Download"),
    }
}

#[test]
fn parse_attachment_download_mixed_bug_and_positional() {
    let cli =
        Cli::try_parse_from(["bzr", "attachment", "download", "--bug", "12345", "9876"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Download { ids, bug_ids, .. },
        } => {
            assert_eq!(ids, vec![9876]);
            assert_eq!(bug_ids, vec![12345]);
        }
        _ => panic!("expected Attachment Download"),
    }
}

#[test]
fn parse_attachment_download_out_dir_default() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "download", "--bug", "12345"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Download { out_dir, .. },
        } => assert_eq!(out_dir, "./attachments"),
        _ => panic!("expected Attachment Download"),
    }
}

#[test]
fn parse_attachment_download_out_dir_explicit() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "download",
        "--bug",
        "12345",
        "--out-dir",
        "/tmp/att",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Download { out_dir, .. },
        } => assert_eq!(out_dir, "/tmp/att"),
        _ => panic!("expected Attachment Download"),
    }
}

#[test]
fn parse_attachment_download_clap_conflict_out_with_out_dir() {
    let result = Cli::try_parse_from([
        "bzr",
        "attachment",
        "download",
        "100",
        "--out",
        "x",
        "--out-dir",
        "y",
    ]);
    assert!(result.is_err(), "clap should reject --out with --out-dir");
}

#[test]
fn parse_attachment_download_clap_conflict_out_with_bug() {
    let result = Cli::try_parse_from([
        "bzr",
        "attachment",
        "download",
        "--bug",
        "12345",
        "--out",
        "x",
    ]);
    assert!(result.is_err(), "clap should reject --out with --bug");
}

#[test]
fn parse_product_list() {
    let cli = Cli::try_parse_from(["bzr", "product", "list"]).unwrap();
    match cli.command {
        Commands::Product {
            action: ProductAction::List { r#type },
        } => assert_eq!(r#type, ProductListType::Accessible),
        _ => panic!("expected Product List"),
    }
}

#[test]
fn parse_product_view() {
    let cli = Cli::try_parse_from(["bzr", "product", "view", "Firefox"]).unwrap();
    match cli.command {
        Commands::Product {
            action: ProductAction::View { name },
        } => assert_eq!(name, "Firefox"),
        _ => panic!("expected Product View"),
    }
}

#[test]
fn parse_user_search() {
    let cli = Cli::try_parse_from(["bzr", "user", "search", "alice"]).unwrap();
    match cli.command {
        Commands::User {
            action: UserAction::Search { query, details },
        } => {
            assert_eq!(query, "alice");
            assert!(!details);
        }
        _ => panic!("expected User Search"),
    }
}

#[test]
fn parse_group_add_user() {
    let cli = Cli::try_parse_from([
        "bzr",
        "group",
        "add-user",
        "--group",
        "admin",
        "--user",
        "alice@test.com",
    ])
    .unwrap();
    match cli.command {
        Commands::Group {
            action: GroupAction::AddUser { group, user },
        } => {
            assert_eq!(group, "admin");
            assert_eq!(user, "alice@test.com");
        }
        _ => panic!("expected Group AddUser"),
    }
}

#[test]
fn parse_field_list() {
    let cli = Cli::try_parse_from(["bzr", "field", "list", "status"]).unwrap();
    match cli.command {
        Commands::Field {
            action: FieldAction::List { name },
        } => assert_eq!(name, "status"),
        _ => panic!("expected Field List"),
    }
}

#[test]
fn parse_server_info() {
    let cli = Cli::try_parse_from(["bzr", "server", "info"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Server {
            action: ServerAction::Info
        }
    ));
}

#[test]
fn parse_classification_view() {
    let cli = Cli::try_parse_from(["bzr", "classification", "view", "Unclassified"]).unwrap();
    match cli.command {
        Commands::Classification {
            action: ClassificationAction::View { name },
        } => assert_eq!(name, "Unclassified"),
        _ => panic!("expected Classification View"),
    }
}

#[test]
fn parse_component_create() {
    let cli = Cli::try_parse_from([
        "bzr",
        "component",
        "create",
        "--product",
        "TestProduct",
        "--name",
        "Backend",
        "--description",
        "Backend component",
        "--default-assignee",
        "dev@test.com",
    ])
    .unwrap();
    match cli.command {
        Commands::Component {
            action:
                ComponentAction::Create {
                    product,
                    name,
                    description,
                    default_assignee,
                },
        } => {
            assert_eq!(product, "TestProduct");
            assert_eq!(name, "Backend");
            assert_eq!(description, "Backend component");
            assert_eq!(default_assignee, "dev@test.com");
        }
        _ => panic!("expected Component Create"),
    }
}

#[test]
fn parse_verbose_flag() {
    let cli = Cli::try_parse_from(["bzr", "-vvv", "whoami"]).unwrap();
    assert_eq!(cli.verbose, 3);
}

#[test]
fn parse_no_color_flag() {
    let cli = Cli::try_parse_from(["bzr", "--no-color", "whoami"]).unwrap();
    assert!(cli.no_color);
}

#[test]
fn parse_quiet_flag() {
    let cli = Cli::try_parse_from(["bzr", "--quiet", "whoami"]).unwrap();
    assert!(cli.quiet);
}

#[test]
fn parse_config_path_flag() {
    let cli = Cli::try_parse_from(["bzr", "--config", "/tmp/agent.toml", "whoami"]).unwrap();
    assert_eq!(
        cli.config.as_deref(),
        Some(std::path::Path::new("/tmp/agent.toml"))
    );
    // Absent by default.
    let none = Cli::try_parse_from(["bzr", "whoami"]).unwrap();
    assert!(none.config.is_none());
}

#[test]
fn quiet_help_mentions_tracing_is_suppressed() {
    let help = Cli::command().render_long_help().to_string();
    assert!(help.contains("tracing logs are also suppressed"), "{help}");
}

#[test]
fn parse_api_override() {
    let cli = Cli::try_parse_from(["bzr", "--api", "xmlrpc", "whoami"]).unwrap();
    assert_eq!(cli.api, Some(ApiMode::XmlRpc));
}

#[test]
fn parse_config_set_default() {
    let cli = Cli::try_parse_from(["bzr", "config", "set-default", "prod"]).unwrap();
    match cli.command {
        Commands::Config {
            action: ConfigAction::SetDefault { name },
        } => assert_eq!(name, "prod"),
        _ => panic!("expected Config SetDefault"),
    }
}

#[test]
fn parse_config_show() {
    let cli = Cli::try_parse_from(["bzr", "config", "show"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Config {
            action: ConfigAction::Show
        }
    ));
}

#[test]
fn parse_bug_list_with_filters() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--status",
        "NEW",
        "--limit",
        "25",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action:
                BugAction::List {
                    product,
                    status,
                    limit,
                    ..
                },
        } => {
            assert_eq!(product, vec!["Firefox"]);
            assert_eq!(status, vec!["NEW"]);
            assert_eq!(limit, 25);
        }
        _ => panic!("expected Bug List"),
    }
}

#[test]
fn parse_bug_list_summary_substring() {
    let cli = Cli::try_parse_from(["bzr", "bug", "list", "--summary", "kernel panic"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::List { summary, .. },
        } => {
            assert_eq!(summary.as_deref(), Some("kernel panic"));
        }
        _ => panic!("expected Bug List"),
    }
}

#[test]
fn parse_comment_tag() {
    let cli = Cli::try_parse_from([
        "bzr", "comment", "tag", "200", "--add", "spam", "--remove", "good",
    ])
    .unwrap();
    match cli.command {
        Commands::Comment {
            action:
                CommentAction::Tag {
                    comment_id,
                    add,
                    remove,
                },
        } => {
            assert_eq!(comment_id, 200);
            assert_eq!(add, vec!["spam"]);
            assert_eq!(remove, vec!["good"]);
        }
        _ => panic!("expected Comment Tag"),
    }
}

#[test]
fn parse_bug_my_defaults() {
    let cli = Cli::try_parse_from(["bzr", "bug", "my"]).unwrap();
    match cli.command {
        Commands::Bug {
            action:
                BugAction::My {
                    created,
                    cc,
                    all,
                    limit,
                    ..
                },
        } => {
            assert!(!created);
            assert!(!cc);
            assert!(!all);
            assert_eq!(limit, 50);
        }
        _ => panic!("expected Bug My"),
    }
}

#[test]
fn parse_bug_my_all_conflicts_with_created() {
    let result = Cli::try_parse_from(["bzr", "bug", "my", "--all", "--created"]);
    assert!(result.is_err(), "--all should conflict with --created");
}

#[test]
fn parse_bug_my_all_conflicts_with_cc() {
    let result = Cli::try_parse_from(["bzr", "bug", "my", "--all", "--cc"]);
    assert!(result.is_err(), "--all should conflict with --cc");
}

#[test]
fn parse_bug_clone_minimal() {
    let cli = Cli::try_parse_from(["bzr", "bug", "clone", "123"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Clone { id, summary, .. },
        } => {
            assert_eq!(id, "123");
            assert!(summary.is_none());
        }
        _ => panic!("expected Bug Clone"),
    }
}

#[test]
fn parse_template_save_with_fields() {
    let cli = Cli::try_parse_from([
        "bzr",
        "template",
        "save",
        "security-bug",
        "--product",
        "Security",
        "--component",
        "Vulnerabilities",
        "--severity",
        "critical",
    ])
    .unwrap();
    match cli.command {
        Commands::Template {
            action: TemplateAction::Save { name, fields },
        } => {
            assert_eq!(name, "security-bug");
            assert_eq!(fields.product.as_deref(), Some("Security"));
            assert_eq!(fields.component.as_deref(), Some("Vulnerabilities"));
            assert_eq!(fields.severity.as_deref(), Some("critical"));
        }
        _ => panic!("expected Template Save"),
    }
}

#[test]
fn parse_query_save_list_kind() {
    let cli = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "firefox-new",
        "--product",
        "Firefox",
        "--status",
        "NEW",
        "--limit",
        "25",
    ])
    .unwrap();
    match cli.command {
        Commands::Query {
            action:
                QueryAction::Save {
                    name,
                    product,
                    status,
                    limit,
                    ..
                },
        } => {
            assert_eq!(name, "firefox-new");
            assert_eq!(product, vec!["Firefox"]);
            assert_eq!(status, vec!["NEW"]);
            assert_eq!(limit, Some(25));
        }
        _ => panic!("expected Query Save"),
    }
}

#[test]
fn parse_query_save_search_kind() {
    let cli = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "crashes",
        "--search",
        "crash in tab",
        "--limit",
        "10",
    ])
    .unwrap();
    match cli.command {
        Commands::Query {
            action:
                QueryAction::Save {
                    name,
                    search,
                    limit,
                    ..
                },
        } => {
            assert_eq!(name, "crashes");
            assert_eq!(search.as_deref(), Some("crash in tab"));
            assert_eq!(limit, Some(10));
        }
        _ => panic!("expected Query Save"),
    }
}

#[test]
fn parse_query_run() {
    let cli = Cli::try_parse_from(["bzr", "query", "run", "firefox-new"]).unwrap();
    match cli.command {
        Commands::Query {
            action: QueryAction::Run { name, limit, .. },
        } => {
            assert_eq!(name, "firefox-new");
            assert!(limit.is_none());
        }
        _ => panic!("expected Query Run"),
    }
}

#[test]
fn parse_query_run_with_limit_override() {
    let cli = Cli::try_parse_from(["bzr", "query", "run", "firefox-new", "--limit", "10"]).unwrap();
    match cli.command {
        Commands::Query {
            action: QueryAction::Run { name, limit, .. },
        } => {
            assert_eq!(name, "firefox-new");
            assert_eq!(limit, Some(10));
        }
        _ => panic!("expected Query Run"),
    }
}

#[test]
fn parse_query_list() {
    let cli = Cli::try_parse_from(["bzr", "query", "list"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Query {
            action: QueryAction::List
        }
    ));
}

#[test]
fn parse_query_show() {
    let cli = Cli::try_parse_from(["bzr", "query", "show", "firefox-new"]).unwrap();
    match cli.command {
        Commands::Query {
            action: QueryAction::Show { name },
        } => {
            assert_eq!(name, "firefox-new");
        }
        _ => panic!("expected Query Show"),
    }
}

#[test]
fn parse_query_delete() {
    let cli = Cli::try_parse_from(["bzr", "query", "delete", "firefox-new"]).unwrap();
    match cli.command {
        Commands::Query {
            action: QueryAction::Delete { name },
        } => {
            assert_eq!(name, "firefox-new");
        }
        _ => panic!("expected Query Delete"),
    }
}

#[test]
fn parse_set_server_tls_ca_cert() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-ca-cert",
        "/path/to/ca.pem",
    ])
    .unwrap();
    match cli.command {
        Commands::Config {
            action: ConfigAction::SetServer { tls_ca_cert, .. },
        } => assert_eq!(tls_ca_cert.as_deref(), Some("/path/to/ca.pem")),
        _ => panic!("expected Config SetServer"),
    }
}

#[test]
fn parse_set_server_tls_insecure_conflicts_with_ca_cert() {
    let result = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-insecure",
        "--tls-ca-cert",
        "/path/to/ca.pem",
    ]);
    assert!(result.is_err(), "should conflict");
}

#[test]
fn parse_set_server_tls_pin_now() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-pin-now",
    ])
    .unwrap();
    match cli.command {
        Commands::Config {
            action: ConfigAction::SetServer { tls_pin_now, .. },
        } => assert!(tls_pin_now),
        _ => panic!("expected Config SetServer"),
    }
}

#[test]
fn parse_set_server_tls_pin_sha256() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-pin-sha256",
        "sha256//abc123",
    ])
    .unwrap();
    match cli.command {
        Commands::Config {
            action: ConfigAction::SetServer { tls_pin_sha256, .. },
        } => assert_eq!(tls_pin_sha256.as_deref(), Some("sha256//abc123")),
        _ => panic!("expected Config SetServer"),
    }
}

#[test]
fn parse_set_server_tls_pin_clear() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-pin-clear",
    ])
    .unwrap();
    match cli.command {
        Commands::Config {
            action: ConfigAction::SetServer { tls_pin_clear, .. },
        } => assert!(tls_pin_clear),
        _ => panic!("expected Config SetServer"),
    }
}

#[test]
fn parse_set_server_tls_pin_sha256_conflicts_with_tls_insecure() {
    let result = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-insecure",
        "--tls-pin-sha256",
        "sha256//abc123",
    ]);
    assert!(
        result.is_err(),
        "--tls-insecure should conflict with --tls-pin-sha256"
    );
}

#[test]
fn parse_set_server_tls_pin_now_conflicts_with_tls_pin_sha256() {
    let result = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-pin-now",
        "--tls-pin-sha256",
        "sha256//abc123",
    ]);
    assert!(
        result.is_err(),
        "--tls-pin-now should conflict with --tls-pin-sha256"
    );
}

#[test]
fn parse_set_server_tls_pin_clear_conflicts_with_tls_pin_now() {
    let result = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "test",
        "--url",
        "https://example.com",
        "--api-key",
        "key",
        "--tls-pin-clear",
        "--tls-pin-now",
    ]);
    assert!(
        result.is_err(),
        "--tls-pin-clear should conflict with --tls-pin-now"
    );
}

#[test]
fn parse_whoami_without_subcommand() {
    let cli = Cli::try_parse_from(["bzr", "whoami"]).unwrap();
    assert!(matches!(cli.command, Commands::Whoami));
}

#[test]
fn parse_attachment_upload_with_summary() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "patch.diff",
        "--summary",
        "Fix crash",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Upload {
                    bug_id,
                    file,
                    summary,
                    ..
                },
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(file, "patch.diff");
            assert_eq!(summary.as_deref(), Some("Fix crash"));
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_with_private_flag() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "secret.bin",
        "--private",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Upload {
                bug_id, private, ..
            },
        } => {
            assert_eq!(bug_id, 42);
            assert!(private, "--private should set the flag to true");
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_without_private_defaults_to_false() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "upload", "42", "f.txt"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Upload { private, .. },
        } => assert!(!private, "--private absent should default to false"),
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_with_comment() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "patch.diff",
        "--comment",
        "see this",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Upload {
                    bug_id,
                    file,
                    comment,
                    ..
                },
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(file, "patch.diff");
            assert_eq!(comment.as_deref(), Some("see this"));
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_without_comment_defaults_to_none() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "upload", "42", "f.txt"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Upload { comment, .. },
        } => assert!(comment.is_none(), "--comment absent should default to None"),
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_with_patch_flag() {
    let cli =
        Cli::try_parse_from(["bzr", "attachment", "upload", "42", "fix.patch", "--patch"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Upload {
                    bug_id,
                    patch,
                    no_patch,
                    ..
                },
        } => {
            assert_eq!(bug_id, 42);
            assert!(patch, "--patch should set the flag to true");
            assert!(!no_patch);
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_without_patch_defaults_to_false() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "upload", "42", "f.txt"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Upload {
                patch, no_patch, ..
            },
        } => {
            assert!(!patch, "--patch absent should default to false");
            assert!(!no_patch);
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_no_patch_overrides_patch() {
    // overrides_with: the later flag wins, so --no-patch after --patch yields
    // patch=false / no_patch=true (resolves to Some(false)).
    let cli =
        Cli::try_parse_from(["bzr", "attachment", "update", "9", "--patch", "--no-patch"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Update {
                patch, no_patch, ..
            },
        } => {
            assert!(!patch, "--no-patch should override the earlier --patch");
            assert!(no_patch);
        }
        _ => panic!("expected Attachment Update"),
    }
}

#[test]
fn parse_attachment_update_old_value_boolean_is_rejected() {
    // The old `--is-patch true` value grammar is gone; clap now treats `true`
    // as an unexpected positional and errors.
    let err = Cli::try_parse_from(["bzr", "attachment", "update", "9", "--is-patch", "true"]);
    assert!(
        err.is_err(),
        "old --is-patch <BOOL> form must no longer parse"
    );
}

#[test]
fn parse_attachment_upload_with_comment_private() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "patch.diff",
        "--comment",
        "sensitive",
        "--comment-private",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Upload {
                    bug_id,
                    comment,
                    comment_private,
                    ..
                },
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(comment.as_deref(), Some("sensitive"));
            assert!(comment_private, "--comment-private should set the flag");
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_without_comment_private_defaults_to_false() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "upload", "42", "f.txt"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Upload {
                comment_private, ..
            },
        } => assert!(
            !comment_private,
            "--comment-private absent should default to false"
        ),
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_template_delete() {
    let cli = Cli::try_parse_from(["bzr", "template", "delete", "security-bug"]).unwrap();
    match cli.command {
        Commands::Template {
            action: TemplateAction::Delete { name },
        } => assert_eq!(name, "security-bug"),
        _ => panic!("expected Template Delete"),
    }
}

#[test]
fn bug_list_parses_created_since_and_changed_since() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--created-since",
        "2026-04-01",
        "--changed-since",
        "2026-04-15T12:00:00Z",
    ])
    .unwrap();
    let Commands::Bug {
        action:
            BugAction::List {
                created_since,
                changed_since,
                ..
            },
    } = cli.command
    else {
        panic!("expected Bug::List variant");
    };
    assert_eq!(created_since.as_deref(), Some("2026-04-01"));
    assert_eq!(changed_since.as_deref(), Some("2026-04-15T12:00:00Z"));
}

#[test]
fn bug_list_parses_158_field_filters() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "list",
        "--whiteboard",
        "wip",
        "--target-milestone",
        "5.0",
        "--version",
        "9.4",
        "--op-sys",
        "Linux",
        "--platform",
        "x86_64",
        "--resolution",
        "FIXED",
        "--qa-contact",
        "qa@example.com",
        "--url",
        "github.com/foo",
    ])
    .unwrap();
    let Commands::Bug {
        action:
            BugAction::List {
                whiteboard,
                target_milestone,
                version,
                op_sys,
                platform,
                resolution,
                qa_contact,
                url,
                ..
            },
    } = cli.command
    else {
        panic!("expected Bug::List variant");
    };
    assert_eq!(whiteboard, vec!["wip"]);
    assert_eq!(target_milestone, vec!["5.0"]);
    assert_eq!(version, vec!["9.4"]);
    assert_eq!(op_sys, vec!["Linux"]);
    assert_eq!(platform, vec!["x86_64"]);
    assert_eq!(resolution, vec!["FIXED"]);
    assert_eq!(qa_contact, vec!["qa@example.com"]);
    assert_eq!(url, vec!["github.com/foo"]);
}

#[test]
fn bug_list_parses_repeated_whiteboard() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "list",
        "--whiteboard",
        "wip",
        "--whiteboard",
        "review",
    ])
    .unwrap();
    let Commands::Bug {
        action: BugAction::List { whiteboard, .. },
    } = cli.command
    else {
        panic!("expected Bug::List variant");
    };
    assert_eq!(whiteboard, vec!["wip", "review"]);
}

#[test]
fn bug_list_parses_negated_whiteboard() {
    let cli = Cli::try_parse_from(["bzr", "bug", "list", "--whiteboard", "!wip"]).unwrap();
    let Commands::Bug {
        action: BugAction::List { whiteboard, .. },
    } = cli.command
    else {
        panic!("expected Bug::List variant");
    };
    assert_eq!(whiteboard, vec!["!wip"]);
}

#[test]
fn query_save_parses_158_field_filters() {
    let cli = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "field-filters",
        "--whiteboard",
        "wip",
        "--target-milestone",
        "5.0",
        "--version",
        "9.4",
        "--op-sys",
        "Linux",
        "--platform",
        "x86_64",
        "--resolution",
        "FIXED",
        "--qa-contact",
        "qa@example.com",
        "--url",
        "github.com/foo",
    ])
    .unwrap();
    let Commands::Query {
        action:
            QueryAction::Save {
                whiteboard,
                target_milestone,
                version,
                op_sys,
                platform,
                resolution,
                qa_contact,
                url,
                ..
            },
    } = cli.command
    else {
        panic!("expected Query::Save variant");
    };
    assert_eq!(whiteboard, vec!["wip"]);
    assert_eq!(target_milestone, vec!["5.0"]);
    assert_eq!(version, vec!["9.4"]);
    assert_eq!(op_sys, vec!["Linux"]);
    assert_eq!(platform, vec!["x86_64"]);
    assert_eq!(resolution, vec!["FIXED"]);
    assert_eq!(qa_contact, vec!["qa@example.com"]);
    assert_eq!(url, vec!["github.com/foo"]);
}

#[test]
fn query_run_parses_158_field_filter_overrides() {
    let cli = Cli::try_parse_from([
        "bzr",
        "query",
        "run",
        "saved-q",
        "--whiteboard",
        "overridden",
        "--target-milestone",
        "6.0",
        "--version",
        "10.0",
        "--op-sys",
        "Windows",
        "--platform",
        "arm64",
        "--resolution",
        "WONTFIX",
        "--qa-contact",
        "newqa@example.com",
        "--url",
        "gitlab.com/x",
    ])
    .unwrap();
    let Commands::Query {
        action:
            QueryAction::Run {
                whiteboard,
                target_milestone,
                version,
                op_sys,
                platform,
                resolution,
                qa_contact,
                url,
                ..
            },
    } = cli.command
    else {
        panic!("expected Query::Run variant");
    };
    assert_eq!(whiteboard, vec!["overridden"]);
    assert_eq!(target_milestone, vec!["6.0"]);
    assert_eq!(version, vec!["10.0"]);
    assert_eq!(op_sys, vec!["Windows"]);
    assert_eq!(platform, vec!["arm64"]);
    assert_eq!(resolution, vec!["WONTFIX"]);
    assert_eq!(qa_contact, vec!["newqa@example.com"]);
    assert_eq!(url, vec!["gitlab.com/x"]);
}

#[test]
fn parse_bug_update_keywords_add_comma_list() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "100",
        "--keywords-add",
        "fix-needed,regression",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Update { keywords_add, .. },
        } => {
            assert_eq!(keywords_add, vec!["fix-needed", "regression"]);
        }
        _ => panic!("expected Bug Update"),
    }
}

#[test]
fn parse_bug_update_cc_add_comma_list() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "100",
        "--cc-add",
        "a@example.com,b@example.com",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Update { cc_add, .. },
        } => {
            assert_eq!(cc_add, vec!["a@example.com", "b@example.com"]);
        }
        _ => panic!("expected Bug Update"),
    }
}

#[test]
fn parse_bug_update_groups_remove_comma_list() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "100",
        "--groups-remove",
        "secret,internal",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Update { groups_remove, .. },
        } => {
            assert_eq!(groups_remove, vec!["secret", "internal"]);
        }
        _ => panic!("expected Bug Update"),
    }
}

#[test]
fn parse_query_save_rejects_search_with_filter_flag() {
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "crashes",
        "--search",
        "crash in tab",
        "--product",
        "Firefox",
    ]);

    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_query_save_rejects_search_with_bzl_parity_filter() {
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "crashes",
        "--search",
        "crash in tab",
        "--whiteboard",
        "needs-triage",
    ]);

    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_query_save_rejects_search_with_from_url() {
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "crashes",
        "--search",
        "crash in tab",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
    ]);

    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_query_save_search_alone_is_accepted() {
    let cli = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "crashes",
        "--search",
        "crash in tab",
    ])
    .unwrap();
    match cli.command {
        Commands::Query {
            action: QueryAction::Save { name, search, .. },
        } => {
            assert_eq!(name, "crashes");
            assert_eq!(search.as_deref(), Some("crash in tab"));
        }
        _ => panic!("expected Query Save"),
    }
}

#[test]
fn parse_query_save_rejects_from_url_with_filter_flag() {
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "saved",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
        "--product",
        "Firefox",
    ]);

    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_query_save_rejects_from_url_with_bzl_parity_filter() {
    // Exercises the centralized FILTER_FLAG_ARGS conflict list on the
    // --from-url side via a bzl-parity filter (target_milestone).
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "save",
        "saved",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
        "--target-milestone",
        "9.0",
    ]);

    match result {
        Ok(_) => panic!("expected ArgumentConflict, got Ok"),
        Err(err) => assert!(
            err.kind() == clap::error::ErrorKind::ArgumentConflict,
            "expected ArgumentConflict, got {:?}",
            err.kind()
        ),
    }
}

#[test]
fn parse_bug_update_see_also_add_repeated_flag() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "100",
        "--see-also-add",
        "https://a.example/?x=1,y=2",
        "--see-also-add",
        "https://b.example/issue/3",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::Update { see_also_add, .. },
        } => {
            assert_eq!(
                see_also_add,
                vec!["https://a.example/?x=1,y=2", "https://b.example/issue/3"]
            );
        }
        _ => panic!("expected Bug Update"),
    }
}
