use std::io::Cursor;

use super::{read_yes_no, should_prompt, BATCH_THRESHOLD};

#[test]
fn should_prompt_only_above_threshold_at_interactive_tty() {
    // At or below threshold: never prompt, regardless of TTY.
    assert!(!should_prompt(BATCH_THRESHOLD, false, true));
    // Above threshold at a TTY without --yes: prompt.
    assert!(should_prompt(BATCH_THRESHOLD + 1, false, true));
    // --yes bypasses.
    assert!(!should_prompt(BATCH_THRESHOLD + 1, true, true));
    // Non-TTY (piped/agent) bypasses.
    assert!(!should_prompt(BATCH_THRESHOLD + 1, false, false));
}

#[test]
fn read_yes_no_accepts_y_and_yes_case_insensitively() {
    for answer in ["y\n", "Y\n", "yes\n", "YES\n", "  yes \n"] {
        let mut reader = Cursor::new(answer.as_bytes().to_vec());
        let mut w = Vec::new();
        assert!(
            read_yes_no(&mut reader, &mut w, 12).unwrap(),
            "{answer:?} should be a yes"
        );
        assert!(String::from_utf8(w)
            .unwrap()
            .contains("About to modify 12 bugs"));
    }
}

#[test]
fn read_yes_no_treats_a_typed_non_yes_as_no() {
    // A line was typed but it isn't yes: decline (safe default), not an error.
    for answer in ["n\n", "no\n", "\n", "maybe\n"] {
        let mut reader = Cursor::new(answer.as_bytes().to_vec());
        let mut w = Vec::new();
        assert!(
            !read_yes_no(&mut reader, &mut w, 5).unwrap(),
            "{answer:?} should be a no (safe default)"
        );
    }
}

#[test]
fn read_yes_no_errors_on_eof_with_yes_hint() {
    // No line at all (stdin already consumed, e.g. by --comment -) is not a
    // silent decline: surface an actionable error naming --yes.
    let mut reader = Cursor::new(Vec::new());
    let mut w = Vec::new();
    let err = read_yes_no(&mut reader, &mut w, 12).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(m) if m.contains("--yes")),
        "EOF should error with a --yes hint, got {err:?}"
    );
}
