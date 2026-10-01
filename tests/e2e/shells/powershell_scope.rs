use super::matrix::case_dir;
use crate::support::{
    CommandResult, create_tab, locked_with_env, parse_command_result, required_executable,
    send_command,
};
use functerm::shell::quote;
use std::fs;
#[test]
fn powershell_preserves_native_session_scope() {
    let executable = required_executable(&["pwsh", "pwsh.exe"]);
    let _guard = locked_with_env(&[("FUNCTERM_POWERSHELL", executable.to_str().unwrap())]);
    let tab = create_tab(&case_dir("powershell", "native scope"), "powershell");
    assert_success(
        &tab.tab_id,
        "
$FuncTermScopeValue = 'initial'
$local:FuncTermScopeLocal = 'local'
$script:FuncTermScopeScript = 'script'
$FuncTermScopeRemoved = 'remove-me'
[int]$FuncTermScopeTyped = 41
function Get-FuncTermScopeRemoved { 'remove-me' }
Set-Alias FuncTermScopeRemovedAlias Get-Date
New-Variable FuncTermScopeReadOnly -Value 'read-only' -Option ReadOnly
New-Variable FuncTermScopeConstant -Value 'constant' -Option Constant
",
    );
    assert_success(
        &tab.tab_id,
        "
$FuncTermScopeValue = 'updated'
$FuncTermScopeTyped = '42'
Remove-Variable FuncTermScopeRemoved
Remove-Item Function:Get-FuncTermScopeRemoved
Remove-Item Alias:FuncTermScopeRemovedAlias
",
    );
    assert_success (& tab . tab_id , "
if ($FuncTermScopeValue -ne 'updated') { throw 'updated variable was lost' }
if ($FuncTermScopeLocal -ne 'local') { throw 'explicit local variable was lost' }
if ($FuncTermScopeScript -ne 'script') { throw 'explicit script variable was lost' }
if ($FuncTermScopeTyped -ne 42 -or $FuncTermScopeTyped -isnot [int]) { throw 'variable type was lost' }
if ($FuncTermScopeReadOnly -ne 'read-only' -or (Get-Variable FuncTermScopeReadOnly).Options -ne 'ReadOnly') { throw 'read-only variable was lost' }
if ($FuncTermScopeConstant -ne 'constant' -or (Get-Variable FuncTermScopeConstant).Options -ne 'Constant') { throw 'constant variable was lost' }
if (Test-Path Variable:FuncTermScopeRemoved) { throw 'removed variable survived' }
if (Test-Path Function:Get-FuncTermScopeRemoved) { throw 'removed function survived' }
if (Test-Path Alias:FuncTermScopeRemovedAlias) { throw 'removed alias survived' }
" ,) ;
}
#[test]
fn powershell_dot_sources_scripts_in_the_callers_scope() {
    let executable = required_executable(&["pwsh", "pwsh.exe"]);
    let _guard = locked_with_env(&[("FUNCTERM_POWERSHELL", executable.to_str().unwrap())]);
    let directory = case_dir("powershell", "dot source scope");
    let script = directory.join("scope.ps1");
    fs::write(
        &script,
        "
$FuncTermScopeValue = 'script-value'
$FuncTermScopeOnlyInScript = 'script-only'
function Get-FuncTermScopeScript { 'script-function' }
Set-Alias FuncTermScopeScriptAlias Get-FuncTermScopeScript
",
    )
    .unwrap();
    let path = quote::powershell_path(&script).unwrap();
    let tab = create_tab(&directory, "powershell");
    assert_success (& tab . tab_id , & format ! ("
$FuncTermScopeValue = 'outer-value'
function Invoke-FuncTermScopeScript {{
    . {path}
    if ($FuncTermScopeValue -ne 'script-value') {{ throw 'script did not modify the local variable' }}
    if ($FuncTermScopeOnlyInScript -ne 'script-only') {{ throw 'script local variable was lost' }}
    if ((FuncTermScopeScriptAlias) -ne 'script-function') {{ throw 'script local alias was lost' }}
}}
Invoke-FuncTermScopeScript
") ,) ;
    assert_success(
        &tab.tab_id,
        "
if ($FuncTermScopeValue -ne 'outer-value') { throw 'function local variable leaked' }
if (Test-Path Variable:FuncTermScopeOnlyInScript) { throw 'script local variable leaked' }
if (Test-Path Function:Get-FuncTermScopeScript) { throw 'script local function leaked' }
if (Test-Path Alias:FuncTermScopeScriptAlias) { throw 'script local alias leaked' }
",
    );
    assert_success(&tab.tab_id, &format!(". {path}"));
    assert_success(
        &tab.tab_id,
        "
if ($FuncTermScopeValue -ne 'script-value') { throw 'top-level script variable was lost' }
if ($FuncTermScopeOnlyInScript -ne 'script-only') { throw 'top-level script definition was lost' }
if ((FuncTermScopeScriptAlias) -ne 'script-function') { throw 'top-level script alias was lost' }
",
    );
}
#[test]
fn powershell_cleans_command_state_before_returning_to_the_prompt() {
    let executable = required_executable(&["pwsh", "pwsh.exe"]);
    let _guard = locked_with_env(&[("FUNCTERM_POWERSHELL", executable.to_str().unwrap())]);
    let directory = case_dir("powershell", "command cleanup");
    let report = directory.join("prompt-variables.txt");
    let path = quote::powershell_path(&report).unwrap();
    let tab = create_tab(&directory, "powershell");
    assert_success (& tab . tab_id , & format ! ("
function prompt {{
    & {{
        $variables = (Get-Variable -Scope Global).Name | Where-Object {{
            $_ -match '_[a-z0-9]{{12}}$' -and $_ -notmatch '^(readyWritten|originalPrompt)_'
        }}
        [IO.File]::WriteAllText({path}, [string]::Join([Environment]::NewLine, [string[]]@($variables)))
    }}
    'PS> '
}}
") ,) ;
    for (command, exit_code) in [
        ("Write-Output 'normal completion'", 0_i32),
        ("return 'early return'", 0_i32),
        ("throw 'scope cleanup failure'", 1_i32),
        ("Write-Output \"unterminated", 1_i32),
    ] {
        let result = parse_command_result(&send_command(&tab.tab_id, command, 10.0));
        assert!(result.finished, "{}", result.stderr);
        assert_eq!(result.exit_code, Some(exit_code), "{}", result.stderr);
        let query = assert_success(
            &tab.tab_id,
            &format!("[IO.File]::ReadAllText({path}); Write-Output 'FUNCTERM_CLEAN_PROMPT'"),
        );
        assert_eq!(query.stdout.trim(), "FUNCTERM_CLEAN_PROMPT");
    }
}
fn assert_success(tab_id: &str, command: &str) -> CommandResult {
    let result = parse_command_result(&send_command(tab_id, command, 10.0));
    assert!(result.finished, "{}", result.stderr);
    assert_eq!(
        result.exit_code,
        Some(0_i32),
        "stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
    assert!(result.stderr.is_empty(), "{}", result.stderr);
    result
}
