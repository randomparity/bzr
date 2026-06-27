#![expect(clippy::unwrap_used, clippy::panic)]

use super::{
    AttachmentAction, BugAction, ClassificationAction, Cli, Commands, CommentAction,
    ComponentAction, ConfigAction, FieldAction, GroupAction, ProductAction, QueryAction,
    ServerAction, TemplateAction, UserAction,
};
use crate::types::{ApiMode, ProductListType};
use clap::{Command, CommandFactory as _, Parser as _};

fn display_command_path(path: &[String]) -> String {
    if path.is_empty() {
        "bzr".to_string()
    } else {
        format!("bzr {}", path.join(" "))
    }
}

fn assert_long_about_coverage(command: &Command, path: &mut Vec<String>) {
    let command_path = display_command_path(path);
    let about = command.get_about().map(ToString::to_string);
    let long_about = command.get_long_about().map(ToString::to_string);
    assert!(
        long_about.is_some(),
        "command `{command_path}` is missing long_about \
         (multi-paragraph doc comment required)"
    );
    assert_ne!(
        about, long_about,
        "command `{command_path}` has long_about identical to about \
         (single-paragraph doc -- expand to multi-paragraph per docs/dev/cli-doc-style.md)"
    );

    for subcommand in command.get_subcommands() {
        path.push(subcommand.get_name().to_string());
        assert_long_about_coverage(subcommand, path);
        path.pop();
    }
}

#[test]
fn cli_doc_long_about_coverage() {
    let cmd = Cli::command();
    assert_long_about_coverage(&cmd, &mut Vec::new());
}

#[test]
fn version_output_includes_package_version_and_build_metadata() {
    let Err(err) = Cli::try_parse_from(["bzr", "--version"]) else {
        panic!("expected --version to return a display-version clap error");
    };

    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    let version = err.to_string();
    assert!(version.contains(env!("CARGO_PKG_VERSION")));
    assert!(version.contains(env!("BZR_GIT_SHA")));
    assert!(version.contains('('));
    assert!(version.contains(')'));
}

#[test]
fn parse_bug_list_minimal() {
    let cli = Cli::try_parse_from(["bzr", "bug", "list"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Bug {
            action: BugAction::List(super::ListArgs { .. })
        }
    ));
}

#[test]
fn parse_bug_view_by_id() {
    let cli = Cli::try_parse_from(["bzr", "bug", "view", "12345"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::View(super::ViewArgs { ids, .. }),
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
        action:
            BugAction::Update(super::UpdateArgs {
                expect_unchanged_since,
                ..
            }),
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
fn parse_bug_verbs_expect_unchanged_since() {
    let since = "2026-06-19T12:00:00Z";
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (
            vec![
                "bzr",
                "bug",
                "resolve",
                "42",
                "--expect-unchanged-since",
                since,
            ],
            "resolve",
        ),
        (
            vec![
                "bzr",
                "bug",
                "close",
                "42",
                "--expect-unchanged-since",
                since,
            ],
            "close",
        ),
        (
            vec![
                "bzr",
                "bug",
                "reopen",
                "42",
                "--expect-unchanged-since",
                since,
            ],
            "reopen",
        ),
        (
            vec![
                "bzr",
                "bug",
                "dup",
                "42",
                "99",
                "--expect-unchanged-since",
                since,
            ],
            "dup",
        ),
    ];

    for (argv, name) in cases {
        let cli = Cli::try_parse_from(argv).unwrap();
        let Commands::Bug { action } = cli.command else {
            panic!("expected bug command for {name}");
        };
        let parsed = match action {
            BugAction::Resolve(args) => args.expect_unchanged_since,
            BugAction::Close(args) => args.expect_unchanged_since,
            BugAction::Reopen(args) => args.expect_unchanged_since,
            BugAction::Dup(args) => args.expect_unchanged_since,
            _ => panic!("expected {name} action"),
        };
        assert_eq!(parsed.as_deref(), Some(since), "{name}");
    }
}

#[test]
fn parse_bug_update_url_and_target_milestone() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "update",
        "42",
        "--url",
        "https://example.com/repro",
        "--target-milestone",
        "5.0",
    ])
    .unwrap();
    let Commands::Bug {
        action:
            BugAction::Update(super::UpdateArgs {
                url,
                target_milestone,
                ..
            }),
    } = cli.command
    else {
        panic!("expected bug update");
    };
    assert_eq!(url.as_deref(), Some("https://example.com/repro"));
    assert_eq!(target_milestone.as_deref(), Some("5.0"));
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
fn inline_server_url_no_longer_requires_api_key_env() {
    let cli = Cli::try_parse_from([
        "bzr",
        "--server-url",
        "https://bz.example.com",
        "bug",
        "view",
        "42",
    ])
    .unwrap();

    assert_eq!(cli.server_url.as_deref(), Some("https://bz.example.com"));
    assert_eq!(cli.server_api_key_env, None);
}

