use super::matrix::{ShellCase, case_dir, required_executable, shell_cases};
use crate::support::{create_tab, locked_with_env, parse_command_result, send_command};
const REPL_CASES: [ShellCase; 2] = [
    ShellCase {
        name: "python",
        env_var: "FUNCTERM_PYTHON",
        executables: &["python3", "python", "python3.exe", "python.exe"],
        expected_exit_code: 0,
    },
    ShellCase {
        name: "bun",
        env_var: "FUNCTERM_BUN",
        executables: &["bun", "bun.exe"],
        expected_exit_code: 0,
    },
];
#[test]
fn cli_recovers_after_shell_environment_is_cleared() {
    for case in shell_cases().iter().chain(&REPL_CASES) {
        assert_recovers(case);
    }
}
fn assert_recovers(case: &ShellCase) {
    let executable = required_executable(case);
    let _guard = locked_with_env(&[(case.env_var, &executable)]);
    let directory = case_dir(case.name, "cleared environment");
    let created = create_tab(&directory, case.name);
    let cleared = parse_command_result(&send_command(
        &created.tab_id,
        clear_environment_command(case.name),
        10.0,
    ));
    assert!(
        cleared.finished
            && cleared.exit_code == Some(0_i32)
            && cleared.stdout.contains("MCP_PTY_ENVIRONMENT_CLEARED"),
        "{} did not finish after clearing its environment: stdout: {}\nstderr: {}",
        case.name,
        cleared.stdout,
        cleared.stderr
    );
    let next = parse_command_result(&send_command(
        &created.tab_id,
        plain_marker_command(case.name),
        10.0,
    ));
    assert!(
        next.finished
            && next.exit_code == Some(0_i32)
            && next.stdout.contains("MCP_PTY_ENVIRONMENT_RECOVERED"),
        "{} did not recover after clearing its environment: stdout: {}\nstderr: {}",
        case.name,
        next.stdout,
        next.stderr
    );
    assert!(!directory.join("state").join("done.json").exists());
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
