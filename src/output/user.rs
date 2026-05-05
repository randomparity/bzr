use std::io::{self, Write as _};

use colored::Colorize;
use tabled::{Table, Tabled};

use super::formatting::{opt_yes_no, print_field, print_formatted, print_optional_field};
use crate::types::{BugzillaUser, OutputFormat, WhoamiResponse};

#[derive(Tabled)]
struct UserRow {
    #[tabled(rename = "ID")]
    id: u64,
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "REAL NAME")]
    real_name: String,
    #[tabled(rename = "EMAIL")]
    email: String,
}

#[derive(Tabled)]
struct DetailedUserRow {
    #[tabled(rename = "ID")]
    id: u64,
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "REAL NAME")]
    real_name: String,
    #[tabled(rename = "EMAIL")]
    email: String,
    #[tabled(rename = "CAN LOGIN")]
    can_login: String,
    #[tabled(rename = "GROUPS")]
    groups: String,
}

fn basic_row(user: &BugzillaUser) -> UserRow {
    UserRow {
        id: user.id,
        name: user.name.clone(),
        real_name: user.real_name.clone().unwrap_or_default(),
        email: user.email.clone().unwrap_or_default(),
    }
}

fn detailed_row(user: &BugzillaUser) -> DetailedUserRow {
    DetailedUserRow {
        id: user.id,
        name: user.name.clone(),
        real_name: user.real_name.clone().unwrap_or_default(),
        email: user.email.clone().unwrap_or_default(),
        can_login: opt_yes_no(user.can_login).into(),
        groups: if user.groups.is_empty() {
            "-".into()
        } else {
            user.groups
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        },
    }
}

pub fn print_users(users: &[BugzillaUser], format: OutputFormat) {
    print_formatted(users, format, |users| {
        if users.is_empty() {
            let _ = writeln!(io::stdout(), "No users found.");
            return;
        }
        let rows: Vec<UserRow> = users.iter().map(basic_row).collect();
        let _ = writeln!(io::stdout(), "{}", Table::new(rows));
    });
}

pub fn print_users_detailed(users: &[BugzillaUser], format: OutputFormat) {
    print_formatted(users, format, |users| {
        if users.is_empty() {
            let _ = writeln!(io::stdout(), "No users found.");
            return;
        }
        let rows: Vec<DetailedUserRow> = users.iter().map(detailed_row).collect();
        let _ = writeln!(io::stdout(), "{}", Table::new(rows));
    });
}

pub fn print_whoami(whoami: &WhoamiResponse, format: OutputFormat) {
    print_formatted(whoami, format, |whoami| {
        let _ = writeln!(io::stdout(), "{} {}", "User".bold(), whoami.name.bold());
        print_optional_field("Name", whoami.real_name.as_deref());
        print_optional_field("Login", whoami.login.as_deref());
        print_field("ID", &whoami.id.to_string());
    });
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