#[test]
fn inline_server_api_key_env_still_requires_url() {
    let result =
        Cli::try_parse_from(["bzr", "--server-api-key-env", "BZR_KEY", "bug", "view", "1"]);
    assert!(
        result.is_err(),
        "--server-api-key-env without --server-url must fail"
    );
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
fn parse_inline_server_tls_flags() {
    let cases = [
        vec![
            "bzr",
            "--server-url",
            "https://bz.example.com",
            "--server-tls-insecure",
            "bug",
            "view",
            "42",
        ],
        vec![
            "bzr",
            "--server-url",
            "https://bz.example.com",
            "--server-tls-ca-cert",
            "/path/to/ca.pem",
            "bug",
            "view",
            "42",
        ],
        vec![
            "bzr",
            "--server-url",
            "https://bz.example.com",
            "--server-tls-pin-sha256",
            "sha256//abc123",
            "bug",
            "view",
            "42",
        ],
        vec![
            "bzr",
            "--server-url",
            "https://bz.example.com",
            "--server-tls-pin-now",
            "bug",
            "view",
            "42",
        ],
    ];

    for argv in cases {
        let result = Cli::try_parse_from(argv);
        assert!(
            result.is_ok(),
            "ad-hoc TLS flags should parse: {:?}",
            result.err()
        );
    }
}

#[test]
fn inline_server_tls_flags_require_url() {
    let result = Cli::try_parse_from(["bzr", "--server-tls-insecure", "bug", "view", "1"]);
    assert!(
        result.is_err(),
        "--server-tls-insecure without --server-url must fail"
    );
}

#[test]
fn inline_server_tls_choices_are_mutually_exclusive() {
    let result = Cli::try_parse_from([
        "bzr",
        "--server-url",
        "https://bz.example.com",
        "--server-tls-insecure",
        "--server-tls-ca-cert",
        "/path/to/ca.pem",
        "bug",
        "view",
        "1",
    ]);
    assert!(
        result.is_err(),
        "--server-tls-insecure should conflict with --server-tls-ca-cert"
    );

    let result = Cli::try_parse_from([
        "bzr",
        "--server-url",
        "https://bz.example.com",
        "--server-tls-pin-now",
        "--server-tls-pin-sha256",
        "sha256//abc123",
        "bug",
        "view",
        "1",
    ]);
    assert!(
        result.is_err(),
        "--server-tls-pin-now should conflict with --server-tls-pin-sha256"
    );
}

#[test]
fn parse_bug_create_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "bug", "create", "--from-json", "-"]).unwrap();
    let Commands::Bug {
        action: BugAction::Create(super::CreateArgs { from_json, .. }),
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
fn parse_bug_update_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "bug", "update", "--from-json", "-"]).unwrap();
    let Commands::Bug {
        action: BugAction::Update(super::UpdateArgs { from_json, ids, .. }),
    } = cli.command
    else {
        panic!("expected bug update");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
    assert!(ids.is_empty());
}

#[test]
fn parse_product_create_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "product", "create", "--from-json", "-"]).unwrap();
    let Commands::Product {
        action: ProductAction::Create { from_json, .. },
    } = cli.command
    else {
        panic!("expected product create");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
}

#[test]
fn parse_product_update_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "product", "update", "--from-json", "-"]).unwrap();
    let Commands::Product {
        action: ProductAction::Update {
            from_json, name, ..
        },
    } = cli.command
    else {
        panic!("expected product update");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
    assert!(name.is_none());
}

#[test]
fn parse_component_create_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "component", "create", "--from-json", "-"]).unwrap();
    let Commands::Component {
        action: ComponentAction::Create { from_json, .. },
    } = cli.command
    else {
        panic!("expected component create");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
}

#[test]
fn parse_component_update_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "component", "update", "--from-json", "-"]).unwrap();
    let Commands::Component {
        action: ComponentAction::Update { from_json, id, .. },
    } = cli.command
    else {
        panic!("expected component update");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
    assert!(id.is_none());
}

