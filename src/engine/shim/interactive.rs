use super::{exit_code, invocation, routing, spawning, stdio};
use crate::contract::{
    CommandOutcome, CommandReport, SESSION_COMMANDS_DIRECTORY, command_directory,
};
use crate::runtime::protocol::{EnvironmentSnapshot, environment_name_equals};
use crate::shell::frame::ShellFrame;
use crate::shell::{ShellChoice, shims};
use anyhow::{Context as _, Result, ensure};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn run(choice: ShellChoice, arguments: &[OsString]) -> Result<i32> {
    let started = Instant::now();
    let parent = ShellFrame::from_environment()?;
    let frame = ShellFrame::nested(choice);
    let tab_root =
        PathBuf::from(std::env::var_os(shims::SESSION_ROOT_ENV).context("missing Tab workspace")?);
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let command_id = active_command_id()?;
    let workspace = create_workspace(frame, &tab_root)?;
    let mut variables = EnvironmentSnapshot::capture().variables();
    variables.retain(|pair| {
        ![shims::COMMAND_ID_ENV, shims::COMMAND_DIRECTORY_ENV]
            .iter()
            .any(|name| environment_name_equals(&pair.0, name))
    });
    let startup = frame.startup(
        &cwd,
        &tab_root,
        routing::load_profile(arguments)?,
        routing::ignore_shell_style()?,
        &EnvironmentSnapshot::from_variables(variables),
    )?;
    let ready_file = startup.ready_file.clone();
    let mut activated = false;
    let mut suspended = None;
    let status = invocation::run_shell_until_exit(
        &ready_file,
        || spawning::spawn_shell(choice, startup),
        || {
            frame.route(cwd.clone(), None).publish(&tab_root)?;
            activated = true;
            if let Some(id) = command_id {
                hand_off(&tab_root, &id, &cwd, started)?;
                suspended = Some(id);
            }
            Ok(())
        },
    );
    if activated {
        parent.route(cwd, suspended).publish(&tab_root)?;
    }
    if let Err(error) = workspace.close() {
        tracing::error!("could not remove exited shell workspace");
        tracing::debug!(%error, "nested shell workspace cleanup failure");
    }
    Ok(exit_code(status?))
}
fn create_workspace(frame: ShellFrame, tab_root: &Path) -> Result<tempfile::TempDir> {
    let workspace = frame.workspace(tab_root);
    let parent = workspace
        .parent()
        .context("missing nested shell parent directory")?;
    fs_err::create_dir_all(parent)?;
    tempfile::Builder::new()
        .prefix(&frame.id().to_string())
        .rand_bytes(0)
        .tempdir_in(parent)
        .context("failed to create nested shell workspace")
}
fn active_command_id() -> Result<Option<String>> {
    let Some(value) = std::env::var_os(shims::COMMAND_ID_ENV) else {
        return Ok(None);
    };
    let command_id = value
        .into_string()
        .map_err(|_value| anyhow::anyhow!("command id is not UTF-8"))?;
    Ok(Some(command_id))
}
fn hand_off(tab_root: &Path, command_id: &str, cwd: &Path, started: Instant) -> Result<()> {
    let directory = command_directory(&tab_root.join(SESSION_COMMANDS_DIRECTORY), command_id)?;
    if let Some(previous) = CommandReport::read(&directory)? {
        ensure!(
            previous.command_id == command_id
                && matches!(previous.outcome, CommandOutcome::HandedOff),
            "cannot hand off an already returned command"
        );
        return Ok(());
    }
    let cwd_text = crate::text::path_text(cwd, "child shell working directory")?;
    let report = CommandReport {
        command_id,
        elapsed: Some(started.elapsed()),
        cwd: cwd_text.as_str(),
        outcome: CommandOutcome::HandedOff,
    };
    crate::app::command_state::report_boundary(&report, &directory, &mut stdio::terminal_output()?)
        .context("failed to publish nested shell handoff")
}
