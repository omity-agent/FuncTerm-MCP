use crate::support::{
    TestGuard, locked_with_env, parse_command_result, required_executable, run_cli, send_command,
    temp_dir,
};
use functerm::shell::quote;
use std::path::PathBuf;
pub(super) fn start_runtime(label: &str) -> (TestGuard, PathBuf, &'static str) {
    let python = required_executable(&["python3", "python"]);
    let bash = required_executable(&["bash"]);
    let mut environment = vec![("FUNCTERM_PYTHON", python.to_str().unwrap())];
    environment.push(("FUNCTERM_BASH", bash.to_str().unwrap()));
    #[cfg(windows)]
    let powershell = required_executable(&["pwsh", "powershell"]);
    #[cfg(windows)]
    environment.push(("FUNCTERM_POWERSHELL", powershell.to_str().unwrap()));
    let guard = locked_with_env(&environment);
    (guard, temp_dir(label), parent_shell())
}
#[cfg(windows)]
const fn parent_shell() -> &'static str {
    "powershell"
}
#[cfg(not(windows))]
const fn parent_shell() -> &'static str {
    "bash"
}
pub(super) fn pipe_command(parent: &str, source: &str, target: &str) -> String {
    match parent {
        "powershell" => format!("@'\n{source}\n'@ | {target}"),
        "bash" => format!("printf '%s' {} | {target}", quote::posix_string(source)),
        other => panic!("unsupported pipeline parent {other}"),
    }
}
pub(super) const fn parent_marker(parent: &str) -> &'static str {
    match parent.as_bytes() {
        b"powershell" => "Write-Output -InputObject 'PARENT_CONTINUED'",
        b"bash" => "printf '%s\\n' 'PARENT_CONTINUED'",
        b"cmd" => "echo PARENT_CONTINUED",
        _ => panic!("unsupported parent shell"),
    }
}
pub(super) fn assert_parent(tab_id: &str, parent: &str) {
    let expected = match parent {
        "powershell" => "PowerShell",
        "bash" => "Bash",
        "cmd" => "Windows CMD",
        other => panic!("unsupported parent {other}"),
    };
    let viewed = run_cli(&["view", tab_id]);
    assert!(viewed.status.success(), "view failed: {viewed:?}");
    let text = String::from_utf8(viewed.stdout).unwrap();
    assert!(
        text.contains(&format!("<TYPE>\n{expected}\n</TYPE>")),
        "parent shell changed: {text}"
    );
    let result = parse_command_result(&send_command(tab_id, parent_marker(parent), 10.0));
    assert!(result.finished, "parent marker should finish");
    assert_eq!(result.exit_code, Some(0_i32), "stderr: {}", result.stderr);
    assert!(
        result.stdout.contains("PARENT_CONTINUED"),
        "stdout: {}",
        result.stdout
    );
}