#[test]
fn parse_component_update_with_product_component_target() {
    let cli = Cli::try_parse_from([
        "bzr",
        "component",
        "update",
        "--product",
        "MyApp",
        "--component",
        "Backend",
        "--description",
        "Updated",
    ])
    .unwrap();
    let Commands::Component {
        action:
            ComponentAction::Update {
                id,
                product,
                component,
                description,
                ..
            },
    } = cli.command
    else {
        panic!("expected component update");
    };
    assert!(id.is_none());
    assert_eq!(product.as_deref(), Some("MyApp"));
    assert_eq!(component.as_deref(), Some("Backend"));
    assert_eq!(description.as_deref(), Some("Updated"));
}

#[test]
fn parse_user_create_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "user", "create", "--from-json", "-"]).unwrap();
    let Commands::User {
        action: UserAction::Create { from_json, .. },
    } = cli.command
    else {
        panic!("expected user create");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
}

#[test]
fn parse_user_update_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "user", "update", "--from-json", "-"]).unwrap();
    let Commands::User {
        action: UserAction::Update {
            from_json, user, ..
        },
    } = cli.command
    else {
        panic!("expected user update");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
    assert!(user.is_none());
}

#[test]
fn parse_group_create_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "group", "create", "--from-json", "-"]).unwrap();
    let Commands::Group {
        action: GroupAction::Create { from_json, .. },
    } = cli.command
    else {
        panic!("expected group create");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
}

#[test]
fn parse_group_update_from_json_stdin() {
    let cli = Cli::try_parse_from(["bzr", "group", "update", "--from-json", "-"]).unwrap();
    let Commands::Group {
        action: GroupAction::Update {
            from_json, group, ..
        },
    } = cli.command
    else {
        panic!("expected group update");
    };
    assert_eq!(from_json.as_deref(), Some("-"));
    assert!(group.is_none());
}

#[test]
fn parse_bug_list_offset_and_paginate() {
    let cli =
        Cli::try_parse_from(["bzr", "bug", "list", "--limit", "10", "--offset", "20"]).unwrap();
    let Commands::Bug {
        action: BugAction::List(super::ListArgs { page_args, .. }),
    } = cli.command
    else {
        panic!("expected bug list");
    };
    assert_eq!(page_args.offset, Some(20));
    assert!(!page_args.paginate);
}

#[test]
fn bug_list_offset_conflicts_with_paginate() {
    let result = Cli::try_parse_from(["bzr", "bug", "list", "--offset", "20", "--paginate"]);
    assert!(
        result.is_err(),
        "--offset and --paginate must be mutually exclusive"
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
fn parse_config_set_server_without_api_key_source() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "public",
        "--url",
        "https://bz.example.com",
    ])
    .unwrap();

    assert!(matches!(
        cli.command,
        Commands::Config {
            action: ConfigAction::SetServer {
                api_key: None,
                api_key_env: None,
                ..
            }
        }
    ));
}

