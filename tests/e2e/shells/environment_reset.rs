use super::matrix::{case_dir, shell_cases};
use crate::support::{create_tab, parse_command_result, send_command};
#[test]
fn cli_recovers_after_shell_environment_is_cleared() {
    let _guard = super::matrix::daemon();
    for shell in shell_cases()
        .iter()
        .map(|case| case.name)
        .chain(["python", "bun"])
    {
        assert_recovers(shell);
    }
}
fn assert_recovers(shell: &str) {
    let directory = case_dir(shell, "cleared environment");
    let created = create_tab(&directory, shell);
    let cleared = parse_command_result(&send_command(
        &created.tab_id,
        clear_environment_command(shell),
        10.0,
    ));
    assert!(cleared.finished, "{shell}: {cleared:?}");
    assert_eq!(cleared.exit_code, Some(0_i32), "{shell}: {cleared:?}");
    assert!(
        cleared.stdout.contains("MCP_PTY_ENVIRONMENT_CLEARED"),
        "{cleared:?}"
    );
    let next = parse_command_result(&send_command(
        &created.tab_id,
        plain_marker_command(shell),
        10.0,
    ));
    assert!(next.finished, "{shell}: {next:?}");
    assert_eq!(next.exit_code, Some(0_i32), "{shell}: {next:?}");
    assert!(
        next.stdout.contains("MCP_PTY_ENVIRONMENT_RECOVERED"),
        "{next:?}"
    );
    assert!(!directory.join("done.json").exists());
}
fn clear_environment_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => {
            "Get-ChildItem Env: | Remove-Item; Write-Output MCP_PTY_ENVIRONMENT_CLEARED"
        }
        "bash" | "zsh" => {
            "for name in $(env | sed 's/=.*//'); do unset \"$name\"; done; printf 'MCP_PTY_ENVIRONMENT_CLEARED\\n'"
        }
        "nu" => {
            "for name in ($env | columns | where $it != PWD) { hide-env $name }; print MCP_PTY_ENVIRONMENT_CLEARED"
        }
        "cmd" => {
            "for /f \"delims==\" %%A in ('set') do set \"%%A=\"& echo MCP_PTY_ENVIRONMENT_CLEARED"
        }
        "python" => "__import__('os').environ.clear(); print('MCP_PTY_ENVIRONMENT_CLEARED')",
        "bun" => {
            "for (const name of Object.keys(process.env)) delete process.env[name]; console.log('MCP_PTY_ENVIRONMENT_CLEARED')"
        }
        other => panic!("unsupported shell case {other}"),
    }
}
fn plain_marker_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => "Write-Output MCP_PTY_ENVIRONMENT_RECOVERED",
        "bash" | "zsh" => "printf 'MCP_PTY_ENVIRONMENT_RECOVERED\\n'",
        "nu" => "print MCP_PTY_ENVIRONMENT_RECOVERED",
        "cmd" => "echo MCP_PTY_ENVIRONMENT_RECOVERED",
        "python" => "print('MCP_PTY_ENVIRONMENT_RECOVERED')",
        "bun" => "console.log('MCP_PTY_ENVIRONMENT_RECOVERED')",
        other => panic!("unsupported shell case {other}"),
    }
}
