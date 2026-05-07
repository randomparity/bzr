use std::io::{self, Write as _};

use colored::Colorize;

use super::formatting::{print_field, print_formatted, print_optional_field};
use crate::types::{Attachment, OutputFormat};

pub fn print_attachments(attachments: &[Attachment], format: OutputFormat) {
    print_formatted(attachments, format, |attachments| {
        if attachments.is_empty() {
            let _ = writeln!(io::stdout(), "No attachments.");
            return;
        }
        for a in attachments {
            let patch = if a.is_patch { " [PATCH]" } else { "" };
            let obsolete = if a.is_obsolete { " [OBSOLETE]" } else { "" };
            let private = if a.is_private { " [PRIVATE]" } else { "" };
            let _ = writeln!(
                io::stdout(),
                "{} #{} - {}{}{}{}",
                "Attachment".bold(),
                a.id,
                a.summary.bold(),
                patch.cyan(),
                obsolete.red(),
                private.red(),
            );
            print_field(
                "File",
                &format!("{} ({}, {} bytes)", a.file_name, a.content_type, a.size),
            );
            print_optional_field("Creator", a.creator.as_deref());
            print_optional_field("Created", a.creation_time.as_deref());
            let _ = writeln!(io::stdout());
        }
    });
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