#[test]
fn parse_config_set_server_auth_method_accepts_documented_query_param() {
    let cli = Cli::try_parse_from([
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz.example.com",
        "--auth-method",
        "query-param",
    ])
    .unwrap();

    assert!(matches!(
        cli.command,
        Commands::Config {
            action: ConfigAction::SetServer {
                auth_method: Some(crate::types::AuthMethod::QueryParam),
                ..
            }
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
            action: BugAction::Search(super::SearchArgs { query, limit, .. }),
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
            action: BugAction::Search(super::SearchArgs { limit, .. }),
        } => assert_eq!(limit, Some(10)),
        _ => panic!("expected Bug Search"),
    }
}

#[test]
fn parse_bug_history() {
    let cli = Cli::try_parse_from(["bzr", "bug", "history", "42"]).unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::History(super::HistoryArgs { id, since }),
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
                BugAction::Create(super::CreateArgs {
                    product,
                    component,
                    summary,
                    version,
                    ..
                }),
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
            action:
                BugAction::Create(super::CreateArgs {
                    description_file, ..
                }),
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
            action:
                BugAction::Update(super::UpdateArgs {
                    ids, status, flag, ..
                }),
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
                BugAction::Update(super::UpdateArgs {
                    ids,
                    dupe_of,
                    status,
                    resolution,
                    ..
                }),
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
    let BugAction::Update(super::UpdateArgs {
        alias,
        deadline,
        estimated_time,
        remaining_time,
        work_time,
        reset_assigned_to,
        reset_qa_contact,
        ..
    }) = action
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
                BugAction::Update(super::UpdateArgs {
                    ids,
                    comment,
                    comment_file,
                    comment_private,
                    ..
                }),
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
                BugAction::Update(super::UpdateArgs {
                    comment,
                    comment_file,
                    comment_private,
                    ..
                }),
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
            action: AttachmentAction::List { bug_id, .. },
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
fn parse_attachment_download_with_out_dash() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "download", "100", "--out", "-"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Download { ids, out, .. },
        } => {
            assert_eq!(ids, vec![100]);
            assert_eq!(out.as_deref(), Some("-"));
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
            action: ProductAction::List { r#type, .. },
        } => assert_eq!(r#type, ProductListType::Accessible),
        _ => panic!("expected Product List"),
    }
}

#[test]
fn parse_product_view() {
    let cli = Cli::try_parse_from(["bzr", "product", "view", "Firefox"]).unwrap();
    match cli.command {
        Commands::Product {
            action: ProductAction::View { name, .. },
        } => assert_eq!(name, "Firefox"),
        _ => panic!("expected Product View"),
    }
}

#[test]
fn parse_user_search() {
    let cli = Cli::try_parse_from(["bzr", "user", "search", "alice"]).unwrap();
    match cli.command {
        Commands::User {
            action: UserAction::Search { query, details, .. },
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
            action: FieldAction::List { name, .. },
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
            action: ClassificationAction::View { name, .. },
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
                    from_json,
                    product,
                    name,
                    description,
                    default_assignee,
                },
        } => {
            assert_eq!(from_json, None);
            assert_eq!(product.as_deref(), Some("TestProduct"));
            assert_eq!(name.as_deref(), Some("Backend"));
            assert_eq!(description.as_deref(), Some("Backend component"));
            assert_eq!(default_assignee.as_deref(), Some("dev@test.com"));
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
fn api_help_describes_transport_preference_not_exclusivity() {
    let help = Cli::command().render_long_help().to_string();
    assert!(
        help.contains("`rest` prefers Bugzilla's REST API"),
        "{help}"
    );
    assert!(help.contains("transport-specific exceptions"), "{help}");
    assert!(!help.contains("REST API exclusively"), "{help}");
    assert!(!help.contains("XML-RPC for every call"), "{help}");
}

#[test]
fn migrate_to_keyring_help_requires_yes_without_prompt_text() {
    let mut command = Cli::command();
    let Some(config) = command.find_subcommand_mut("config") else {
        panic!("config subcommand exists");
    };
    let Some(migrate) = config.find_subcommand_mut("migrate-to-keyring") else {
        panic!("config migrate-to-keyring subcommand exists");
    };
    let help = migrate.render_long_help().to_string();

    assert!(help.contains("`--yes`"), "{help}");
    assert!(help.contains("required to confirm"), "{help}");
    assert!(!help.contains("confirmation prompt"), "{help}");
    assert!(!help.contains("waits for a"), "{help}");
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
        "--url",
        "github.com/foo",
        "--limit",
        "25",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action: BugAction::List(super::ListArgs { filters, limit, .. }),
        } => {
            assert_eq!(filters.product, vec!["Firefox"]);
            assert_eq!(filters.status, vec!["NEW"]);
            assert_eq!(filters.url, vec!["github.com/foo"]);
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
            action: BugAction::List(super::ListArgs { summary, .. }),
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
                BugAction::My(super::MyArgs {
                    created,
                    cc,
                    all,
                    limit,
                    ..
                }),
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
fn bug_my_parses_shared_filter_set() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "my",
        "--product",
        "Core",
        "--component",
        "Networking",
        "--priority",
        "P1",
        "--severity",
        "S2",
        "--created-since",
        "2026-04-01",
        "--changed-since",
        "2026-04-15T12:00:00Z",
        "--whiteboard",
        "needs-review",
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
            BugAction::My(super::MyArgs {
                filters,
                created_since,
                changed_since,
                ..
            }),
    } = cli.command
    else {
        panic!("expected Bug My");
    };
    assert_eq!(filters.product, vec!["Core"]);
    assert_eq!(filters.component, vec!["Networking"]);
    assert_eq!(filters.priority, vec!["P1"]);
    assert_eq!(filters.severity, vec!["S2"]);
    assert_eq!(created_since.as_deref(), Some("2026-04-01"));
    assert_eq!(changed_since.as_deref(), Some("2026-04-15T12:00:00Z"));
    assert_eq!(filters.whiteboard, vec!["needs-review"]);
    assert_eq!(filters.target_milestone, vec!["5.0"]);
    assert_eq!(filters.version, vec!["9.4"]);
    assert_eq!(filters.op_sys, vec!["Linux"]);
    assert_eq!(filters.platform, vec!["x86_64"]);
    assert_eq!(filters.resolution, vec!["FIXED"]);
    assert_eq!(filters.qa_contact, vec!["qa@example.com"]);
    assert_eq!(filters.url, vec!["github.com/foo"]);
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
            action: BugAction::Clone(super::CloneArgs { id, summary, .. }),
        } => {
            assert_eq!(id, "123");
            assert!(summary.is_none());
        }
        _ => panic!("expected Bug Clone"),
    }
}

