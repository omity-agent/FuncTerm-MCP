pub(crate) const COMMAND_DIRECTORY_ENV: &str = "FUNCTERM_COMMAND_DIRECTORY";
pub(crate) const COMMAND_ID_ENV: &str = "FUNCTERM_COMMAND_ID";
pub(crate) const COMMAND_INPUT_DIRECTORY: &str = "input";
pub(crate) const COMMAND_FILE: &str = "command.txt";
pub(crate) const COMMAND_SCRIPT_FILE: &str = "command.cmd";
pub(crate) const COMMAND_WORKING_DIRECTORY_FILE: &str = "cwd.txt";
pub(crate) const COMMAND_OUTPUT_DIRECTORY: &str = "output";
pub(crate) const COMMAND_POWERSHELL_SCRIPT_FILE: &str = "command.ps1";
pub(crate) const COMMAND_STATE_DIRECTORY: &str = "state";
pub(crate) const DISPATCH_FILE: &str = "dispatch";
pub(crate) const DISPATCHER_COMMAND: &str = "f";
pub(crate) const DONE_FILE: &str = "done.json";
pub(crate) const HELPER_EXECUTABLE_ENV: &str = "FUNCTERM_HELPER_EXECUTABLE";
pub(crate) const POWERSHELL_COMMAND_FUNCTION: &str = "Invoke-FuncTermCommand";
pub(crate) const POSIX_COMMAND_FUNCTION: &str = "functerm_run_command";
pub(crate) const SESSION_COMMANDS_DIRECTORY: &str = "commands";
pub(crate) const SESSION_STATE_DIRECTORY: &str = "state";
pub(crate) const STDERR_FILE: &str = "stderr.txt";
pub(crate) const STARTED_FILE: &str = "started";
pub(crate) const STDOUT_FILE: &str = "stdout.txt";
pub(crate) const TERMINAL_MARKER_CODE: &[u8] = b"9999";
pub(crate) const TERMINAL_MARKER_END: &[u8] = b"end";
pub(crate) const TERMINAL_MARKER_NAME: &[u8] = b"FuncTerm";
pub(crate) const TERMINAL_MARKER_START: &[u8] = b"start";
pub(crate) fn window_title_sequence(title: &str) -> anyhow::Result<String> {
    if title.chars().any(char::is_control) {
        anyhow::bail!("terminal_model_title must not contain control characters");
    }
    Ok(format!("\x1b]2;{title}\x1b\\"))
}
