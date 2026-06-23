#![expect(clippy::unwrap_used, clippy::panic)]

use super::ConfigAction;
use crate::cli::{Cli, Commands};
use crate::types::common::AuthMethod;
use clap::error::ErrorKind;
use clap::Parser as _;

fn config_action(args: &[&str]) -> ConfigAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Config { action } => action,
        _ => panic!("expected Commands::Config"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_config_set_server_minimal_defaults() {
    match config_action(&[
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz.example.com",
    ]) {
        ConfigAction::SetServer {
            name,
            url,
            api_key,
            api_key_env,
            email,
            auth_method,
            tls_insecure,
            tls_ca_cert,
            tls_pin_sha256,
            tls_pin_now,
            tls_pin_clear,
        } => {
            assert_eq!(name, "prod");
            assert_eq!(url, "https://bz.example.com");
            assert!(api_key.is_none());
            assert!(api_key_env.is_none());
            assert!(email.is_none());
            assert!(auth_method.is_none());
            assert!(!tls_insecure);
            assert!(tls_ca_cert.is_none());
            assert!(tls_pin_sha256.is_none());
            assert!(!tls_pin_now);
            assert!(!tls_pin_clear);
        }
        _ => panic!("expected SetServer"),
    }
}

#[test]
fn parse_config_set_server_requires_url() {
    assert_eq!(
        parse_error_kind(&["bzr", "config", "set-server", "prod"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_config_set_server_requires_name() {
    assert_eq!(
        parse_error_kind(&["bzr", "config", "set-server", "--url", "https://bz"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_config_set_server_binds_credentials() {
    match config_action(&[
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz",
        "--api-key-env",
        "BZR_API_KEY",
        "--email",
        "me@example.com",
    ]) {
        ConfigAction::SetServer {
            api_key_env, email, ..
        } => {
            assert_eq!(api_key_env.as_deref(), Some("BZR_API_KEY"));
            assert_eq!(email.as_deref(), Some("me@example.com"));
        }
        _ => panic!("expected SetServer"),
    }
}

#[test]
fn parse_config_set_server_rejects_api_key_with_api_key_env() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz",
            "--api-key",
            "secret",
            "--api-key-env",
            "BZR_API_KEY",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_config_set_server_auth_method_header() {
    match config_action(&[
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz",
        "--auth-method",
        "header",
    ]) {
        ConfigAction::SetServer { auth_method, .. } => {
            assert_eq!(auth_method, Some(AuthMethod::Header));
        }
        _ => panic!("expected SetServer"),
    }
}

#[test]
fn parse_config_set_server_auth_method_query_param() {
    match config_action(&[
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz",
        "--auth-method",
        "query-param",
    ]) {
        ConfigAction::SetServer { auth_method, .. } => {
            assert_eq!(auth_method, Some(AuthMethod::QueryParam));
        }
        _ => panic!("expected SetServer"),
    }
}

#[test]
fn parse_config_set_server_rejects_invalid_auth_method() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz",
            "--auth-method",
            "basic",
        ]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn parse_config_set_server_tls_pin_now_alone() {
    match config_action(&[
        "bzr",
        "config",
        "set-server",
        "prod",
        "--url",
        "https://bz",
        "--tls-pin-now",
    ]) {
        ConfigAction::SetServer { tls_pin_now, .. } => assert!(tls_pin_now),
        _ => panic!("expected SetServer"),
    }
}

#[test]
fn parse_config_set_server_rejects_tls_insecure_with_ca_cert() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz",
            "--tls-insecure",
            "--tls-ca-cert",
            "/etc/ca.pem",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_config_set_server_rejects_tls_insecure_with_pin_now() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz",
            "--tls-insecure",
            "--tls-pin-now",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_config_set_server_rejects_pin_sha256_with_pin_now() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz",
            "--tls-pin-sha256",
            "sha256//abc",
            "--tls-pin-now",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_config_set_server_rejects_pin_clear_with_pin_now() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz",
            "--tls-pin-clear",
            "--tls-pin-now",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_config_set_default_binds_name() {
    match config_action(&["bzr", "config", "set-default", "prod"]) {
        ConfigAction::SetDefault { name } => assert_eq!(name, "prod"),
        _ => panic!("expected SetDefault"),
    }
}

#[test]
fn parse_config_show() {
    assert!(matches!(
        config_action(&["bzr", "config", "show"]),
        ConfigAction::Show
    ));
}

#[test]
fn parse_config_set_keyring_with_service_and_account() {
    match config_action(&[
        "bzr",
        "config",
        "set-keyring",
        "shared",
        "--service",
        "bzr-team",
        "--account",
        "ci",
    ]) {
        ConfigAction::SetKeyring {
            name,
            service,
            account,
        } => {
            assert_eq!(name, "shared");
            assert_eq!(service.as_deref(), Some("bzr-team"));
            assert_eq!(account.as_deref(), Some("ci"));
        }
        _ => panic!("expected SetKeyring"),
    }
}

#[test]
fn parse_config_set_keyring_defaults_service_account_none() {
    match config_action(&["bzr", "config", "set-keyring", "prod"]) {
        ConfigAction::SetKeyring {
            service, account, ..
        } => {
            assert!(service.is_none());
            assert!(account.is_none());
        }
        _ => panic!("expected SetKeyring"),
    }
}

#[test]
fn parse_config_unset_keyring_binds_name() {
    match config_action(&["bzr", "config", "unset-keyring", "prod"]) {
        ConfigAction::UnsetKeyring { name } => assert_eq!(name, "prod"),
        _ => panic!("expected UnsetKeyring"),
    }
}

#[test]
fn parse_config_migrate_to_keyring_requires_yes_flag_mapping() {
    match config_action(&["bzr", "config", "migrate-to-keyring", "prod", "--yes"]) {
        ConfigAction::MigrateToKeyring { name, yes, .. } => {
            assert_eq!(name, "prod");
            assert!(yes);
        }
        _ => panic!("expected MigrateToKeyring"),
    }
}

#[test]
fn parse_config_migrate_to_keyring_defaults_yes_false() {
    match config_action(&["bzr", "config", "migrate-to-keyring", "prod"]) {
        ConfigAction::MigrateToKeyring { yes, .. } => assert!(!yes),
        _ => panic!("expected MigrateToKeyring"),
    }
}

#[test]
fn parse_config_remove_server_binds_name() {
    match config_action(&["bzr", "config", "remove-server", "staging"]) {
        ConfigAction::RemoveServer { name } => assert_eq!(name, "staging"),
        _ => panic!("expected RemoveServer"),
    }
}

#[test]
fn parse_config_rename_server_binds_old_and_new() {
    match config_action(&["bzr", "config", "rename-server", "stage", "staging"]) {
        ConfigAction::RenameServer { old, new } => {
            assert_eq!(old, "stage");
            assert_eq!(new, "staging");
        }
        _ => panic!("expected RenameServer"),
    }
}

#[test]
fn parse_config_rename_server_requires_two_positionals() {
    assert_eq!(
        parse_error_kind(&["bzr", "config", "rename-server", "stage"]),
        ErrorKind::MissingRequiredArgument
    );
}