#[test]
fn parse_bug_clone_with_create_metadata_overrides() {
    let cli = Cli::try_parse_from([
        "bzr",
        "bug",
        "clone",
        "123",
        "--url",
        "https://example.com/repro",
        "--whiteboard",
        "needs-routing",
        "--target-milestone",
        "M1",
        "--deadline",
        "2026-12-31",
        "--cc",
        "a@example.com,b@example.com",
        "--keywords",
        "regression,security",
        "--groups",
        "confidential",
        "--flag",
        "review?(qa@example.com)",
    ])
    .unwrap();
    match cli.command {
        Commands::Bug {
            action:
                BugAction::Clone(super::CloneArgs {
                    id, create_fields, ..
                }),
        } => {
            assert_eq!(id, "123");
            assert_eq!(
                create_fields.url.as_deref(),
                Some("https://example.com/repro")
            );
            assert_eq!(create_fields.whiteboard.as_deref(), Some("needs-routing"));
            assert_eq!(create_fields.target_milestone.as_deref(), Some("M1"));
            assert_eq!(create_fields.deadline.as_deref(), Some("2026-12-31"));
            assert_eq!(create_fields.cc, vec!["a@example.com", "b@example.com"]);
            assert_eq!(create_fields.keywords, vec!["regression", "security"]);
            assert_eq!(create_fields.groups, vec!["confidential"]);
            assert_eq!(create_fields.flag, vec!["review?(qa@example.com)"]);
        }
        _ => panic!("expected Bug Clone"),
    }
}

