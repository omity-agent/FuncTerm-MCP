use super::{DoneOutput, receipt::CommandReceipt, write_done};
use crate::contract::{
    COMMAND_INPUT_DIRECTORY, COMMAND_OUTPUT_DIRECTORY, COMMAND_STATE_DIRECTORY,
    COMMAND_WORKING_DIRECTORY_FILE, DISPATCH_FILE, SESSION_COMMANDS_DIRECTORY,
    SESSION_STATE_DIRECTORY, STDERR_FILE,
};
use crate::shell::{ShellChoice, shims};
use anyhow::{Context as _, Result, ensure};
use std::path::Path;
pub(crate) fn claim_command(session_root: &Path, shell: ShellChoice) -> Result<String> {
    let command_id = consume_dispatch(session_root)?;
    validate_command_id(&command_id)?;
    let directory = session_root
        .join(SESSION_COMMANDS_DIRECTORY)
        .join(&command_id);
    let prepared = (|| prepare(&command_id, &directory, session_root, shell)?.encode(shell))();
    match prepared {
        Ok(receipt) => Ok(receipt),
        Err(error) => {
            publish_preparation_failure(&command_id, &directory, &error)?;
            Err(error)
        }
    }
}
fn consume_dispatch(session_root: &Path) -> Result<String> {
    let state = session_root.join(SESSION_STATE_DIRECTORY);
    let claimed = tempfile::tempdir_in(&state).context("failed to allocate dispatch claim")?;
    let destination = claimed.path().join(DISPATCH_FILE);
    let consumed = (|| {
        fs_err::rename(state.join(DISPATCH_FILE), &destination)
            .context("failed to claim pending command")?;
        fs_err::read_to_string(&destination).context("failed to read claimed command id")
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
    command_id: &str,
    directory: &Path,
    session_root: &Path,
    shell: ShellChoice,
) -> Result<CommandReceipt> {
    let input = directory.join(COMMAND_INPUT_DIRECTORY);
    let working_directory = fs_err::read_to_string(input.join(COMMAND_WORKING_DIRECTORY_FILE))
        .context("failed to read command working directory")?;
    ensure!(
        !working_directory.is_empty(),
        "command working directory is empty"
    );
    for child in [COMMAND_OUTPUT_DIRECTORY, COMMAND_STATE_DIRECTORY] {
        fs_err::create_dir_all(directory.join(child))?;
    }
    let shim_directory =
        std::env::var_os(shims::SHIM_DIR_ENV).context("missing shell shim directory")?;
    ensure!(!shim_directory.is_empty(), "shell shim directory is empty");
    shims::ensure_directory(Path::new(&shim_directory))?;
    shell.prepare_command(directory, session_root)?;
    Ok(CommandReceipt {
        command_id: command_id.to_owned(),
        directory: crate::text::path_text(directory, "claimed command directory")?,
        working_directory,
    })
}
fn publish_preparation_failure(
    command_id: &str,
    directory: &Path,
    error: &anyhow::Error,
) -> Result<()> {
    let stderr = directory.join(COMMAND_OUTPUT_DIRECTORY).join(STDERR_FILE);
    crate::publication::write_replace(&stderr, format!("failed to prepare command: {error:#}\n"))?;
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
