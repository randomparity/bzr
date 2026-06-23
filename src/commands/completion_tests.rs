use clap_complete::Shell;

use crate::test_helpers::CapturedIo;
use crate::types::OutputFormat;

async fn script_for(shell: Shell) -> String {
    let mut io = CapturedIo::new();
    let result = super::execute(shell, None, OutputFormat::Table, None, &mut io.writers()).await;
    assert!(result.is_ok(), "completion generation should succeed");
    io.out_str().to_string()
}

#[tokio::test]
async fn bash_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::Bash).await;
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

#[tokio::test]
async fn zsh_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::Zsh).await;
    assert!(!script.is_empty(), "zsh script should not be empty");
    assert!(script.contains("bzr"), "zsh script should reference bzr");
    assert!(
        script.contains("#compdef bzr"),
        "zsh script should carry a #compdef directive"
    );
}

#[tokio::test]
async fn fish_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::Fish).await;
    assert!(!script.is_empty(), "fish script should not be empty");
    assert!(
        script.contains("bzr"),
        "fish script should reference the bzr binary name"
    );
}

#[tokio::test]
async fn powershell_script_is_nonempty_and_names_binary() {
    let script = script_for(Shell::PowerShell).await;
    assert!(!script.is_empty(), "powershell script should not be empty");
    assert!(
        script.contains("bzr"),
        "powershell script should reference the bzr binary name"
    );
}
