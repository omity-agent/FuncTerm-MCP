use super::{receipt::CommandReceipt, report_boundary};
use crate::contract::{
    CommandDispatch, CommandOutcome, CommandReport, DISPATCH_FILE, SESSION_COMMANDS_DIRECTORY,
    STDERR_FILE, command_directory,
};
use crate::shell::{ShellChoice, shims::FRAME_STATE_ENV};
use anyhow::{Context as _, Result, ensure};
use core::time::Duration;
use std::path::{Path, PathBuf};
pub(crate) fn claim_command(
    session_root: &Path,
    shell: ShellChoice,
    output_file: Option<&Path>,
) -> Result<String> {
    let state = PathBuf::from(
        std::env::var_os(FRAME_STATE_ENV).context("missing shell frame state directory")?,
    );
    let dispatch = consume_dispatch(&state)?;
    let command_id = &dispatch.command_id;
    let directory = command_directory(&session_root.join(SESSION_COMMANDS_DIRECTORY), command_id)?;
    let prepared = (|| {
        let receipt = prepare(&dispatch, &directory, shell)?.encode(shell)?;
        if let Some(path) = output_file {
            crate::publication::write_replace(path, &receipt).map_err(|error| {
                tracing::debug!(?error, "command receipt publication failed");
                anyhow::anyhow!("failed to publish command receipt")
            })?;
        }
        Ok(receipt)
    })();
    match prepared {
        Ok(receipt) => Ok(receipt),
        Err(error) => {
            publish_preparation_failure(command_id, &directory, &error)?;
            Err(error)
        }
    }
}
fn consume_dispatch(state: &Path) -> Result<CommandDispatch<String>> {
    let claimed = tempfile::tempdir_in(state).context("failed to allocate dispatch claim")?;
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
fn prepare(
    dispatch: &CommandDispatch<String>,
    directory: &Path,
    shell: ShellChoice,
) -> Result<CommandReceipt> {
    ensure!(
        !dispatch.working_directory.is_empty(),
        "command working directory is empty"
    );
    shell.prepare_command(directory)?;
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
    report_boundary(
        &CommandReport {
            command_id,
            elapsed: Some(Duration::ZERO),
            cwd: &cwd,
            outcome: CommandOutcome::Returned {
                exit_code: 1,
                shell_exit: false,
            },
        },
        directory,
        &mut terminal,
    )
    .context("failed to publish command preparation failure")
}
