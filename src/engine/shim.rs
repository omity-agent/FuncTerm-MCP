mod invocation;
mod routing;
mod stdio;
pub(crate) use self::stdio::terminal_output;
use crate::shell::{ShellChoice, ShellStartup, shims};
use anyhow::{Context as _, Result};
use core::time::Duration;
use routing::LaunchRoute;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::Instant;
pub(crate) fn run_if_requested() -> Result<Option<i32>> {
    let Some(choice) = requested_shell() else {
        return Ok(None);
    };
    if !is_shim_invocation()? {
        return Ok(None);
    }
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    match LaunchRoute::detect(choice, &arguments) {
        LaunchRoute::ManagedSession => run_interactive(choice, &arguments).map(Some),
        LaunchRoute::NativeProcess => run_passthrough(choice, arguments).map(Some),
    }
}
fn requested_shell() -> Option<ShellChoice> {
    let argument = std::env::args_os().next()?;
    let name = Path::new(&argument)
        .file_name()
        .and_then(|name| name.to_str())?;
    ShellChoice::from_shim_name(name)
}
fn is_shim_invocation() -> Result<bool> {
    let Some(shim_dir) = std::env::var_os(shims::SHIM_DIR_ENV) else {
        return Ok(false);
    };
    let invocation = std::env::args_os()
        .next()
        .context("missing executable invocation")?;
    let executable = which::which(invocation).context("failed to resolve shim invocation")?;
    let executable_dir = executable
        .parent()
        .context("shim executable path has no parent")?;
    let directory_matches = executable_dir.canonicalize().with_context(|| {
        format!(
            "failed to resolve shim executable directory {}",
            executable_dir.display()
        )
    })? == PathBuf::from(shim_dir)
        .canonicalize()
        .context("failed to resolve shim directory")?;
    Ok(directory_matches
        && same_file::is_same_file(
            executable,
            std::env::current_exe().context("failed to resolve shim executable")?,
        )?)
}
fn run_passthrough(choice: ShellChoice, arguments: Vec<std::ffi::OsString>) -> Result<i32> {
    let status = Command::new(real_executable(choice)?)
        .args(arguments)
        .status()
        .with_context(|| format!("failed to run {}", choice.canonical_name()))?;
    Ok(exit_code(status))
}
fn run_interactive(choice: ShellChoice, arguments: &[std::ffi::OsString]) -> Result<i32> {
    let command_started_at = Instant::now();
    let parent_shell = current_shell().unwrap_or(choice);
    let active_shell_file = active_shell_file()?;
    let session_root = nested_session_root(choice)?;
    fs_err::create_dir_all(&session_root)?;
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let startup = choice.startup(
        &cwd,
        &session_root,
        routing::load_profile(arguments)?,
        &crate::runtime::protocol::EnvironmentSnapshot::from_variables(std::env::vars_os()),
    )?;
    let ready_file = startup.ready_file.clone();
    let child = spawn_shell(choice, startup)?;
    let mut active_shell = ActiveShellGuard::new(active_shell_file, parent_shell);
    let status = invocation::run_shell_until_exit(child, &ready_file, || {
        active_shell.activate(choice)?;
        complete_active_command(&cwd, command_started_at.elapsed())?;
        Ok(())
    })?;
    Ok(exit_code(status))
}
fn spawn_shell(choice: ShellChoice, startup: ShellStartup) -> Result<std::process::Child> {
    let mut command = Command::new(real_executable(choice)?);
    command.args(startup.args);
    for (name, value) in startup.env {
        command.env(name, value);
    }
    stdio::attach_session_stdio(&mut command)?;
    command
        .spawn()
        .with_context(|| format!("failed to spawn {}", choice.canonical_name()))
}
fn complete_active_command(cwd: &Path, time_consumption: Duration) -> Result<()> {
    let Some(directory) = std::env::var_os(shims::COMMAND_DIRECTORY_ENV) else {
        return Ok(());
    };
    let Some(command_id) = std::env::var_os(shims::COMMAND_ID_ENV) else {
        return Ok(());
    };
    let directory_path = PathBuf::from(directory);
    let command_id_text = command_id.to_string_lossy().into_owned();
    let time_consumption_text = format!("{time_consumption:?}");
    let cwd_text = cwd.to_string_lossy().into_owned();
    let done = crate::app::command_state::DoneOutput {
        command_id: &command_id_text,
        exit_code: 0,
        time_consumption: &time_consumption_text,
        cwd: &cwd_text,
    };
    let mut terminal = stdio::terminal_output()?;
    crate::app::command_state::write_early_done_guard(&command_id_text, &directory_path)?;
    crate::app::command_state::write_done_to(&done, &directory_path, &mut terminal)
        .context("failed to publish early done file")
}
fn current_shell() -> Option<ShellChoice> {
    let value = std::env::var(shims::CURRENT_SHELL_ENV).ok()?;
    ShellChoice::from_canonical_name(&value).ok()
}
fn active_shell_file() -> Result<PathBuf> {
    std::env::var_os(shims::ACTIVE_SHELL_FILE_ENV)
        .map(PathBuf::from)
        .context("missing active shell state path")
}
fn nested_session_root(choice: ShellChoice) -> Result<PathBuf> {
    let root = std::env::var_os(shims::SESSION_ROOT_ENV)
        .map(PathBuf::from)
        .context("missing shell session root")?;
    Ok(root
        .join("nested")
        .join(choice.canonical_name())
        .join(std::process::id().to_string()))
}
fn real_executable(choice: ShellChoice) -> Result<String> {
    std::env::var(choice.shim_env_name()).with_context(|| {
        format!(
            "missing real executable for {} shim",
            choice.canonical_name()
        )
    })
}
fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}
struct ActiveShellGuard {
    path: PathBuf,
    parent_shell: ShellChoice,
    active: bool,
}
impl ActiveShellGuard {
    const fn new(path: PathBuf, parent_shell: ShellChoice) -> Self {
        Self {
            path,
            parent_shell,
            active: false,
        }
    }
    fn activate(&mut self, shell: ShellChoice) -> Result<()> {
        shims::write_active_shell(&self.path, shell)?;
        self.active = true;
        Ok(())
    }
}
impl Drop for ActiveShellGuard {
    fn drop(&mut self) {
        if self.active
            && let Err(error) = shims::write_active_shell(&self.path, self.parent_shell)
        {
            eprintln!("failed to restore active shell state: {error:#}");
        }
    }
}