#[test]
fn parse_bug_clone_cc_override_conflicts_with_no_cc() {
    let result = Cli::try_parse_from([
        "bzr",
        "bug",
        "clone",
        "123",
        "--no-cc",
        "--cc",
        "a@example.com",
    ]);
    let Err(err) = result else {
        panic!("--cc should conflict with --no-cc");
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn parse_bug_clone_keywords_override_conflicts_with_no_keywords() {
    let result = Cli::try_parse_from([
        "bzr",
        "bug",
        "clone",
        "123",
        "--no-keywords",
        "--keywords",
        "regression",
    ]);
    let Err(err) = result else {
        panic!("--keywords should conflict with --no-keywords");
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
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
fn parse_template_save_with_create_metadata_fields() {
    let cli = Cli::try_parse_from([
        "bzr",
        "template",
        "save",
        "routing",
        "--url",
        "https://example.com/repro",
        "--whiteboard",
        "needs-triage",
        "--target-milestone",
        "M1",
        "--deadline",
        "2026-12-31",
        "--cc",
        "a@example.com,b@example.com",
        "--keywords",
        "regression,security",
        "--groups",
        "confidential",
        "--flag",
        "review?(qa@example.com)",
    ])
    .unwrap();
    match cli.command {
        Commands::Template {
            action: TemplateAction::Save { name, fields },
        } => {
            assert_eq!(name, "routing");
            assert_eq!(fields.url.as_deref(), Some("https://example.com/repro"));
            assert_eq!(fields.whiteboard.as_deref(), Some("needs-triage"));
            assert_eq!(fields.target_milestone.as_deref(), Some("M1"));
            assert_eq!(fields.deadline.as_deref(), Some("2026-12-31"));
            assert_eq!(fields.cc, vec!["a@example.com", "b@example.com"]);
            assert_eq!(fields.keywords, vec!["regression", "security"]);
            assert_eq!(fields.groups, vec!["confidential"]);
            assert_eq!(fields.flag, vec!["review?(qa@example.com)"]);
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
                QueryAction::Save(super::SaveArgs {
                    name,
                    filters,
                    limit,
                    ..
                }),
        } => {
            assert_eq!(name, "firefox-new");
            assert_eq!(filters.product, vec!["Firefox"]);
            assert_eq!(filters.status, vec!["NEW"]);
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
                QueryAction::Save(super::SaveArgs {
                    name,
                    search,
                    limit,
                    ..
                }),
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
            action: QueryAction::Run(super::RunArgs { name, limit, .. }),
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
            action: QueryAction::Run(super::RunArgs { name, limit, .. }),
        } => {
            assert_eq!(name, "firefox-new");
            assert_eq!(limit, Some(10));
        }
        _ => panic!("expected Query Run"),
    }
}

#[test]
fn parse_query_run_count() {
    let cli = Cli::try_parse_from(["bzr", "query", "run", "firefox-new", "--count"]).unwrap();
    match cli.command {
        Commands::Query {
            action: QueryAction::Run(super::RunArgs { name, count, .. }),
        } => {
            assert_eq!(name, "firefox-new");
            assert!(count);
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
            action: QueryAction::Show(super::ShowArgs { name }),
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
            action: QueryAction::Delete(super::DeleteArgs { name }),
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
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id,
                    file,
                    summary,
                    ..
                }),
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
            action:
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id, private, ..
                }),
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
            action: AttachmentAction::Upload(super::UploadArgs { private, .. }),
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
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id,
                    file,
                    comment,
                    ..
                }),
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(file, "patch.diff");
            assert_eq!(comment.as_deref(), Some("see this"));
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_with_comment_dash() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "patch.diff",
        "--comment",
        "-",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id, comment, ..
                }),
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(comment.as_deref(), Some("-"));
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_with_comment_file() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "patch.diff",
        "--comment-file",
        "notes.md",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id,
                    comment_file,
                    ..
                }),
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(
                comment_file.as_deref(),
                Some(std::path::Path::new("notes.md"))
            );
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_with_comment_file_dash() {
    let cli = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "patch.diff",
        "--comment-file",
        "-",
    ])
    .unwrap();
    match cli.command {
        Commands::Attachment {
            action:
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id,
                    comment_file,
                    ..
                }),
        } => {
            assert_eq!(bug_id, 42);
            assert_eq!(comment_file.as_deref(), Some(std::path::Path::new("-")));
        }
        _ => panic!("expected Attachment Upload"),
    }
}

#[test]
fn parse_attachment_upload_comment_and_comment_file_conflict() {
    let result = Cli::try_parse_from([
        "bzr",
        "attachment",
        "upload",
        "42",
        "patch.diff",
        "--comment",
        "inline",
        "--comment-file",
        "notes.md",
    ]);
    let Err(err) = result else {
        panic!("--comment and --comment-file should conflict");
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn parse_attachment_upload_without_comment_defaults_to_none() {
    let cli = Cli::try_parse_from(["bzr", "attachment", "upload", "42", "f.txt"]).unwrap();
    match cli.command {
        Commands::Attachment {
            action: AttachmentAction::Upload(super::UploadArgs { comment, .. }),
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
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id,
                    patch,
                    no_patch,
                    ..
                }),
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
            action:
                AttachmentAction::Upload(super::UploadArgs {
                    patch, no_patch, ..
                }),
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
            action:
                AttachmentAction::Update(super::AttachmentUpdateArgs {
                    patch, no_patch, ..
                }),
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
                AttachmentAction::Upload(super::UploadArgs {
                    bug_id,
                    comment,
                    comment_private,
                    ..
                }),
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
            action:
                AttachmentAction::Upload(super::UploadArgs {
                    comment_private, ..
                }),
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
            BugAction::List(super::ListArgs {
                created_since,
                changed_since,
                ..
            }),
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
        action: BugAction::List(super::ListArgs { filters, .. }),
    } = cli.command
    else {
        panic!("expected Bug::List variant");
    };
    assert_eq!(filters.whiteboard, vec!["wip"]);
    assert_eq!(filters.target_milestone, vec!["5.0"]);
    assert_eq!(filters.version, vec!["9.4"]);
    assert_eq!(filters.op_sys, vec!["Linux"]);
    assert_eq!(filters.platform, vec!["x86_64"]);
    assert_eq!(filters.resolution, vec!["FIXED"]);
    assert_eq!(filters.qa_contact, vec!["qa@example.com"]);
    assert_eq!(filters.url, vec!["github.com/foo"]);
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
        action: BugAction::List(super::ListArgs { filters, .. }),
    } = cli.command
    else {
        panic!("expected Bug::List variant");
    };
    assert_eq!(filters.whiteboard, vec!["wip", "review"]);
}

