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
    render(
        CMD_DISPATCHER,
        &[
            ("@DISPATCH@", DISPATCH_FILE),
            ("@SESSION_COMMANDS@", SESSION_COMMANDS_DIRECTORY),
            ("@SESSION_STATE@", SESSION_STATE_DIRECTORY),
            ("@WORKING_DIRECTORY@", COMMAND_WORKING_DIRECTORY_FILE),
        ],
    )
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
    $dispatchFile = Join-Path $env:{session_root_env} '{state_directory}\{dispatch_file}'
    $commandId = [IO.File]::ReadAllText($dispatchFile, [Text.Encoding]::UTF8)
    [IO.File]::Delete($dispatchFile)
    $directory = Join-Path $env:{session_root_env} ('{commands_directory}\' + $commandId)
    $workingDirectoryFile = Join-Path $directory '{input_directory}\{working_directory_file}'
    $workingDirectory = [IO.File]::ReadAllText($workingDirectoryFile, [Text.Encoding]::UTF8)
    {runner} -CommandId $commandId -Directory $directory -WorkingDirectory $workingDirectory
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
    local dispatch_file="${{{session_root_env}}}/{state_directory}/{dispatch_file}"
    local command_id
    command_id="$(cat "$dispatch_file")" || return 1
    rm -f -- "$dispatch_file" || return 1
    local native_directory="${{{session_root_env}}}/{commands_directory}/$command_id"
    local working_directory_file="$native_directory/{input_directory}/{working_directory_file}"
    local working_directory
    working_directory="$(cat "$working_directory_file")" || return 1
    {runner} "$command_id" "$native_directory" "$working_directory"
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
        "def --env f [] {{\n    let dispatch_file = ($env.{session_root_env} | path join '{state_directory}' '{dispatch_file}')\n    let command_id = (open --raw $dispatch_file)\n    rm --force $dispatch_file\n    let directory = ($env.{session_root_env} | path join '{commands_directory}' $command_id)\n    let working_directory = (open --raw ($directory | path join '{input_directory}' '{working_directory_file}'))\n    {runner} $command_id $directory $working_directory\n}}",
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
set "dispatch_file=%FUNCTERM_SESSION_ROOT%\@SESSION_STATE@\@DISPATCH@"
set /p "command_id="<"%dispatch_file%" || exit /b 1
del /q "%dispatch_file%" || exit /b 1
set "directory=%FUNCTERM_SESSION_ROOT%\@SESSION_COMMANDS@\%command_id%"
set /p "working_directory="<"%directory%\@INPUT_DIR@\@WORKING_DIRECTORY@" || exit /b 1
call "%FUNCTERM_SESSION_ROOT%\startup\cmd_run.bat" "%command_id%" "%directory%" "%working_directory%"
exit /b %ERRORLEVEL%
"#;
pub (super) const POWERSHELL_STATE_PROMOTION : & str = "        foreach ($variable in Get-Variable -Scope Local) {
            if (
                $variable.Name -notin $existingVariables -and
                $variable.Options -notmatch 'ReadOnly|Constant'
            ) {
                Set-Variable -Scope Global -Name $variable.Name -Value $variable.Value
            }
        }
        foreach ($function in Get-ChildItem Function:) {
            if (
                -not $existingFunctions.ContainsKey($function.Name) -or
                $existingFunctions[$function.Name] -ne $function.ScriptBlock.ToString()
            ) {
                Set-Item -LiteralPath ('Function:\\global:' + $function.Name) -Value $function.ScriptBlock
            }
        }
        foreach ($alias in Get-ChildItem Alias:) {
            if (
                -not $existingAliases.ContainsKey($alias.Name) -or
                $existingAliases[$alias.Name] -ne $alias.Definition
            ) {
                Set-Alias -Scope Global -Name $alias.Name -Value $alias.Definition
            }
	        }" ;
