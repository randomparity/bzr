use clap_complete::Shell;

use crate::test_helpers::CapturedIo;

fn script_for(shell: Shell) -> String {
    let mut io = CapturedIo::new();
    let result = super::execute(shell, &mut io.writers());
    assert!(result.is_ok(), "completion generation should succeed");
    io.out_str().to_string()
}

#[test]
fn bash_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::Bash);
    assert!(!script.is_empty(), "bash script should not be empty");
    assert!(
        script.contains("bzr"),
        "bash script should reference the bzr binary name"
    );
    assert!(
        script.contains("complete"),
        "bash script should register a completion function"
    );
}

#[test]
fn zsh_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::Zsh);
    assert!(!script.is_empty(), "zsh script should not be empty");
    assert!(script.contains("bzr"), "zsh script should reference bzr");
    assert!(
        script.contains("#compdef bzr"),
        "zsh script should carry a #compdef directive"
    );
}

#[test]
fn fish_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::Fish);
    assert!(!script.is_empty(), "fish script should not be empty");
    assert!(
        script.contains("bzr"),
        "fish script should reference the bzr binary name"
    );
}

#[test]
fn powershell_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::PowerShell);
    assert!(!script.is_empty(), "powershell script should not be empty");
    assert!(
        script.contains("bzr"),
        "powershell script should reference the bzr binary name"
    );
}