#[test]
fn bug_list_parses_negated_whiteboard() {
    let cli = Cli::try_parse_from(["bzr", "bug", "list", "--whiteboard", "!wip"]).unwrap();
    let Commands::Bug {
        action: BugAction::List(super::ListArgs { filters, .. }),
    } = cli.command
    else {
        panic!("expected Bug::List variant");
    };
    assert_eq!(filters.whiteboard, vec!["!wip"]);
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
        action: QueryAction::Save(super::SaveArgs { filters, .. }),
    } = cli.command
    else {
        panic!("expected Query::Save variant");
    };
    assert_eq!(filters.whiteboard, vec!["wip"]);
    assert_eq!(filters.target_milestone, vec!["5.0"]);
    assert_eq!(filters.version, vec!["9.4"]);
    assert_eq!(filters.op_sys, vec!["Linux"]);
    assert_eq!(filters.platform, vec!["x86_64"]);
    assert_eq!(filters.resolution, vec!["FIXED"]);
    assert_eq!(filters.qa_contact, vec!["qa@example.com"]);
    assert_eq!(filters.url, vec!["github.com/foo"]);
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
        action: QueryAction::Run(super::RunArgs { filters, url, .. }),
    } = cli.command
    else {
        panic!("expected Query::Run variant");
    };
    assert_eq!(filters.whiteboard, vec!["overridden"]);
    assert_eq!(filters.target_milestone, vec!["6.0"]);
    assert_eq!(filters.version, vec!["10.0"]);
    assert_eq!(filters.op_sys, vec!["Windows"]);
    assert_eq!(filters.platform, vec!["arm64"]);
    assert_eq!(filters.resolution, vec!["WONTFIX"]);
    assert_eq!(filters.qa_contact, vec!["newqa@example.com"]);
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
            action: BugAction::Update(super::UpdateArgs { keywords_add, .. }),
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
            action: BugAction::Update(super::UpdateArgs { cc_add, .. }),
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
            action: BugAction::Update(super::UpdateArgs { groups_remove, .. }),
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
            action: QueryAction::Save(super::SaveArgs { name, search, .. }),
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
fn parse_query_update_accepts_from_url_with_refresh_overrides() {
    let cli = Cli::try_parse_from([
        "bzr",
        "query",
        "update",
        "saved",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
        "--limit",
        "25",
        "--fields",
        "id,summary",
        "--exclude-fields",
        "creator",
        "--changed-since",
        "2026-04-01",
        "--sort",
        "priority",
    ])
    .unwrap();

    match cli.command {
        Commands::Query {
            action:
                QueryAction::Update(super::QueryUpdateArgs {
                    name,
                    from_url,
                    limit,
                    fields,
                    exclude_fields,
                    changed_since,
                    sort_args,
                    ..
                }),
        } => {
            assert_eq!(name, "saved");
            assert_eq!(
                from_url.as_deref(),
                Some("https://bz/buglist.cgi?product=Firefox")
            );
            assert_eq!(limit, Some(25));
            assert_eq!(fields.as_deref(), Some("id,summary"));
            assert_eq!(exclude_fields.as_deref(), Some("creator"));
            assert_eq!(changed_since.as_deref(), Some("2026-04-01"));
            assert_eq!(sort_args.sort.as_deref(), Some("priority"));
        }
        _ => panic!("expected Query Update"),
    }
}

#[test]
fn parse_query_update_rejects_from_url_with_filter_flag() {
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "update",
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
fn parse_query_update_rejects_from_url_with_search() {
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "update",
        "saved",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
        "--search",
        "crash",
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
fn parse_query_update_rejects_from_url_with_clear() {
    let result = Cli::try_parse_from([
        "bzr",
        "query",
        "update",
        "saved",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
        "--clear",
        "status",
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
            action: BugAction::Update(super::UpdateArgs { see_also_add, .. }),
        } => {
            assert_eq!(
                see_also_add,
                vec!["https://a.example/?x=1,y=2", "https://b.example/issue/3"]
            );
        }
        _ => panic!("expected Bug Update"),
    }
}
