use crate::contract::{
    CLAIM_RECEIPT_FILE, COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_SCRIPT_FILE,
    DONE_FILE, HELPER_EXECUTABLE_ENV, POWERSHELL_COMMAND_FUNCTION, SESSION_STATE_DIRECTORY,
    STDERR_FILE, STDOUT_FILE,
};
use crate::shell::shims::SESSION_ROOT_ENV;
const COMMON: [(&str, &str); 6] = [
    ("@COMMAND_DIR_ENV@", COMMAND_DIRECTORY_ENV),
    ("@COMMAND_ID_ENV@", COMMAND_ID_ENV),
    ("@DONE@", DONE_FILE),
    ("@HELPER_ENV@", HELPER_EXECUTABLE_ENV),
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
        &[("@COMMAND@", COMMAND_FILE), ("@SCRIPT@", COMMAND_FILE)],
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
    let dispatcher = crate::contract::POWERSHELL_DISPATCHER_FUNCTION;
    format ! ("function {dispatcher} {{
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
fn render(template: &str, extra: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for &(placeholder, value) in COMMON.iter().chain(extra.iter()) {
        text = text.replace(placeholder, value);
    }
    text
}
const CMD_DISPATCHER: &str = r#"@echo off
set "@VAR_previous_status@=%ERRORLEVEL%"
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
"%~dp0cmd_run.bat" "%@VAR_command_id@%" "%@VAR_directory@%" "%@VAR_working_directory@%" "%@VAR_separate_output@%" "%@VAR_previous_status@%"
"#;
