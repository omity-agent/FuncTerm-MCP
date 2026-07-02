use crate::shell::{ShellChoice, ShellStartup, shims};
use anyhow::{Context as _, Result};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as _};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
pub(crate) fn run_if_requested() -> Result<Option<i32>> {
    let Some(choice) = requested_shell() else {
        return Ok(None);
    };
    if std::env::var_os(shims::SHIM_DIR_ENV).is_none() {
        return Ok(None);
    }
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.is_empty() {
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
    shims::write_active_shell(&active_shell_file, choice)?;
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let startup = choice.startup(&cwd, &session_root)?;
    let ready_file = startup.ready_file.clone();
    let child = spawn_shell(choice, startup)?;
    let status = run_shell_until_exit(child, &ready_file, || {
        shims::write_active_shell(&active_shell_file, choice)?;
        complete_active_command(&cwd)
    })?;
    shims::write_active_shell(&active_shell_file, parent_shell)?;
    Ok(exit_code(status))
}
fn spawn_shell(choice: ShellChoice, startup: ShellStartup) -> Result<std::process::Child> {
    let mut command = Command::new(real_executable(choice)?);
    command.args(startup.args);
    for (name, value) in startup.env {
        command.env(name, value);
    }
    attach_terminal_stdio(&mut command)?;
    command
        .spawn()
        .with_context(|| format!("failed to spawn {}", choice.canonical_name()))
}
#[cfg(unix)]
fn attach_terminal_stdio(command: &mut Command) -> Result<()> {
    let input = fs::File::open("/dev/tty").context("failed to open terminal input")?;
    let output = fs::OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .context("failed to open terminal output")?;
    let error = output
        .try_clone()
        .context("failed to clone terminal output")?;
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error));
    Ok(())
}
#[cfg(windows)]
fn attach_terminal_stdio(command: &mut Command) -> Result<()> {
    let input = fs::File::open("CONIN$").context("failed to open console input")?;
    let output = fs::OpenOptions::new()
        .write(true)
        .open("CONOUT$")
        .context("failed to open console output")?;
    let error = output
        .try_clone()
        .context("failed to clone console output")?;
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error));
    Ok(())
}
#[cfg(not(any(unix, windows)))]
fn attach_terminal_stdio(_command: &mut Command) -> Result<()> {
    anyhow::bail!("interactive shell shims are not supported on this platform")
}
fn run_shell_until_exit(
    mut child: std::process::Child,
    ready_file: &Path,
    on_ready: impl FnOnce() -> Result<()>,
) -> Result<ExitStatus> {
    let parent = ready_file
        .parent()
        .context("nested ready path has no parent")?;
    let (tx, rx) = mpsc::channel();
    let notify_tx = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            let _sent = notify_tx.send(StartupEvent::Filesystem(event.map(|_| ())));
        },
        Config::default(),
    )
    .context("failed to create nested shell startup watcher")?;
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .with_context(|| {
            format!(
                "failed to watch nested shell directory {}",
                parent.display()
            )
        })?;
    let wait_tx = tx;
    thread::spawn(move || {
        let status = child.wait().context("failed to wait for nested shell");
        let _sent = wait_tx.send(StartupEvent::Exited(status));
    });
    let mut ready_handler = Some(on_ready);
    if ready_file.exists()
        && let Some(handler) = ready_handler.take()
    {
        handler()?;
    }
    loop {
        match rx
            .recv()
            .context("nested shell startup watcher disconnected")?
        {
            StartupEvent::Filesystem(Ok(())) => {
                if ready_file.exists()
                    && let Some(handler) = ready_handler.take()
                {
                    handler()?;
                }
            }
            StartupEvent::Filesystem(Err(error)) => return Err(error).context("watcher failed"),
            StartupEvent::Exited(status) => return status,
        }
    }
}
fn complete_active_command(cwd: &Path) -> Result<()> {
    let Some(directory) = std::env::var_os(shims::COMMAND_DIRECTORY_ENV) else {
        return Ok(());
    };
    let Some(command_id) = std::env::var_os(shims::COMMAND_ID_ENV) else {
        return Ok(());
    };
    let directory_path = PathBuf::from(directory);
    let done_path = directory_path.join("done.json");
    if done_path.exists() {
        return Ok(());
    }
    let done = EarlyDone {
        command_id: command_id.to_string_lossy().into_owned(),
        exit_code: 0,
        cwd: cwd.to_string_lossy().into_owned(),
    };
    let text = sonic_rs::to_string(&done).context("failed to serialize early done file")?;
    let temp_path = directory_path.join("done.json.tmp");
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
enum StartupEvent {
    Filesystem(notify::Result<()>),
    Exited(Result<ExitStatus>),
}
