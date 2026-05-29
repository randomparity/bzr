use std::io::Write;

use colored::Colorize;
use tabled::Tabled;

use crate::output::formatting::{
    opt_yes_no, write_field, write_formatted, write_optional_field, write_table_or_empty,
};
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

pub fn write_users<W: Write + ?Sized>(users: &[BugzillaUser], format: OutputFormat, out: &mut W) {
    write_table_or_empty(users, format, out, "No users found.", basic_row);
}

pub fn write_users_detailed<W: Write + ?Sized>(
    users: &[BugzillaUser],
    format: OutputFormat,
    out: &mut W,
) {
    write_table_or_empty(users, format, out, "No users found.", detailed_row);
}

pub fn write_whoami<W: Write + ?Sized>(whoami: &WhoamiResponse, format: OutputFormat, out: &mut W) {
    write_formatted(whoami, format, out, |whoami, out| {
        let _ = writeln!(out, "{} {}", "User".bold(), whoami.name.bold());
        write_optional_field(out, "Name", whoami.real_name.as_deref());
        write_optional_field(out, "Login", whoami.login.as_deref());
        write_field(out, "ID", &whoami.id.to_string());
    });
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
