use super::matrix::{case_dir, required_executable, shell_cases};
use crate::support::{create_tab, locked_with_env, parse_command_result, send_command};
#[test]
fn cli_preserves_shell_state_between_commands() {
    for case in shell_cases() {
        let executable = required_executable(case);
        let _guard = locked_with_env(&[(case.env_var, &executable)]);
        let created = create_tab(&case_dir(case.name, "persistent state"), case.name);
        let definition = parse_command_result(&send_command(
            &created.tab_id,
            definition_command(case.name),
            10.0,
        ));
        assert!(
            definition.finished && definition.exit_code == Some(0_i32),
            "{} state definition failed: stdout: {}\nstderr: {}",
            case.name,
            definition.stdout,
            definition.stderr
        );
        let query = parse_command_result(&send_command(
            &created.tab_id,
            query_command(case.name),
            10.0,
        ));
        assert_eq!(
            query.exit_code,
            Some(0_i32),
            "{} state query failed: stdout: {}\nstderr: {}",
            case.name,
            query.stdout,
            query.stderr
        );
        for marker in expected_markers(case.name) {
            assert!(
                query.stdout.contains(marker),
                "{} did not preserve {marker}: stdout: {}\nstderr: {}",
                case.name,
                query.stdout,
                query.stderr
            );
        }
        assert!(
            query.stderr.contains("MCP_PTY_STATE_STDERR"),
            "{} stderr was not captured separately: {}",
            case.name,
            query.stderr
        );
        assert!(
            !query.stdout.contains("MCP_PTY_STATE_STDERR"),
            "{} stderr leaked into stdout: {}",
            case.name,
            query.stdout
        );
    }
}
fn definition_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => {
            "$FuncTermStateVariable = 'MCP_PTY_STATE_VARIABLE'; $env:FUNCTERM_STATE_ENV = 'MCP_PTY_STATE_ENV'; function Get-FuncTermState { 'MCP_PTY_STATE_FUNCTION_OLD' }; function Get-FuncTermState { 'MCP_PTY_STATE_FUNCTION' }; Set-Alias functerm-state Get-Date; Set-Alias functerm-state Get-FuncTermState"
        }
        "bash" | "zsh" => {
            if shell == "bash" {
                "FuncTermStateVariable=MCP_PTY_STATE_VARIABLE; export FUNCTERM_STATE_ENV=MCP_PTY_STATE_ENV; functerm_state() { printf 'MCP_PTY_STATE_FUNCTION\\n'; }; alias functerm-state=functerm_state; shopt -s nullglob"
            } else {
                "FuncTermStateVariable=MCP_PTY_STATE_VARIABLE; export FUNCTERM_STATE_ENV=MCP_PTY_STATE_ENV; functerm_state() { printf 'MCP_PTY_STATE_FUNCTION\\n'; }; alias functerm-state=functerm_state; setopt null_glob"
            }
        }
        "nu" => {
            "$env.FUNCTERM_STATE_ENV = 'MCP_PTY_STATE_ENV'; def functerm-state [] { 'MCP_PTY_STATE_FUNCTION' }; alias functerm-alias = print MCP_PTY_STATE_ALIAS"
        }
        "cmd" => {
            "set FUNCTERM_STATE_ENV=MCP_PTY_STATE_ENV& doskey functerm-state=echo MCP_PTY_STATE_ALIAS"
        }
        other => panic!("unsupported shell case {other}"),
    }
}
fn query_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => {
            "$FuncTermStateVariable; $env:FUNCTERM_STATE_ENV; functerm-state; Write-Error 'MCP_PTY_STATE_STDERR'"
        }
        "bash" => {
            "printf '%s\\n' \"$FuncTermStateVariable\" \"$FUNCTERM_STATE_ENV\"; functerm-state; shopt -q nullglob && printf 'MCP_PTY_STATE_OPTION\\n'; printf 'MCP_PTY_STATE_STDERR\\n' >&2"
        }
        "zsh" => {
            "printf '%s\\n' \"$FuncTermStateVariable\" \"$FUNCTERM_STATE_ENV\"; functerm-state; [[ -o null_glob ]] && printf 'MCP_PTY_STATE_OPTION\\n'; printf 'MCP_PTY_STATE_STDERR\\n' >&2"
        }
        "nu" => {
            "print $env.FUNCTERM_STATE_ENV; print (functerm-state); functerm-alias; print --stderr MCP_PTY_STATE_STDERR"
        }
        "cmd" => "echo %FUNCTERM_STATE_ENV%& doskey /macros& echo MCP_PTY_STATE_STDERR 1>&2",
        other => panic!("unsupported shell case {other}"),
    }
}
fn expected_markers(shell: &str) -> &'static [&'static str] {
    match shell {
        "powershell" => &[
            "MCP_PTY_STATE_VARIABLE",
            "MCP_PTY_STATE_ENV",
            "MCP_PTY_STATE_FUNCTION",
        ],
        "bash" | "zsh" => &[
            "MCP_PTY_STATE_VARIABLE",
            "MCP_PTY_STATE_ENV",
            "MCP_PTY_STATE_FUNCTION",
            "MCP_PTY_STATE_OPTION",
        ],
        "nu" => &[
            "MCP_PTY_STATE_ENV",
            "MCP_PTY_STATE_FUNCTION",
            "MCP_PTY_STATE_ALIAS",
        ],
        "cmd" => &["MCP_PTY_STATE_ENV", "MCP_PTY_STATE_ALIAS"],
        other => panic!("unsupported shell case {other}"),
    }
}
