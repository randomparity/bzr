#![expect(clippy::unwrap_used)]

use clap::Parser;

use crate::cli::{AuthAction, Cli, Commands};

#[test]
fn parses_login_with_restrict_login() {
    let cli = Cli::try_parse_from([
        "bzr",
        "auth",
        "login",
        "--email",
        "a@example.test",
        "--password",
        "secret",
        "--restrict-login",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Commands::Auth {
            action: AuthAction::Login {
                password: Some(_),
                restrict_login: true,
                ..
            }
        }
    ));
}

#[test]
fn parses_logout() {
    let cli = Cli::try_parse_from(["bzr", "auth", "logout"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Auth {
            action: AuthAction::Logout
        }
    ));
}
