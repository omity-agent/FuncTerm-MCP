use crate::contract::{
    CLAIM_RECEIPT_FILE, COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV,
    COMMAND_INPUT_DIRECTORY, COMMAND_OUTPUT_DIRECTORY, COMMAND_POWERSHELL_SCRIPT_FILE,
    COMMAND_SCRIPT_FILE, COMMAND_STATE_DIRECTORY, DONE_FILE, HELPER_EXECUTABLE_ENV,
    POSIX_COMMAND_FUNCTION, POWERSHELL_COMMAND_FUNCTION, SESSION_STATE_DIRECTORY, STDERR_FILE,
    STDOUT_FILE,
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
            ("@CLAIM@", CLAIM_RECEIPT_FILE),
            ("@SESSION_STATE@", SESSION_STATE_DIRECTORY),
        ],
    );
    super::VariableNamespace::new().render(&rendered)
}
pub(super) fn powershell_dispatcher() -> String {
    let session_root_env = SESSION_ROOT_ENV;
    let runner = POWERSHELL_COMMAND_FUNCTION;
    format ! ("function f {{
    try {{
        $@VAR_receipt@ = Invoke-FuncTermHelper -@VAR_Arguments@ @(
            'internal-claim-command', '--session-root', $env:{session_root_env}, '--shell', 'powershell'
        ) | ConvertFrom-Json
        . {runner} -@VAR_CommandId@ $@VAR_receipt@.command_id -@VAR_Directory@ $@VAR_receipt@.directory -@VAR_WorkingDirectory@ $@VAR_receipt@.working_directory -@VAR_SeparateOutput@ $@VAR_receipt@.separate_output
    }}
    finally {{
        @POWERSHELL_STATE_CLEANUP@
    }}
}}")
}
pub(super) fn posix_dispatcher() -> String {
    let session_root_env = SESSION_ROOT_ENV;
    let state_directory = SESSION_STATE_DIRECTORY;
    let receipt_file = CLAIM_RECEIPT_FILE;
    let helper_env = HELPER_EXECUTABLE_ENV;
    let runner = POSIX_COMMAND_FUNCTION;
    format!(
        r#"f() {{
    local @VAR_helper@="${{{helper_env}}}"
    @VAR_helper@="$(functerm_posix_path "$@VAR_helper@")" || return 1
    local @VAR_receipt_file@="${{{session_root_env}}}/{state_directory}/{receipt_file}"
    @VAR_receipt_file@="$(functerm_posix_path "$@VAR_receipt_file@")" || return 1
    "$@VAR_helper@" internal-claim-command --session-root "${{{session_root_env}}}" --shell "$FUNCTERM_CURRENT_SHELL" > "$@VAR_receipt_file@" || return 1
    local @VAR_command_id@
    local @VAR_native_directory@
    local @VAR_working_directory@
    local @VAR_separate_output@
    {{
        IFS= read -r -d '' @VAR_command_id@ &&
        IFS= read -r -d '' @VAR_native_directory@ &&
        IFS= read -r -d '' @VAR_working_directory@ &&
        IFS= read -r -d '' @VAR_separate_output@
    }} < "$@VAR_receipt_file@" || return 1
    {runner} "$@VAR_command_id@" "$@VAR_native_directory@" "$@VAR_working_directory@" "$@VAR_separate_output@"
}}"#,
    )
}
pub(super) fn nushell_dispatcher() -> String {
    let session_root_env = SESSION_ROOT_ENV;
    let helper_env = HELPER_EXECUTABLE_ENV;
    let runner = POSIX_COMMAND_FUNCTION;
    format!(
        "def --env f [] {{\n    let @VAR_helper@ = $env.{helper_env}\n    let @VAR_result@ = (^$@VAR_helper@ internal-claim-command --session-root $env.{session_root_env} --shell nu | complete)\n    if $@VAR_result@.exit_code != 0 {{\n        print --stderr $@VAR_result@.stderr\n        return\n    }}\n    let @VAR_receipt@ = ($@VAR_result@.stdout | from json)\n    {runner} $@VAR_receipt@.command_id $@VAR_receipt@.directory $@VAR_receipt@.working_directory $@VAR_receipt@.separate_output\n}}",
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
set "@VAR_receipt_file@=%FUNCTERM_SESSION_ROOT%\@SESSION_STATE@\@CLAIM@"
"%@HELPER_ENV@%" internal-claim-command --session-root "%FUNCTERM_SESSION_ROOT%" --shell cmd > "%@VAR_receipt_file@%" || exit /b 1
set "@VAR_command_id@="
set "@VAR_directory@="
set "@VAR_working_directory@="
set "@VAR_separate_output@="
for /f "usebackq delims=" %%r in ("%@VAR_receipt_file@%") do (
    if not defined @VAR_command_id@ (
        set "@VAR_command_id@=%%r"
    ) else if not defined @VAR_directory@ (
        set "@VAR_directory@=%%r"
    ) else if not defined @VAR_working_directory@ (
        set "@VAR_working_directory@=%%r"
    ) else (
        set "@VAR_separate_output@=%%r"
    )
)
call "%~dp0cmd_run.bat" "%@VAR_command_id@%" "%@VAR_directory@%" "%@VAR_working_directory@%" "%@VAR_separate_output@%"
exit /b %ERRORLEVEL%
"#;
pub(super) const POWERSHELL_COMMAND_ERROR_TRAP: &str = concat!(
    "'trap {\n",
    "    if (\n",
    "        $_.Exception -is [System.Management.Automation.CommandNotFoundException] -and\n",
    "        $ErrorActionPreference -ne [System.Management.Automation.ActionPreference]::Stop\n",
    "    ) {\n",
    "        $null = $@VAR_commandErrors@.Add($_)\n",
    "        continue\n",
    "    }\n",
    "    break\n",
    "}'",
);
