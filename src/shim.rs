mod startup;
mod stdio;
use crate::contract::{DONE_FILE, DONE_TEMP_FILE};
use crate::shell::{ShellChoice, ShellStartup, shims};
use anyhow::{Context as _, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
pub(crate) fn run_if_requested() -> Result<Option<i32>> {
    let Some(choice) = requested_shell() else {
        return Ok(None);
    };
    if !is_shim_invocation()? {
        return Ok(None);
    }
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.is_empty() || interactive_arguments(choice, &arguments) {
        return run_interactive(choice).map(Some);
    }
    run_passthrough(choice, arguments).map(Some)
}
fn requested_shell() -> Option<ShellChoice> {
    let argument = std::env::args_os().next()?;
    let name = Path::new(&argument)
        .file_name()
        .and_then(|name| name.to_str())?;
    ShellChoice::parse(name).ok()
}
fn is_shim_invocation() -> Result<bool> {
    let Some(shim_dir) = std::env::var_os(shims::SHIM_DIR_ENV) else {
        return Ok(false);
    };
    let executable = std::env::current_exe().context("failed to resolve shim executable")?;
    let executable_dir = executable
        .parent()
        .context("shim executable path has no parent")?;
    Ok(executable_dir.canonicalize().with_context(|| {
        format!(
            "failed to resolve shim executable directory {}",
            executable_dir.display()
        )
    })? == PathBuf::from(shim_dir)
        .canonicalize()
        .context("failed to resolve shim directory")?)
}
fn interactive_arguments(choice: ShellChoice, arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = arguments
        .iter()
        .map(|argument| argument.to_str().map(str::to_ascii_lowercase))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    match choice {
        ShellChoice::PowerShell => powershell_interactive_arguments(&values),
        ShellChoice::Bash | ShellChoice::Zsh => values
            .iter()
            .all(|value| matches!(value.as_str(), "-i" | "-l" | "--login")),
        ShellChoice::NuShell => values.iter().all(|value| {
            matches!(
                value.as_str(),
                "--login" | "--no-config-file" | "--no-history"
            )
        }),
    }
}
fn powershell_interactive_arguments(values: &[String]) -> bool {
    let mut index = 0_usize;
    while index < values.len() {
        let Some(value) = values.get(index) else {
            return false;
        };
        match value.as_str() {
            "-nologo" | "-noexit" | "-noprofile" => index += 1,
            "-executionpolicy" if index + 1 < values.len() => index += 2,
            _ => return false,
        }
    }
    true
}
fn run_passthrough(choice: ShellChoice, arguments: Vec<std::ffi::OsString>) -> Result<i32> {
    let status = Command::new(real_executable(choice)?)
        .args(arguments)
        .status()
        .with_context(|| format!("failed to run {}", choice.canonical_name()))?;
    Ok(exit_code(status))
}
fn run_interactive(choice: ShellChoice) -> Result<i32> {
    let parent_shell = current_shell().unwrap_or(choice);
    let active_shell_file = active_shell_file()?;
    let session_root = nested_session_root(choice)?;
    fs::create_dir_all(&session_root).context("failed to create nested shell root")?;
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let startup = choice.startup(&cwd, &session_root)?;
    let ready_file = startup.ready_file.clone();
    let child = spawn_shell(choice, startup)?;
    let mut active_shell = ActiveShellGuard::new(active_shell_file, parent_shell);
    let status = startup::run_shell_until_exit(child, &ready_file, || {
        active_shell.activate(choice)?;
        complete_active_command(&cwd)?;
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
    stdio::attach_terminal_stdio(&mut command)?;
    command
        .spawn()
        .with_context(|| format!("failed to spawn {}", choice.canonical_name()))
}
fn complete_active_command(cwd: &Path) -> Result<()> {
    let Some(directory) = std::env::var_os(shims::COMMAND_DIRECTORY_ENV) else {
        return Ok(());
    };
    let Some(command_id) = std::env::var_os(shims::COMMAND_ID_ENV) else {
        return Ok(());
    };
    let directory_path = PathBuf::from(directory);
    let done_path = directory_path.join(DONE_FILE);
    if done_path.exists() {
        return Ok(());
    }
    let done = EarlyDone {
        command_id: command_id.to_string_lossy().into_owned(),
        exit_code: 0,
        cwd: cwd.to_string_lossy().into_owned(),
    };
    let text = sonic_rs::to_string(&done).context("failed to serialize early done file")?;
    let temp_path = directory_path.join(DONE_TEMP_FILE);
    fs::write(&temp_path, text).context("failed to write early done file")?;
    match fs::rename(&temp_path, &done_path) {
        Ok(()) => Ok(()),
        Err(_error) if done_path.exists() => {
            fs::remove_file(&temp_path).context("failed to remove obsolete early done file")?;
            Ok(())
        }
        Err(error) => Err(error).context("failed to publish early done file"),
    }
}
fn current_shell() -> Option<ShellChoice> {
    let value = std::env::var(shims::CURRENT_SHELL_ENV).ok()?;
    ShellChoice::parse(&value).ok()
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
#[derive(Serialize)]
struct EarlyDone {
    command_id: String,
    exit_code: i32,
    cwd: String,
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
