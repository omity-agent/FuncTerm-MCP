use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY,
    COMMAND_OUTPUT_DIRECTORY, COMMAND_POWERSHELL_SCRIPT_FILE, COMMAND_SCRIPT_FILE,
    COMMAND_STATE_DIRECTORY, COMMAND_WORKING_DIRECTORY_FILE, DISPATCH_FILE, DONE_FILE,
    HELPER_EXECUTABLE_ENV, POSIX_COMMAND_FUNCTION, POWERSHELL_COMMAND_FUNCTION,
    SESSION_COMMANDS_DIRECTORY, SESSION_STATE_DIRECTORY, STDERR_FILE, STDOUT_FILE,
};
use crate::shell::shims::SESSION_ROOT_ENV;
const COMMON: [(&str, &str); 9] = [
    ("@COMMAND_DIR_ENV@", COMMAND_DIRECTORY_ENV),
    ("@COMMAND_ID_ENV@", COMMAND_ID_ENV),
    ("@INPUT_DIR@", COMMAND_INPUT_DIRECTORY),
    ("@DONE@", DONE_FILE),
    ("@HELPER_ENV@", HELPER_EXECUTABLE_ENV),
    ("@OUTPUT_DIR@", COMMAND_OUTPUT_DIRECTORY),
    ("@STATE_DIR@", COMMAND_STATE_DIRECTORY),
    ("@STDERR@", STDERR_FILE),
    ("@STDOUT@", STDOUT_FILE),
];
pub(super) fn render_command_function(template: &str, function_name: &str) -> String {
    render(
        template,
        &[("@FUNCTION@", function_name), ("@COMMAND@", COMMAND_FILE)],
    )
}
pub(super) fn render_script(template: &str) -> String {
    render(template, &[("@SCRIPT@", COMMAND_SCRIPT_FILE)])
}
pub(super) fn render_powershell(template: &str) -> String {
    render(
        template,
        &[
            ("@COMMAND@", COMMAND_FILE),
            ("@SCRIPT@", COMMAND_POWERSHELL_SCRIPT_FILE),
        ],
    )
}
pub(in crate::shell) fn cmd_dispatcher() -> String {
    let rendered = render(
        CMD_DISPATCHER,
        &[
            ("@DISPATCH@", DISPATCH_FILE),
            ("@SESSION_COMMANDS@", SESSION_COMMANDS_DIRECTORY),
            ("@SESSION_STATE@", SESSION_STATE_DIRECTORY),
            ("@WORKING_DIRECTORY@", COMMAND_WORKING_DIRECTORY_FILE),
        ],
    );
    super::VariableNamespace::new().render(&rendered)
}
pub(super) fn powershell_dispatcher() -> String {
    let session_root_env = SESSION_ROOT_ENV;
    let state_directory = SESSION_STATE_DIRECTORY;
    let dispatch_file = DISPATCH_FILE;
    let commands_directory = SESSION_COMMANDS_DIRECTORY;
    let input_directory = COMMAND_INPUT_DIRECTORY;
    let working_directory_file = COMMAND_WORKING_DIRECTORY_FILE;
    let runner = POWERSHELL_COMMAND_FUNCTION;
    format!(
        r"function f {{
    $@VAR_dispatchFile@ = Join-Path $env:{session_root_env} '{state_directory}\{dispatch_file}'
    $@VAR_commandId@ = [IO.File]::ReadAllText($@VAR_dispatchFile@, [Text.Encoding]::UTF8)
    [IO.File]::Delete($@VAR_dispatchFile@)
    $@VAR_directory@ = Join-Path $env:{session_root_env} ('{commands_directory}\' + $@VAR_commandId@)
    $@VAR_workingDirectoryFile@ = Join-Path $@VAR_directory@ '{input_directory}\{working_directory_file}'
    $@VAR_workingDirectory@ = [IO.File]::ReadAllText($@VAR_workingDirectoryFile@, [Text.Encoding]::UTF8)
    {runner} -@VAR_CommandId@ $@VAR_commandId@ -@VAR_Directory@ $@VAR_directory@ -@VAR_WorkingDirectory@ $@VAR_workingDirectory@
}}",
    )
}
pub(super) fn posix_dispatcher() -> String {
    let session_root_env = SESSION_ROOT_ENV;
    let state_directory = SESSION_STATE_DIRECTORY;
    let dispatch_file = DISPATCH_FILE;
    let commands_directory = SESSION_COMMANDS_DIRECTORY;
    let input_directory = COMMAND_INPUT_DIRECTORY;
    let working_directory_file = COMMAND_WORKING_DIRECTORY_FILE;
    let runner = POSIX_COMMAND_FUNCTION;
    format!(
        r#"f() {{
    local @VAR_dispatch_file@="${{{session_root_env}}}/{state_directory}/{dispatch_file}"
    local @VAR_command_id@
    @VAR_command_id@="$(cat "$@VAR_dispatch_file@")" || return 1
    rm -f -- "$@VAR_dispatch_file@" || return 1
    local @VAR_native_directory@="${{{session_root_env}}}/{commands_directory}/$@VAR_command_id@"
    local @VAR_working_directory_file@="$@VAR_native_directory@/{input_directory}/{working_directory_file}"
    local @VAR_working_directory@
    @VAR_working_directory@="$(cat "$@VAR_working_directory_file@")" || return 1
    {runner} "$@VAR_command_id@" "$@VAR_native_directory@" "$@VAR_working_directory@"
}}"#,
    )
}
pub(super) fn nushell_dispatcher() -> String {
    let session_root_env = SESSION_ROOT_ENV;
    let state_directory = SESSION_STATE_DIRECTORY;
    let dispatch_file = DISPATCH_FILE;
    let commands_directory = SESSION_COMMANDS_DIRECTORY;
    let input_directory = COMMAND_INPUT_DIRECTORY;
    let working_directory_file = COMMAND_WORKING_DIRECTORY_FILE;
    let runner = POSIX_COMMAND_FUNCTION;
    format!(
        "def --env f [] {{\n    let @VAR_dispatch_file@ = ($env.{session_root_env} | path join '{state_directory}' '{dispatch_file}')\n    let @VAR_command_id@ = (open --raw $@VAR_dispatch_file@)\n    rm --force $@VAR_dispatch_file@\n    let @VAR_directory@ = ($env.{session_root_env} | path join '{commands_directory}' $@VAR_command_id@)\n    let @VAR_working_directory@ = (open --raw ($@VAR_directory@ | path join '{input_directory}' '{working_directory_file}'))\n    {runner} $@VAR_command_id@ $@VAR_directory@ $@VAR_working_directory@\n}}",
    )
}
fn render(template: &str, extra: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for &(placeholder, value) in COMMON.iter().chain(extra.iter()) {
        text = text.replace(placeholder, value);
    }
    text
}
const CMD_DISPATCHER: &str = r#"@echo off
set "@VAR_dispatch_file@=%FUNCTERM_SESSION_ROOT%\@SESSION_STATE@\@DISPATCH@"
set /p "@VAR_command_id@="<"%@VAR_dispatch_file@%" || exit /b 1
del /q "%@VAR_dispatch_file@%" || exit /b 1
set "@VAR_directory@=%FUNCTERM_SESSION_ROOT%\@SESSION_COMMANDS@\%@VAR_command_id@%"
set /p "@VAR_working_directory@="<"%@VAR_directory@%\@INPUT_DIR@\@WORKING_DIRECTORY@" || exit /b 1
call "%FUNCTERM_SESSION_ROOT%\startup\cmd_run.bat" "%@VAR_command_id@%" "%@VAR_directory@%" "%@VAR_working_directory@%"
exit /b %ERRORLEVEL%
"#;
pub (super) const POWERSHELL_STATE_PROMOTION : & str = "        foreach ($@VAR_variable@ in Get-Variable -Scope Local) {
            if (
                $@VAR_variable@.Name -notin $@VAR_existingVariables@ -and
                $@VAR_variable@.Options -notmatch 'ReadOnly|Constant'
            ) {
                Set-Variable -Scope Global -Name $@VAR_variable@.Name -Value $@VAR_variable@.Value
            }
        }
        foreach ($@VAR_function@ in Get-ChildItem Function:) {
            if (
                -not $@VAR_existingFunctions@.ContainsKey($@VAR_function@.Name) -or
                $@VAR_existingFunctions@[$@VAR_function@.Name] -ne $@VAR_function@.ScriptBlock.ToString()
            ) {
                Set-Item -LiteralPath ('Function:\\global:' + $@VAR_function@.Name) -Value $@VAR_function@.ScriptBlock
            }
        }
        foreach ($@VAR_alias@ in Get-ChildItem Alias:) {
            if (
                -not $@VAR_existingAliases@.ContainsKey($@VAR_alias@.Name) -or
                $@VAR_existingAliases@[$@VAR_alias@.Name] -ne $@VAR_alias@.Definition
            ) {
                Set-Alias -Scope Global -Name $@VAR_alias@.Name -Value $@VAR_alias@.Definition
            }
        }" ;
