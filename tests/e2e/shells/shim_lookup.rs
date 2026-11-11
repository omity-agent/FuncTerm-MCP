use super::matrix::{case_dir, shell_cases};
use crate::support::{complete_command, create_tab};
#[test]
fn cli_is_available_with_only_shim_path() {
    let _guard = super::matrix::daemon();
    for shell in shell_cases()
        .iter()
        .map(|case| case.name)
        .chain(["python", "bun"])
    {
        assert_cli_lookup(shell);
    }
}
fn assert_cli_lookup(shell: &str) {
    let directory = case_dir(shell, "shim lookup");
    let created = create_tab(&directory, shell);
    let initial = complete_command(&created.tab_id, &executable_probe_command(shell, false));
    assert_eq!(initial.exit_code, Some(0_i32), "{shell}: {initial:?}");
    assert!(initial.stdout.contains("Usage:"), "{shell}: {initial:?}");
    let next = complete_command(&created.tab_id, &executable_probe_command(shell, true));
    assert_eq!(next.exit_code, Some(0_i32), "{shell}: {next:?}");
    assert!(next.stdout.contains("Usage:"), "{next:?}");
    assert!(!directory.join("done.json").exists());
}
fn executable_probe_command(shell: &str, isolated: bool) -> String {
    let (prefix, invocation) = match shell {
        "powershell" => ("$env:PATH = $env:FUNCTERM_SHIM_DIR; ", "functerm --help"),
        "bash" | "zsh" => ("export PATH=\"${PATH%%:*}\"; ", "functerm --help"),
        "nu" => ("$env.PATH = [$env.FUNCTERM_SHIM_DIR]; ", "functerm --help"),
        "cmd" => ("set \"PATH=%FUNCTERM_SHIM_DIR%\"& ", "functerm --help"),
        "python" => (
            "__import__('os').environ['PATH'] = __import__('os').environ['FUNCTERM_SHIM_DIR']; ",
            "__import__('subprocess').run(['functerm', '--help'], check=True)",
        ),
        "bun" => (
            "process.env.PATH = process.env.FUNCTERM_SHIM_DIR; ",
            "(await import('node:child_process')).execFileSync('functerm', ['--help'], {stdio: 'inherit'})",
        ),
        other => panic!("unsupported shell case {other}"),
    };
    format!("{}{invocation}", if isolated { prefix } else { "" })
}
