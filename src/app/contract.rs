pub(crate) const COMMAND_DIRECTORY_ENV: &str = "FUNCTERM_COMMAND_DIRECTORY";
pub(crate) const COMMAND_ID_ENV: &str = "FUNCTERM_COMMAND_ID";
pub(crate) const COMMAND_FILE: &str = "command.txt";
pub(crate) const COMMAND_SCRIPT_FILE: &str = "command.cmd";
pub(crate) const DISPATCH_FILE: &str = "dispatch";
pub(crate) const CLAIM_RECEIPT_FILE: &str = "claim";
pub(crate) const DISPATCHER_COMMAND: &str = "f";
const COMMAND_REPORT_FILE: &str = "result.json";
pub(crate) const EXECUTABLE_NAME: &str = if cfg!(windows) {
    concat!(env!("CARGO_PKG_NAME"), ".exe")
} else {
    env!("CARGO_PKG_NAME")
};
pub(crate) const HELPER_EXECUTABLE_ENV: &str = "FUNCTERM_HELPER_EXECUTABLE";
pub(crate) const NUSHELL_SUBMISSION_FILE: &str = "submission.nu";
pub(crate) const POWERSHELL_DISPATCHER_FUNCTION: &str = "Invoke-FuncTermDispatcher";
pub(crate) const RELEASED_FILE: &str = "released";
pub(crate) const SESSION_COMMANDS_DIRECTORY: &str = "commands";
pub(crate) const SESSION_STATE_DIRECTORY: &str = "state";
pub(crate) const STDERR_FILE: &str = "stderr.txt";
pub(crate) const STDOUT_FILE: &str = "stdout.txt";
pub(crate) const TITLE_FILE: &str = "title";
pub(crate) const TERMINAL_MARKER_CODE: &[u8] = b"9999";
pub(crate) const TERMINAL_MARKER_END: &[u8] = b"end";
pub(crate) const TERMINAL_MARKER_NAME: &[u8] = b"FuncTerm";
pub(crate) const TERMINAL_MARKER_START: &[u8] = b"start";
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct CommandDispatch<Text> {
    pub(crate) command_id: Text,
    pub(crate) working_directory: Text,
    pub(crate) separate_output: bool,
}
pub(crate) fn command_directory(
    command_root: &std::path::Path,
    command_id: &str,
) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(
        command_id.strip_prefix("command-").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }),
        "invalid command id"
    );
    Ok(command_root.join(command_id))
}
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct CommandReport<Text> {
    pub(crate) command_id: Text,
    pub(crate) elapsed: Option<core::time::Duration>,
    pub(crate) cwd: Text,
    pub(crate) outcome: CommandOutcome,
}
#[derive(Clone, Copy, serde::Deserialize, serde::Serialize)]
pub(crate) enum CommandOutcome {
    Returned { exit_code: i32, shell_exit: bool },
    HandedOff,
}
impl CommandOutcome {
    pub(crate) const fn exit_code(self) -> i32 {
        match self {
            Self::Returned {
                exit_code,
                shell_exit: _,
            } => exit_code,
            Self::HandedOff => 0,
        }
    }
    pub(crate) const fn exits_shell(self) -> bool {
        matches!(
            self,
            Self::Returned {
                exit_code: _,
                shell_exit: true
            }
        )
    }
}
impl CommandReport<String> {
    pub(crate) fn read(directory: &std::path::Path) -> anyhow::Result<Option<Self>> {
        use anyhow::Context as _;
        match fs_err::read(directory.join(COMMAND_REPORT_FILE)) {
            Ok(bytes) => Ok(Some(
                sonic_rs::from_slice(&bytes).context("failed to decode command report")?,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("failed to read command report"),
        }
    }
    pub(crate) fn exists(directory: &std::path::Path) -> anyhow::Result<bool> {
        directory
            .join(COMMAND_REPORT_FILE)
            .try_exists()
            .map_err(Into::into)
    }
}
impl<Text: serde::Serialize> CommandReport<Text> {
    pub(crate) fn publish(&self, directory: &std::path::Path) -> anyhow::Result<()> {
        use anyhow::Context as _;
        let bytes = sonic_rs::to_vec(self).context("failed to encode command report")?;
        crate::publication::write_once(&directory.join(COMMAND_REPORT_FILE), bytes)
            .context("failed to publish command report")
    }
}
pub(crate) fn window_title_sequence(title: &str) -> anyhow::Result<String> {
    validate_window_title(title)?;
    Ok(format!("\x1b]2;{title}\x1b\\"))
}
pub(crate) fn validate_window_title(title: &str) -> anyhow::Result<()> {
    if title.chars().any(char::is_control) {
        anyhow::bail!("terminal_model_title must not contain control characters");
    }
    Ok(())
}
