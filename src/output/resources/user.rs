use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{
    opt_yes_no, write_field, write_formatted, write_formatted_projected, write_optional_field,
    write_records_or_empty, TableSpec,
};
use crate::types::output::OutputFormat;
use crate::types::user::{BugzillaUser, WhoamiOutput};
use crate::validation::fields::FieldProjection;

const USER_HEADERS: &[&str] = &["ID", "NAME", "REAL NAME", "EMAIL"];
const DETAILED_USER_HEADERS: &[&str] = &["ID", "NAME", "REAL NAME", "EMAIL", "CAN LOGIN", "GROUPS"];

struct UserRow {
    id: u64,
    name: String,
    real_name: String,
    email: String,
}

struct DetailedUserRow {
    id: u64,
    name: String,
    real_name: String,
    email: String,
    can_login: String,
    groups: String,
}

fn basic_row(user: &BugzillaUser) -> UserRow {
    UserRow {
        id: user.id,
        name: user.name.clone().unwrap_or_default(),
        real_name: user.real_name.clone().unwrap_or_default(),
        email: user.email.clone().unwrap_or_default(),
    }
}

fn detailed_row(user: &BugzillaUser) -> DetailedUserRow {
    DetailedUserRow {
        id: user.id,
        name: user.name.clone().unwrap_or_default(),
        real_name: user.real_name.clone().unwrap_or_default(),
        email: user.email.clone().unwrap_or_default(),
        can_login: opt_yes_no(user.can_login).into(),
        groups: if user.groups.is_empty() {
            "-".into()
        } else {
            user.groups
                .iter()
                .map(|g| g.name.as_deref().unwrap_or("unknown"))
                .collect::<Vec<_>>()
                .join(", ")
        },
    }
}

fn basic_record(user: &BugzillaUser) -> Vec<String> {
    let row = basic_row(user);
    vec![row.id.to_string(), row.name, row.real_name, row.email]
}

fn detailed_record(user: &BugzillaUser) -> Vec<String> {
    let row = detailed_row(user);
    vec![
        row.id.to_string(),
        row.name,
        row.real_name,
        row.email,
        row.can_login,
        row.groups,
    ]
}

pub fn write_users<W: Write + ?Sized>(
    users: &[BugzillaUser],
    format: OutputFormat,
    projection: &FieldProjection,
    out: &mut W,
) {
    write_formatted_projected(users, format, projection, out, |users, out| {
        write_records_or_empty(
            users,
            TableSpec {
                empty_msg: "No users found.",
                headers: USER_HEADERS,
            },
            basic_record,
            out,
        );
    });
}

pub fn write_users_detailed<W: Write + ?Sized>(
    users: &[BugzillaUser],
    format: OutputFormat,
    projection: &FieldProjection,
    out: &mut W,
) {
    write_formatted_projected(users, format, projection, out, |users, out| {
        write_records_or_empty(
            users,
            TableSpec {
                empty_msg: "No users found.",
                headers: DETAILED_USER_HEADERS,
            },
            detailed_record,
            out,
        );
    });
}

pub fn write_whoami<W: Write + ?Sized>(whoami: &WhoamiOutput, format: OutputFormat, out: &mut W) {
    write_formatted(whoami, format, out, |whoami, out| {
        let _ = writeln!(
            out,
            "{} {}",
            "User".bold(),
            whoami.identity.name.as_deref().unwrap_or("unknown").bold()
        );
        write_optional_field(out, "Name", whoami.identity.real_name.as_deref());
        write_optional_field(out, "Login", whoami.identity.login.as_deref());
        write_field(out, "ID", &whoami.identity.id.to_string());
        write_field(out, "Server", &whoami.server_name);
        write_field(out, "Auth", &whoami.auth_mode.to_string());
    });
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
