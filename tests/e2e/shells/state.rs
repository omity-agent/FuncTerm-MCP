use crate::support::{parse_command_result, send_command};
pub(super) fn assert_persistent_state(tab: &str, shell: &str) {
    let definition = parse_command_result(&send_command(tab, definition_command(shell), 10.0));
    assert!(definition.finished, "{shell}: {definition:?}");
    assert_eq!(definition.exit_code, Some(0_i32), "{shell}: {definition:?}");
    if shell == "nu" {
        let record = crate::support::command_directory(tab, "probe");
        let declarations = record
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("state/nushell-declarations.nu");
        let archived = fs_err::read_to_string(declarations).unwrap();
        assert!(archived.contains("def functerm-state"));
        assert!(archived.contains("alias functerm-alias"));
        assert!(!archived.contains("def exit "), "{archived}");
    }
    let query = parse_command_result(&send_command(tab, query_command(shell), 10.0));
    assert_eq!(query.exit_code, Some(0_i32), "{shell}: {query:?}");
    for marker in expected_markers(shell) {
        assert!(
            query.stdout.contains(&format!("MCP_PTY_STATE_{marker}")),
            "{shell}: {query:?}"
        );
    }
    assert!(
        query.stderr.contains("MCP_PTY_STATE_STDERR"),
        "{shell}: {query:?}"
    );
    assert!(
        !query.stdout.contains("MCP_PTY_STATE_STDERR"),
        "{shell}: {query:?}"
    );
}
fn definition_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => {
            "$FuncTermStateVariable = 'MCP_PTY_STATE_VARIABLE'; $env:FUNCTERM_STATE_ENV = 'MCP_PTY_STATE_ENV'; function Get-FuncTermState { 'MCP_PTY_STATE_FUNCTION_OLD' }; function Get-FuncTermState { 'MCP_PTY_STATE_FUNCTION' }; Set-Alias functerm-state Get-Date; Set-Alias functerm-state Get-FuncTermState"
        }
        "bash" => {
            "FuncTermStateVariable=MCP_PTY_STATE_VARIABLE; export FUNCTERM_STATE_ENV=MCP_PTY_STATE_ENV; functerm_state() { printf 'MCP_PTY_STATE_FUNCTION\\n'; }; alias functerm-state=functerm_state; shopt -s nullglob"
        }
        "zsh" => {
            "FuncTermStateVariable=MCP_PTY_STATE_VARIABLE; export FUNCTERM_STATE_ENV=MCP_PTY_STATE_ENV; functerm_state() { printf 'MCP_PTY_STATE_FUNCTION\\n'; }; alias functerm-state=functerm_state; setopt null_glob"
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
        "powershell" => &["VARIABLE", "ENV", "FUNCTION"],
        "bash" | "zsh" => &["VARIABLE", "ENV", "FUNCTION", "OPTION"],
        "nu" => &["ENV", "FUNCTION", "ALIAS"],
        "cmd" => &["ENV", "ALIAS"],
        other => panic!("unsupported shell case {other}"),
    }
}
