use super::{DoneOutput, receipt::CommandReceipt, write_done};
use crate::contract::{
    CommandDispatch, DISPATCH_FILE, SESSION_COMMANDS_DIRECTORY, SESSION_STATE_DIRECTORY,
    STDERR_FILE,
};
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, ensure};
use std::path::Path;
pub(crate) fn claim_command(session_root: &Path, shell: ShellChoice) -> Result<String> {
    let dispatch = consume_dispatch(session_root)?;
    let command_id = &dispatch.command_id;
    validate_command_id(command_id)?;
    let directory = session_root
        .join(SESSION_COMMANDS_DIRECTORY)
        .join(command_id);
    let prepared = (|| prepare(&dispatch, &directory, session_root, shell)?.encode(shell))();
    match prepared {
        Ok(receipt) => Ok(receipt),
        Err(error) => {
            publish_preparation_failure(command_id, &directory, &error)?;
            Err(error)
        }
    }
}
fn consume_dispatch(session_root: &Path) -> Result<CommandDispatch<String>> {
    let state = session_root.join(SESSION_STATE_DIRECTORY);
    let claimed = tempfile::tempdir_in(&state).context("failed to allocate dispatch claim")?;
    let destination = claimed.path().join(DISPATCH_FILE);
    let consumed = (|| {
        fs_err::rename(state.join(DISPATCH_FILE), &destination)
            .context("failed to claim pending command")?;
        let contents = fs_err::read(&destination).context("failed to read claimed command")?;
        sonic_rs::from_slice(&contents).context("failed to parse command dispatch")
    })();
    claimed.close().context("failed to remove dispatch claim")?;
    consumed
}
fn validate_command_id(command_id: &str) -> Result<()> {
    ensure!(
        !command_id.is_empty()
            && command_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "command id is not a safe path component"
    );
    Ok(())
}
fn prepare(
    dispatch: &CommandDispatch<String>,
    directory: &Path,
    session_root: &Path,
    shell: ShellChoice,
) -> Result<CommandReceipt> {
    ensure!(
        !dispatch.working_directory.is_empty(),
        "command working directory is empty"
    );
    shell.prepare_command(directory, session_root)?;
    Ok(CommandReceipt {
        command_id: dispatch.command_id.clone(),
        directory: crate::text::path_text(directory, "claimed command directory")?,
        working_directory: dispatch.working_directory.clone(),
        separate_output: dispatch.separate_output,
    })
}
fn publish_preparation_failure(
    command_id: &str,
    directory: &Path,
    error: &anyhow::Error,
) -> Result<()> {
    let stderr = directory.join(STDERR_FILE);
    crate::publication::write_replace(&stderr, format!("failed to prepare command: {error:#}\n"))?;
    let mut terminal = crate::shim::terminal_output()?;
    super::write_start_to(command_id, directory, &mut terminal)?;
    std::io::Write::write_all(
        &mut terminal,
        format!("failed to prepare command: {error:#}\n").as_bytes(),
    )
    .context("failed to report command preparation failure to terminal")?;
    let cwd = crate::text::path_text(
        &std::env::current_dir().context("failed to read preparation working directory")?,
        "preparation working directory",
    )?;
    write_done(
        &DoneOutput {
            command_id,
            exit_code: 1,
            time_consumption: "0ns",
            cwd: &cwd,
        },
        directory,
    )
    .context("failed to publish command preparation failure")
}
